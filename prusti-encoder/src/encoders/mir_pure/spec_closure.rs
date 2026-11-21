//! Spec closures, their captures, and the builtins that only pure code encodes.

use super::*;

impl<'vir: 'enc, 'enc> Enc<'vir, 'enc> {
    /// A read of `place` in the body of a spec closure, where `place` is
    /// rooted at one of the closure's captures, `(*_1).k`. The closure value
    /// `_1` itself is never read: the enclosing body supplies the value of
    /// capture `k` in the modes of each read of it (see
    /// `Enc::encode_spec_closure`), so this read records its modes for `k`
    /// and is not wrapped in them here.
    pub(super) fn encode_capture_read(
        &mut self,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
        place: Place<'vir>,
    ) -> EncodeResult<'vir, EncodedPlace<'vir>, MirPureEnc> {
        let [
            mir::ProjectionElem::Deref,
            mir::ProjectionElem::Field(field, field_ty),
            ref rest @ ..,
        ] = place.projection[..]
        else {
            unreachable!("an `Fn` closure body reads a capture `k` only as `(*_1).k`");
        };
        let modes = self.modes;
        self.captures.as_mut().unwrap()[field].insert(modes);

        // The enclosing body supplies a capture of a shared reference as the
        // place it refers to, with the address and pointer metadata of the
        // reference (see `Enc::encode_capture`), and any other capture as its
        // value.
        let shared_referent_ty = match *field_ty.kind() {
            TyKind::Ref(_, referent_ty, ty::Mutability::Not) => Some(referent_ty),
            _ => None,
        };
        let snap = self.vcx.mk_lazy_expr(
            vir::vir_format!(self.vcx, "capture {field:?}"),
            self.ty_use(shared_referent_ty.unwrap_or(field_ty)).snapshot,
            Box::new(move |_vcx, lctx: ExprInput<'vir>| lctx.captures[field][&modes].snap.kind),
        );
        let Some(referent_ty) = shared_referent_ty else {
            let value = EncodedPlace::new(snap, None);
            let value_ty = mir::PlaceTy::from_ty(field_ty);
            return self.encode_projection(curr_ver, value_ty, value, rest, Modes::empty());
        };

        let address = self.vcx.mk_lazy_expr(
            vir::vir_format!(self.vcx, "address of capture {field:?}"),
            vir::TYPE_REF,
            Box::new(move |_vcx, lctx: ExprInput<'vir>| {
                lctx.captures[field][&modes].place_ref.unwrap().kind
            }),
        );
        let metadata_ty = self
            .context
            .normalize(field_ty.pointee_metadata_ty_or_projection(self.vcx.tcx()));
        let metadata = self.vcx.mk_lazy_expr(
            vir::vir_format!(self.vcx, "metadata of capture {field:?}"),
            self.ty_use(metadata_ty).snapshot,
            Box::new(move |_vcx, lctx: ExprInput<'vir>| {
                lctx.captures[field][&modes].metadata.unwrap().kind
            }),
        );
        // Only a `move` closure, which captures a reference by value, reads the
        // reference itself; a capture by reference is only read through.
        let [mir::ProjectionElem::Deref, ref rest @ ..] = rest[..] else {
            return Err(by_value_ref_capture(self.current_span()));
        };
        let referent = EncodedPlace::new(snap, Some(address)).with_metadata(metadata);
        let referent_ty = mir::PlaceTy::from_ty(referent_ty);
        self.encode_projection(curr_ver, referent_ty, referent, rest, Modes::empty())
    }

    /// Encodes the closure argument of a quantifier/spec-block builtin: the
    /// closure's arguments become the returned quantified variable
    /// declarations (empty for `spec_block`) and its body the returned,
    /// recursively encoded expression. `name` is the builtin's
    /// `prusti_contracts` name, for error reporting.
    ///
    /// The closure body is part of this specification, so when it reads a
    /// capture in some modes, it reads the operand this specification built
    /// the capture from, in those modes (a quantified `old(arr.lookup(i))`
    /// reads the reborrow `&*arr` in the pre-state). Those operands are
    /// encoded here, at `curr_ver` in `block`, which builds the closure and
    /// calls the builtin, and passed to the closure body as its captures (see
    /// `ExprInput`).
    fn encode_spec_closure(
        &mut self,
        name: &str,
        closure_ty: ty::Ty<'vir>,
        block: mir::BasicBlock,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
    ) -> EncodeResult<'vir, SpecClosure<'vir>, MirPureEnc> {
        let (qvar_tys, cl_kind, cl_def_id, cl_args) = match closure_ty.kind() {
            TyKind::Closure(cl_def_id, cl_args) => (
                match cl_args.as_closure().sig().skip_binder().inputs()[0].kind() {
                    TyKind::Tuple(list) => list,
                    _ => unreachable!(),
                },
                cl_args.as_closure().kind(),
                *cl_def_id,
                *cl_args,
            ),
            other => panic!("illegal prusti::{name}: expected closure, got {other:?}"),
        };
        // The builtins' signatures should enforce `Fn`-only closures.
        assert_eq!(cl_kind, ty::ClosureKind::Fn);

        let qvars = self.vcx.alloc_slice(
            &qvar_tys
                .iter()
                .enumerate()
                .map(|(idx, qvar_ty)| {
                    let ty_out = self.ty_use(qvar_ty);
                    self.vcx.mk_local_decl(
                        vir::vir_format!(self.vcx, "qvar_{}_{idx}", self.encoding_depth),
                        ty_out.snapshot,
                    )
                })
                .collect::<Vec<_>>(),
        );

        let mut reify_args = FxHashMap::default();
        // The arguments to the closure are the closure itself, which gets no
        // value as the body reads it only through its captures, and the
        // qvars.
        reify_args.extend(
            qvars
                .iter()
                .enumerate()
                .map(|(idx, qvar)| ((idx + 2).into(), self.vcx.mk_local_ex(qvar))),
        );

        use vir::Reify;
        let out = self.deps.require_dep::<MirPureEnc>(MirPureEncTask {
            encoding_depth: self.encoding_depth + 1,
            kind: PureKind::Closure {
                context: self.context_def_id,
            },
            def_id: cl_def_id,
            // The body becomes part of this expression, so it is encoded
            // in this context, in which `cl_args` are expressed.
            gargs: GArgs::new(self.context, cl_args),
        })?;
        // The specification macros build the closure in the `block` that
        // calls the builtin, after which the block assigns only temporaries
        // (borrows of the closure, the triggers); captured places are rooted
        // at variables, so they read at `curr_ver` what the closure captured.
        // Other closures are not supported. A closure without captures needs
        // no aggregate: its borrow may be promoted to a constant.
        let statements = &self.body[block].statements;
        let built = statements
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, stmt)| match &stmt.kind {
                mir::StatementKind::Assign(box (
                    _,
                    mir::Rvalue::Aggregate(box mir::AggregateKind::Closure(def_id, _), operands),
                )) if *def_id == cl_def_id => Some((index, operands)),
                _ => None,
            });
        let assigns_variable = |stmt: &mir::Statement<'vir>| match &stmt.kind {
            mir::StatementKind::Assign(box (dest, _)) => self.is_variable(dest.local),
            _ => false,
        };
        let operands = match built {
            Some((index, operands)) if !statements[index + 1..].iter().any(assigns_variable) => {
                operands.as_slice()
            }
            None if out.captures.is_empty() => IndexSlice::empty(),
            _ => {
                return Err(EncodeFullError::unsupported(
                    format!("a `{name}` whose closure is not built right before it"),
                    vec![self.current_span()],
                ));
            }
        };
        let mut captures = IndexVec::<abi::FieldIdx, _>::with_capacity(operands.len());
        for (capture, reads) in operands.iter().zip(&out.captures) {
            let mut values = FxHashMap::default();
            for &modes in reads {
                values.insert(modes, self.encode_capture(capture, modes, curr_ver)?);
            }
            captures.push(values);
        }
        let captures = self.vcx.alloc(captures);
        let body = out.expr;
        let reify_args = self.vcx.alloc(reify_args);
        let body = self.vcx.mk_lazy_expr(
            vir::vir_format!(self.vcx, "spec closure body ({name})"),
            body.ty(),
            Box::new(move |vcx, lctx: ExprInput<'vir>| {
                let captures = captures
                    .iter()
                    .map(|values| {
                        values
                            .iter()
                            .map(|(&modes, value)| {
                                let value = EncodedPlaceGen {
                                    snap: value.snap.reify(vcx, lctx),
                                    place_ref: value.place_ref.reify(vcx, lctx),
                                    metadata: value.metadata.reify(vcx, lctx),
                                };
                                (modes, value)
                            })
                            .collect()
                    })
                    .collect::<IndexVec<_, _>>();
                let input = ExprInput {
                    def_id: cl_def_id,
                    inputs: reify_args,
                    old: lctx.old,
                    captures: vcx.alloc(captures),
                };
                body.reify(vcx, input).kind
            }),
        );
        Ok((qvars, body.downcast_ty::<vir::Bool>()))
    }

    /// The assignment statements of this body, as destination and value.
    fn assignments(
        &self,
    ) -> impl Iterator<Item = (&'enc mir::Place<'vir>, &'enc mir::Rvalue<'vir>)> {
        self.body
            .basic_blocks
            .iter()
            .flat_map(|block| &block.statements)
            .filter_map(|stmt| match &stmt.kind {
                mir::StatementKind::Assign(box (dest, rvalue)) => Some((dest, rvalue)),
                _ => None,
            })
    }

    /// Whether `local` is a variable or an argument rather than a temporary:
    /// MIR building gives debug info to every variable, to every argument
    /// bound to a name, and to no temporary. (An argument bound to a pattern
    /// counts as a temporary; it is never captured, as the pattern's bindings
    /// are variables of their own. `LocalDecl::is_user_variable` would panic
    /// in bodies of other crates, where the local info it reads is cleared.)
    fn is_variable(&self, local: mir::Local) -> bool {
        self.body.var_debug_info.iter().any(|info| {
            matches!(info.value, mir::VarDebugInfoContents::Place(place)
                if place.as_local() == Some(local))
        })
    }

    /// A capture of a spec closure, passed to the closure as `capture` and
    /// read in `modes`, encoded in those modes and the modes this body is in
    /// at the builtin. MIR building passes a capture by reference as a
    /// temporary that it assigns the reference, such as `&*arr`; the capture is
    /// encoded as the borrowed place `*arr`, so that the modes reach the read
    /// of `arr`, together with the address and metadata of the reference. A
    /// capture by value is the captured place itself. A shared reference
    /// captured by value, which only a `move` closure makes (the specification
    /// syntax never does), is not supported: the closure body reads it like a
    /// capture by reference, which the place it refers to would have to be
    /// supplied for.
    fn encode_capture(
        &mut self,
        capture: &mir::Operand<'vir>,
        modes: Modes,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
    ) -> EncodeResult<'vir, EncodedPlace<'vir>, MirPureEnc> {
        let temp = capture
            .place()
            .and_then(|place| place.as_local())
            .filter(|&local| !self.is_variable(local));
        let value = temp.and_then(|temp| {
            self.assignments()
                .find_map(|(dest, value)| (dest.as_local() == Some(temp)).then_some(value))
        });

        let outer_modes = self.modes;
        self.modes = outer_modes | modes;
        let res = match value {
            Some(value @ mir::Rvalue::Ref(_, _, place)) => {
                let ref_ty = value.ty(self.body, self.vcx.tcx());
                self.encode_borrowed_place(curr_ver, (*place).into(), ref_ty)
                    .map(|(snap, place_ref, metadata)| {
                        EncodedPlace::new(snap, Some(place_ref)).with_metadata(metadata)
                    })
            }
            Some(value) => self
                .encode_rvalue(curr_ver, value, self.current_span())
                .map(|snap| EncodedPlace::new(snap, None)),
            None if matches!(
                capture.ty(self.body, self.vcx.tcx()).kind(),
                TyKind::Ref(.., ty::Mutability::Not)
            ) =>
            {
                Err(by_value_ref_capture(self.current_span()))
            }
            None => self
                .encode_operand_snap(capture, curr_ver)
                .map(|snap| EncodedPlace::new(snap, None)),
        };
        self.modes = outer_modes;
        res
    }

    /// Encodes the pure-only `prusti_contracts` builtins (quantifiers, spec
    /// blocks, and the old/rel/before-expiry mode markers), which need this
    /// encoder's own state. The operand-based builtins are handled by the
    /// shared [`PureRvalueEnc::encode_prusti_builtin`].
    pub(super) fn encode_pure_only_builtin(
        &mut self,
        builtin: SpecBuiltin,
        arg_tys: ty::GenericArgsRef<'vir>,
        args: &[Spanned<mir::Operand<'vir>>],
        block: mir::BasicBlock,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let mk_bool = |prim: vir::ExprGenBool<'vir, _, _>| prim.upcast_ty::<vir::CSnap>();
        Ok(match builtin {
            SpecBuiltin::Forall | SpecBuiltin::Exists => {
                assert_eq!(arg_tys.len(), 3);
                // The operands are the triggers and the closure.
                assert_eq!(args.len(), 2);

                let name = if builtin == SpecBuiltin::Forall {
                    "forall"
                } else {
                    "exists"
                };
                let (qvars, body) =
                    self.encode_spec_closure(name, arg_tys[2].expect_ty(), block, curr_ver)?;
                // TODO: triggers
                let res = if builtin == SpecBuiltin::Forall {
                    self.vcx.mk_forall_expr(qvars, &[], body)
                } else {
                    self.vcx.mk_exists_expr(qvars, &[], body)
                };
                mk_bool(res)
            }
            SpecBuiltin::SpecBlock => {
                assert_eq!(arg_tys.len(), 2);
                assert_eq!(args.len(), 1);

                let (qvars, body) = self.encode_spec_closure(
                    "spec_block",
                    arg_tys[1].expect_ty(),
                    block,
                    curr_ver,
                )?;
                assert!(qvars.is_empty(), "`spec_block` closures take no arguments");
                mk_bool(body)
            }
            SpecBuiltin::ModeStart(mode) => {
                match mode {
                    Mode::Old => {
                        assert!(!self.modes.contains(Modes::OLD));
                        self.modes.insert(Modes::OLD);
                    }
                    Mode::Rel(0) => {
                        assert!(!self.modes.contains(Modes::REL0));
                        assert!(!self.modes.contains(Modes::REL1));
                        self.modes.insert(Modes::REL0);
                    }
                    Mode::Rel(_) => {
                        assert!(!self.modes.contains(Modes::REL1));
                        assert!(!self.modes.contains(Modes::REL0));
                        self.modes.insert(Modes::REL1);
                    }
                    Mode::BeforeExpiry => {
                        assert!(!self.modes.contains(Modes::BEFORE_EXPIRY));
                        self.modes.insert(Modes::BEFORE_EXPIRY);
                    }
                }
                mk_bool(self.vcx.mk_bool::<true>().lazy()) // TODO: what value do we return?
            }
            SpecBuiltin::ModeEnd(mode) => {
                match mode {
                    Mode::Old => {
                        assert!(self.modes.contains(Modes::OLD));
                        self.modes.remove(Modes::OLD);
                    }
                    Mode::Rel(0) => {
                        assert!(self.modes.contains(Modes::REL0));
                        assert!(!self.modes.contains(Modes::REL1));
                        self.modes.remove(Modes::REL0);
                    }
                    Mode::Rel(_) => {
                        assert!(!self.modes.contains(Modes::REL0));
                        assert!(self.modes.contains(Modes::REL1));
                        self.modes.remove(Modes::REL1);
                    }
                    Mode::BeforeExpiry => {
                        assert!(self.modes.contains(Modes::BEFORE_EXPIRY));
                        self.modes.remove(Modes::BEFORE_EXPIRY);
                    }
                }
                mk_bool(self.vcx.mk_bool::<true>().lazy()) // TODO: what value do we return?
            }
        }
        .upcast_ty())
    }
}

/// The error for a shared reference that a spec closure captures by value (see
/// `Enc::encode_capture`).
fn by_value_ref_capture<'vir>(span: Span) -> EncodeFullError<'vir, MirPureEnc> {
    EncodeFullError::unsupported(
        "a reference captured by value in a spec closure".to_string(),
        vec![span],
    )
}
