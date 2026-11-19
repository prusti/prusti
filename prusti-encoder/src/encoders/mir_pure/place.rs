//! Reads of places and updates of places.

use super::*;

impl<'vir: 'enc, 'enc> Enc<'vir, 'enc> {
    fn encode_place_element(
        &mut self,
        curr_ver: &Versions<'vir>,
        place_ty: mir::PlaceTy<'vir>,
        elem: mir::PlaceElem<'vir>,
        encoded_place: EncodedPlace<'vir>,
    ) -> EncodeResult<'vir, EncodedPlace<'vir>, MirPureEnc> {
        let e_ty = self.ty_use(place_ty.ty)?;
        Ok(match elem {
            mir::ProjectionElem::Deref => {
                assert!(place_ty.variant_index.is_none());
                match place_ty.ty.kind() {
                    TyKind::Adt(adt, _) if adt.is_box() => {
                        // The boxed value, its address and its pointer metadata
                        // are all read out of the snapshot.
                        let data = e_ty.expect_structlike();
                        let snap = encoded_place.snap.downcast_ty();
                        let val_expr = data.box_value_access(snap);
                        let place_ref = Some(data.box_address_access(snap));
                        let metadata = data.box_metadata_access(snap);
                        EncodedPlace::new(val_expr, place_ref)
                            .with_metadata(metadata)
                            .with_in_deep_snap(encoded_place.in_deep_snap)
                    }
                    TyKind::Ref(.., ty::Mutability::Not) => {
                        let e_ty = e_ty.expect_immref();
                        let snap = encoded_place.snap.downcast_ty();
                        let metadata = e_ty.metadata_access(snap);
                        let val_expr = e_ty.value_access(snap);
                        let ref_expr = e_ty.addr_access(snap);
                        EncodedPlace::new(val_expr, Some(ref_expr)).with_metadata(metadata)
                    }
                    TyKind::Ref(.., ty::Mutability::Mut) => {
                        let e_ty = e_ty.expect_mutref();
                        let snap = encoded_place.snap.downcast_ty();
                        let metadata = e_ty.metadata_access(snap);
                        let ref_expr = e_ty.deref_access(snap);
                        if encoded_place.in_deep_snap {
                            return Ok(EncodedPlace::new(e_ty.value_access(snap), Some(ref_expr))
                                .with_metadata(metadata)
                                .with_in_deep_snap(true));
                        }
                        if !self.impure_context {
                            // The value field of a mutable reference's snapshot
                            // is never constrained to the referent (see
                            // `p_Ref_mutable_arbitrary_value`).
                            return Err(EncodeFullError::unsupported(
                                format!(
                                    "dereference of the mutable reference `{}` in pure code",
                                    place_ty.ty
                                ),
                                vec![self.current_span()],
                            ));
                        }
                        let val_expr = {
                            // The snapshot is shallow and doesn't contain the
                            // value behind the mutable reference, so we need to
                            // take an extra snapshot here.
                            // TODO: avoid all of this by using shallow and deep snapshots
                            let ty_task = RustTyDecomposition::from_ty(place_ty.ty, self.context);
                            let inner = ty_task.ty.expect_mutref();
                            let normalized = inner
                                .referent
                                .decompose_compare_normalize(ty_task.ty.params, ty_task.args);
                            let caster = self
                                .deps
                                .require_dep::<crate::GArgsCastEnc<crate::Pure>>(normalized)?;
                            let inner_ty_task = inner
                                .referent
                                .decompose_context(ty_task.ty.params, ty_task.args);
                            let inner_ty = self
                                .deps
                                .require_dep::<crate::encoders::TyUseImpureEnc>(inner_ty_task)?;
                            caster.cast_to_caller_ctx(inner_ty.ref_to_snap(ref_expr))
                        };
                        EncodedPlace::new(val_expr, Some(ref_expr)).with_metadata(metadata)
                    }
                    TyKind::RawPtr(..) => {
                        return Err(EncodeFullError::unsupported(
                            format!("dereference of the raw pointer `{}`", place_ty.ty),
                            vec![self.current_span()],
                        ));
                    }
                    _ => unreachable!(),
                }
            }
            mir::ProjectionElem::Field(field_idx, _) => {
                if e_ty.specifics.is_opaque() {
                    return Err(unsupported_opaque(place_ty.ty, self.current_span()));
                }
                let variant = e_ty.expect_variant_opt(place_ty.variant_index);
                let proj = variant[field_idx];
                let proj_app = proj.read(encoded_place.snap.downcast_ty());
                let place_ref = encoded_place.place_ref.map(|pr| proj.field_ref(pr));
                let place = EncodedPlace::new(proj_app, place_ref)
                    .with_in_deep_snap(encoded_place.in_deep_snap);
                // Only the last field can be an unsized DST tail that shares the
                // containing value's pointer metadata; propagate it there, and
                // nowhere else (every other field is sized and thin).
                let is_last_field = field_idx.index() + 1 == variant.fields.len();
                match encoded_place.metadata {
                    Some(metadata) if is_last_field => place.with_metadata(metadata),
                    _ => place,
                }
            }
            mir::ProjectionElem::Index(idx) => {
                let proj = e_ty.expect_array();
                let idx = self
                    .encode_place_with_ref(curr_ver, mir::Place::from(idx).into())?
                    .snap
                    .downcast_ty();
                let proj_app = proj.index(encoded_place.snap.downcast_ty(), idx);
                let place_ref = encoded_place
                    .place_ref
                    .map(|pr| proj.ref_to_index_ref(pr, idx));
                EncodedPlace::new(proj_app, place_ref).with_in_deep_snap(encoded_place.in_deep_snap)
            }
            // `from_end` indexes from the runtime length of a slice; it is
            // always false for an array, whose statically known length lets
            // the compiler resolve the offset. Only the array form is
            // supported (a slice is matched behind a reference and its
            // length is not in the snapshot's type).
            mir::ProjectionElem::ConstantIndex {
                offset,
                from_end: false,
                ..
            } => {
                let proj = e_ty.expect_array();
                let idx = self.usize_const_snap(offset);
                let proj_app = proj.index(encoded_place.snap.downcast_ty(), idx);
                let place_ref = encoded_place
                    .place_ref
                    .map(|pr| proj.ref_to_index_ref(pr, idx));
                EncodedPlace::new(proj_app, place_ref).with_in_deep_snap(encoded_place.in_deep_snap)
            }
            mir::ProjectionElem::Downcast(..) => encoded_place,
            _ => {
                return Err(EncodeFullError::unsupported(
                    format!("projection `{elem:?}` in pure code"),
                    vec![self.current_span()],
                ));
            }
        })
    }

    /// The parts of a reference or raw pointer of type `ref_ty` to `place`:
    /// the snapshot of `place`, its address and the pointer metadata.
    pub(super) fn encode_borrowed_place(
        &mut self,
        curr_ver: &Versions<'vir>,
        place: Place<'vir>,
        ref_ty: ty::Ty<'vir>,
    ) -> EncodeResult<'vir, (ExprRet<'vir>, ExprRetRef<'vir>, ExprRet<'vir>), MirPureEnc> {
        let encoded_place = self.encode_place_with_ref(curr_ver, place)?;
        // A place behind a dereference has the address that the reference or
        // box holds. Any other place exists only in this expression; a
        // reference to it cannot leave pure code, and gets the address null.
        let place_ref = encoded_place
            .place_ref
            .unwrap_or_else(|| self.vcx.mk_null().lazy());
        let metadata = encoded_place
            .metadata
            .unwrap_or_else(|| self.expect_thin_ptr_metadata(ref_ty));
        Ok((encoded_place.snap, place_ref, metadata))
    }

    /// Whether `place` dereferences a mutable reference.
    fn derefs_mut_ref(&self, place: Place<'vir>) -> bool {
        let mut place_ty = mir::PlaceTy::from_ty(self.body.local_decls[place.local].ty);
        place.projection.iter().any(|elem| {
            let derefs = matches!(elem, mir::ProjectionElem::Deref)
                && matches!(place_ty.ty.kind(), TyKind::Ref(.., ty::Mutability::Mut));
            place_ty = place_ty.projection_ty(self.vcx.tcx(), *elem);
            derefs
        })
    }

    /// The snapshot `snap` of `local`, extended through the local's indirect
    /// predicates so that its `&mut`s hold their referents.
    fn deep_snap(
        &mut self,
        local: mir::Local,
        snap: ExprRet<'vir>,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let ty = RustTyDecomposition::from_ty(self.body.local_decls[local].ty, self.context);
        let nodes = NodePredicates::new(ty, self.deps)?;
        Ok(self.vcx.mk_lazy_expr(
            "deep_snap",
            snap.ty(),
            Box::new(move |vcx, lctx: ExprInput<'vir>| {
                use vir::Reify;
                nodes.deep_snap(snap.reify(vcx, lctx)).kind
            }),
        ))
    }

    /// A read of `place` at the versions `curr_ver` of the locals, in the
    /// current modes: its snapshot, with the address and pointer metadata
    /// that the dereferences in `place` give it.
    pub(super) fn encode_place_with_ref(
        &mut self,
        curr_ver: &Versions<'vir>,
        place: Place<'vir>,
    ) -> EncodeResult<'vir, EncodedPlace<'vir>, MirPureEnc> {
        if self.captures.is_some() && place.local == ty::CAPTURE_STRUCT_LOCAL {
            return self.encode_capture_read(curr_ver, place);
        }

        let local = place.local;
        // The modes change only a read of an argument or the result (see
        // `Modes`).
        let modes = match self.body.local_kind(local) {
            mir::LocalKind::Arg | mir::LocalKind::ReturnPointer => self.modes,
            mir::LocalKind::Temp => Modes::empty(),
        };
        let snap = if modes.is_empty() {
            let version = curr_ver[&local];
            self.uses.read(local, version.index);
            self.mk_local_ex(version)
        } else {
            // A wrapper such as `old` only reinterprets the reads written
            // inside it, and a variable keeps its value under it, so a read
            // in a mode starts at the expression the body's inputs supply for
            // the local rather than at the variable the body binds to it.
            self.uses.read(local, 0);
            self.input(local)?
        };
        // Only reads behind a `&mut` need the extension.
        let in_deep_snap = self.has_deep_snap(local) && self.derefs_mut_ref(place);
        let snap = if in_deep_snap {
            self.deep_snap(local, snap)?
        } else {
            snap
        };
        let local_ty = mir::PlaceTy::from_ty(self.body.local_decls[local].ty);
        self.encode_projection(
            curr_ver,
            local_ty,
            EncodedPlace::new(snap, None).with_in_deep_snap(in_deep_snap),
            place.projection,
            modes,
        )
    }

    /// Applies `projection` to `base`, an encoded place of type `base_ty`,
    /// for a read in `modes`, which are empty unless the read is rooted at an
    /// argument or the result.
    // TODO: factor this out (duplication with impure encoder)?
    pub(super) fn encode_projection(
        &mut self,
        curr_ver: &Versions<'vir>,
        base_ty: mir::PlaceTy<'vir>,
        base: EncodedPlace<'vir>,
        projection: &[mir::PlaceElem<'vir>],
        modes: Modes,
    ) -> EncodeResult<'vir, EncodedPlace<'vir>, MirPureEnc> {
        let mut place_ty = base_ty;
        let mut encoded_place = base;
        for &elem in projection {
            // The snapshots of the arguments and the result are pinned to a
            // state, so the only read of the heap is the dereference of a
            // mutable reference in an impure context; only that is wrapped.
            let heap_read = self.impure_context
                && matches!(elem, mir::ProjectionElem::Deref)
                && matches!(place_ty.ty.kind(), TyKind::Ref(.., ty::Mutability::Mut));
            encoded_place = self.encode_place_element(curr_ver, place_ty, elem, encoded_place)?;
            place_ty = place_ty.projection_ty(self.vcx.tcx(), elem);
            if heap_read && modes.contains(Modes::OLD) {
                // An `old` around a pinned snapshot would be a no-op, but would
                // break the match with the callee's wand at a call site, where
                // the `old` is labelled. The label is only known at
                // instantiation (`ExprInput::old`).
                let inner = encoded_place.snap;
                encoded_place.snap = self.vcx.mk_lazy_expr(
                    "old_mode_wrap",
                    inner.ty(),
                    Box::new(move |vcx, lctx: ExprInput<'vir>| {
                        use vir::Reify;
                        let reified = inner.reify(vcx, lctx);
                        vcx.mk_old(reified, lctx.old).kind
                    }),
                );
            }
            if heap_read && modes.contains(Modes::BEFORE_EXPIRY) {
                // Silicon evaluates the heap-independent parts of a wand when
                // inhaling it, where `old[lhs]` is undefined, so an `old[lhs]`
                // around a pinned snapshot would be ill-defined there.
                encoded_place.snap = self.vcx.mk_old_lhs_expr(encoded_place.snap);
            }
        }
        // A downcast is always followed by a field of the variant, so no
        // place that is read ends in one.
        assert!(place_ty.variant_index.is_none());

        if modes.contains(Modes::REL0) {
            encoded_place.snap = self.vcx.mk_rel_expr(encoded_place.snap, 0);
        }
        if modes.contains(Modes::REL1) {
            encoded_place.snap = self.vcx.mk_rel_expr(encoded_place.snap, 1);
        }
        Ok(encoded_place)
    }

    /// The snapshot of `place.local` after `value` is written to `place`: the
    /// old snapshot with only the part at `place` replaced.
    pub(super) fn encode_place_update(
        &mut self,
        curr_ver: &Versions<'vir>,
        place: mir::Place<'vir>,
        mut value: ExprRet<'vir>,
        span: Span,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let Some((_, prefix)) = place.projection.split_last() else {
            return Ok(value);
        };
        // The type and snapshot of each place that `place` projects from.
        let mut base_ty = mir::PlaceTy::from_ty(self.body.local_decls[place.local].ty);
        let mut base =
            self.encode_place_with_ref(curr_ver, mir::Place::from(place.local).into())?;
        let mut bases = vec![(base_ty, base.snap)];
        for &elem in prefix {
            base = self.encode_place_element(curr_ver, base_ty, elem, base)?;
            base_ty = base_ty.projection_ty(self.vcx.tcx(), elem);
            bases.push((base_ty, base.snap));
        }
        for ((base_ty, base_snap), &elem) in bases.into_iter().zip(&place.projection[..]).rev() {
            let e_ty = self.ty_use(base_ty.ty)?;
            value = match elem {
                mir::ProjectionElem::Downcast(..) => value,
                mir::ProjectionElem::Field(..) if e_ty.specifics.is_opaque() => {
                    return Err(unsupported_opaque(base_ty.ty, span));
                }
                mir::ProjectionElem::Field(field_idx, _) => e_ty
                    .expect_variant_opt(base_ty.variant_index)
                    .with_field(base_snap.downcast_ty(), field_idx, value)
                    .upcast_ty(),
                mir::ProjectionElem::Deref if base_ty.ty.is_box() => e_ty
                    .expect_structlike()
                    .with_box_value(base_snap.downcast_ty(), value)
                    .upcast_ty(),
                // TODO: array elements need a function on the array snapshot
                // that replaces one element; writes through a `&mut` need its
                // referent's value, which pure code does not have.
                _ => {
                    return Err(EncodeFullError::unsupported(
                        format!(
                            "assignment to a part of a `{}` in pure code: only fields and \
                             boxed values can be assigned to",
                            base_ty.ty
                        ),
                        vec![span],
                    ));
                }
            };
        }
        Ok(value)
    }
}
