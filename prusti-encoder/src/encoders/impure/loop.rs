use rustc_hash::FxHashMap;

use pcg::{
    borrow_pcg::region_projection::{
        ExtractRegionsCtxt, LifetimeProjection, PcgLifetimeProjectionBase,
        PcgLifetimeProjectionBaseLike,
    },
    r#loop::PlaceUsages,
    pcg::{CapabilityKind, EvalStmtPhase, PcgNode},
    results::PcgBasicBlock,
    utils::{
        HasCompilerCtxt, HasPlace, Place, maybe_old::MaybeLabelledPlace,
        maybe_remote::MaybeRemotePlace,
    },
};
use prusti_rustc_interface::middle::{mir, ty};

use task_encoder::TaskEncoder;
use vir::Reify;

use crate::encoders::{
    EncodeResult, ImpureEncVisitor, TyUseImpureEnc,
    ty::{RustTyDecomposition, indirect::IndirectPredicatesEnc, use_impure::TyUseImpure},
};

pub(super) enum WandOldOuter<'vir> {
    LetBind(Vec<LetBind<'vir>>),
    Label(Option<&'vir str>),
}

impl<'vir: 'a, 'a, 'enc, E: TaskEncoder> ImpureEncVisitor<'vir, 'enc, E> {
    /// Calculate invariant at loop head
    pub(crate) fn get_loop_inv(
        &mut self,
        cfpcs: &PcgBasicBlock<'_, 'vir>,
        loop_place_usages: &PlaceUsages<'vir>,
        ctxt: impl HasCompilerCtxt<'a, 'vir>,
    ) -> EncodeResult<
        'vir,
        (
            Vec<vir::ExprBool<'vir>>,
            FxHashMap<Place<'vir>, CapabilityKind>,
        ),
        E,
    > {
        let mut inv = Vec::new();
        let start = &cfpcs.statements[0];
        let state = &start.states[EvalStmtPhase::PreOperands];
        let loop_invariant_place_capabilities =
            cfpcs.loop_invariant_place_capabilities(loop_place_usages, ctxt);

        // The atoms of the interior-mutable values the kept places reach.
        let mut im_entries = Vec::new();
        for (place, capability) in loop_invariant_place_capabilities.iter() {
            if self.is_spec_only_local(place.local) {
                continue;
            }
            if capability.is_write() {
                // A place without a value holds its `Uninit` token.
                inv.push(self.place_uninit(*place)?);
                continue;
            }
            let (place_res, _snap, _, _) = self.encode_place_with_snap(*place)?;
            let ty = (*place).ty(self.pcg_ctxt());
            let task = RustTyDecomposition::from_ty(ty.ty, self.def_id);
            let ty_out = self.deps.require_dep::<TyUseImpureEnc>(task).unwrap();
            // A place the loop only reads keeps the rest of its permission
            // outside the loop, so its value (e.g. the address a `&Cell`'s
            // atom is keyed on) stays the same.
            let perm = (matches!(capability, CapabilityKind::Read) && read_part_supported(ty.ty))
                .then(|| self.loop_read_part());
            let pred = ty_out.ref_to_pred(self.vcx, place_res.expr.expect_predicate(), perm);
            inv.push(pred);
            if crate::encoders::ty::im::reaches_im(task) {
                let addr = place_res.expr.expect_predicate();
                im_entries.extend(crate::encoders::ty::im::footprint(
                    self.deps,
                    task,
                    addr,
                    ty_out.ref_to_snap(addr),
                )?);
            }
            // gets all the lifetime projections of the place
            // for example, if we add an invariant for x and x is a reference, then we also want to add
            // an invariant for *x
            let projs = ctxt.extract_lifetime_projections(place.with_inherent_region(ctxt));
            for proj in projs {
                let indirect = self
                    .deps
                    .require_dep::<IndirectPredicatesEnc>(proj.with_base(task))
                    .unwrap();
                inv.extend(indirect.predicate_applications.iter().map(|p| {
                    p.reify(
                        self.vcx,
                        ty_out.ref_to_snap(place_res.expr.expect_predicate()),
                    )
                }));
            }
        }

        if !im_entries.is_empty() {
            inv.push(crate::encoders::ty::im::loop_footprint_assertion(
                &im_entries,
            ));
        }

        for (inputs, outputs) in self.get_abstraction_edges(state.borrow_pcg().graph()) {
            let mut let_bind = WandOldOuter::LetBind(Vec::new());
            let mut wand_rhs = Vec::new();
            for i in inputs {
                self.encode_pcg_node(&i, &mut wand_rhs, &mut let_bind)?;
            }
            let mut wand_lhs = Vec::new();
            for i in outputs {
                let i = match *i {
                    PcgNode::LifetimeProjection(region_projection) => region_projection,
                    PcgNode::Place(_) => unreachable!(),
                };
                let exprs = self.encode_lifetime_projection(i, &mut let_bind)?;
                wand_lhs.extend(exprs);
            }
            let wand = self.vcx.mk_wand(
                self.vcx.mk_conj(self.vcx.alloc_slice(&wand_lhs)),
                self.vcx.mk_conj(self.vcx.alloc_slice(&wand_rhs)),
            );
            let mut wand = self.vcx.mk_wand_expr(wand);
            let WandOldOuter::LetBind(let_bind) = let_bind else {
                unreachable!()
            };
            for lb in let_bind {
                wand = lb.map_or_else(
                    |(d, e)| self.vcx.mk_let_expr(d, e, wand),
                    |(d, e)| self.vcx.mk_let_expr(d, e, wand),
                );
            }
            inv.push(wand);
        }

        Ok((inv, loop_invariant_place_capabilities.into_iter().collect()))
    }

    pub(super) fn encode_pcg_node<T: PcgLifetimeProjectionBaseLike<'vir>>(
        &mut self,
        node: &PcgNode<'vir, MaybeLabelledPlace<'vir>, T>,
        wand_rhs: &mut Vec<vir::ExprBool<'vir>>,
        old_outer: &mut WandOldOuter<'vir>,
    ) -> EncodeResult<'vir, (), E> {
        match node {
            PcgNode::Place(place) => {
                let p = place.place();
                let ty = (*p).ty(self.local_decls, self.vcx.tcx());
                let task = RustTyDecomposition::from_ty(ty.ty, self.def_id);
                let ty_out = self.deps.require_dep::<TyUseImpureEnc>(task).unwrap();
                let p = self.encode_place(p)?;
                let p = self.configure_old((*place).into(), p.expr.expect_predicate(), old_outer);

                let pred = ty_out.ref_to_pred(self.vcx, p, None);
                wand_rhs.push(pred);
            }
            PcgNode::LifetimeProjection(r) => {
                let exprs = self.encode_lifetime_projection(*r, old_outer)?;
                wand_rhs.extend(exprs);
            }
        }
        Ok(())
    }

    pub(super) fn encode_lifetime_projection<T: PcgLifetimeProjectionBaseLike<'vir>>(
        &mut self,
        r: LifetimeProjection<'vir, T>,
        old_outer: &mut WandOldOuter<'vir>,
    ) -> EncodeResult<'vir, Vec<vir::ExprBool<'vir>>, E> {
        let place = r.base().to_pcg_lifetime_projection_base();
        let (place_snap, ty, _) = match place {
            PcgLifetimeProjectionBase::Place(p) => {
                self.encode_maybe_remote_place_snap(p, old_outer)?
            }
            PcgLifetimeProjectionBase::Const(c) => todo!("{c:?}"),
        };
        let ty = RustTyDecomposition::from_ty(ty.ty, self.def_id);
        let indirect = self
            .deps
            .require_dep::<IndirectPredicatesEnc>(r.with_base(ty))
            .unwrap();
        Ok(indirect
            .predicate_applications
            .into_iter()
            .map(|expr| expr.reify(self.vcx, place_snap))
            .collect::<Vec<_>>())
    }

    fn get_place(place: MaybeRemotePlace<'vir>) -> Place<'vir> {
        match place {
            MaybeRemotePlace::Local(MaybeLabelledPlace::Current(place)) => place,
            MaybeRemotePlace::Local(MaybeLabelledPlace::Labelled(place)) => place.place(),
            MaybeRemotePlace::Remote(r) => r.assigned_local().into(),
        }
    }

    fn encode_maybe_remote_place_snap(
        &mut self,
        place: MaybeRemotePlace<'vir>,
        old_outer: &mut WandOldOuter<'vir>,
    ) -> EncodeResult<'vir, (vir::ExprSnap<'vir>, mir::PlaceTy<'vir>, TyUseImpure<'vir>), E> {
        let p = Self::get_place(place);
        let (_, place_snap, ty, ty_out) = self.encode_place_with_snap(p)?;
        let place_snap = self.configure_old(place, place_snap, old_outer);
        Ok((place_snap, ty, ty_out))
    }

    fn configure_old<T: SnapOrRef>(
        &mut self,
        place: MaybeRemotePlace,
        expr: vir::Expr<'vir, T>,
        old_outer: &mut WandOldOuter<'vir>,
    ) -> vir::Expr<'vir, T> {
        match place {
            MaybeRemotePlace::Local(MaybeLabelledPlace::Current { .. }) => {
                self.mk_wand_outer(expr, old_outer)
            }
            MaybeRemotePlace::Local(MaybeLabelledPlace::Labelled(place)) => {
                let label = self.get_location_label(place.at());
                self.vcx.mk_old(expr, label)
            }
            MaybeRemotePlace::Remote(_) => self.vcx.mk_old_expr(expr),
        }
    }

    fn mk_wand_outer<T: SnapOrRef>(
        &mut self,
        expr: vir::Expr<'vir, T>,
        old_outer: &mut WandOldOuter<'vir>,
    ) -> vir::Expr<'vir, T> {
        match old_outer {
            WandOldOuter::LetBind(let_bind) => {
                let ident = vir::vir_format!(self.vcx, "_lb{}", let_bind.len());
                let decl = self.vcx.mk_local_decl(ident, expr.ty());
                let_bind.push(T::as_result(decl, expr));
                self.vcx.mk_local_ex(decl)
            }
            WandOldOuter::Label(label) => {
                let label = *label.get_or_insert_with(|| self.new_label("outer_package"));
                self.vcx.mk_local_labelled_old_expr(expr, label)
            }
        }
    }
}

type LetBind<'vir> = Result<
    (vir::LocalDeclSnap<'vir>, vir::ExprSnap<'vir>),
    (vir::LocalDeclRef<'vir>, vir::ExprRef<'vir>),
>;

trait SnapOrRef: vir::CompType {
    fn as_result<'vir>(d: vir::LocalDecl<'vir, Self>, e: vir::Expr<'vir, Self>) -> LetBind<'vir>;
}

impl SnapOrRef for vir::Snap {
    fn as_result<'vir>(d: vir::LocalDecl<'vir, Self>, e: vir::Expr<'vir, Self>) -> LetBind<'vir> {
        Ok((d, e))
    }
}

impl SnapOrRef for vir::Ref {
    fn as_result<'vir>(d: vir::LocalDecl<'vir, Self>, e: vir::Expr<'vir, Self>) -> LetBind<'vir> {
        Err((d, e))
    }
}

/// Whether a place of type `ty` can be held (and repacked) at a part of its
/// permission: it holds no array or slice and no mutable reference inline,
/// whose repacks need the whole permission.
pub(crate) fn read_part_supported(ty: ty::Ty<'_>) -> bool {
    fn go<'tcx>(tcx: ty::TyCtxt<'tcx>, ty: ty::Ty<'tcx>, seen: &mut Vec<ty::Ty<'tcx>>) -> bool {
        if seen.contains(&ty) {
            return true;
        }
        seen.push(ty);
        match *ty.kind() {
            ty::TyKind::Array(..) | ty::TyKind::Slice(_) | ty::TyKind::Ref(_, _, ty::Mutability::Mut) => {
                false
            }
            ty::TyKind::Tuple(tys) => tys.iter().all(|ty| go(tcx, ty, seen)),
            ty::TyKind::Adt(adt, args) => adt
                .all_fields()
                .all(|field| go(tcx, field.ty(tcx, args).skip_normalization(), seen)),
            _ => true,
        }
    }
    vir::with_vcx(|vcx| go(vcx.tcx(), ty, &mut Vec::new()))
}
