use rustc_hash::FxHashMap;

use pcg::{
    borrow_pcg::region_projection::{
        ExtractRegionsCtxt, LifetimeProjection, PcgLifetimeProjectionBase,
        PcgLifetimeProjectionBaseLike,
    },
    r#loop::{LoopId, PlaceUsages},
    pcg::{CapabilityKind, EvalStmtPhase, PcgNode},
    results::PcgBasicBlock,
    utils::{
        HasCompilerCtxt, HasPlace, Place, PrefixRelation, maybe_old::MaybeLabelledPlace,
        maybe_remote::MaybeRemotePlace,
    },
};
use prusti_rustc_interface::{index::Idx, middle::mir};

use task_encoder::TaskEncoder;
use vir::{CastType, Foldable, Reify};

use crate::encoders::{
    EncodeResult, ImpureEncVisitor, TyUseImpureEnc,
    custom::ReadPermEnc,
    ty::{RustTyDecomposition, indirect::IndirectPredicatesEnc, use_impure::TyUseImpure},
};

pub(crate) enum WandOldOuter<'vir> {
    LetBind(Vec<LetBind<'vir>>),
    Label(Option<&'vir str>),
}

impl<'vir: 'a, 'a, 'enc, E: TaskEncoder> ImpureEncVisitor<'vir, 'enc, E> {
    /// Calculate invariant at loop head
    pub(crate) fn get_loop_inv(
        &mut self,
        loop_id: LoopId,
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
        self.loop_read_places.insert(
            loop_id,
            loop_invariant_place_capabilities
                .iter()
                .filter(|(_, capability)| matches!(capability, CapabilityKind::Read))
                .map(|(place, _)| *place)
                .collect(),
        );

        for (place, capability) in loop_invariant_place_capabilities.iter() {
            if capability.is_write() {
                continue; // No permissions are encoded for places with write capabilities currently
            }
            let (place_res, _snap, _, _) = self.encode_place_with_snap(*place)?;
            let ty = (*place).ty(self.pcg_ctxt());
            let task = RustTyDecomposition::from_ty(ty.ty, self.def_id);
            let ty_out = self.deps.require_dep::<TyUseImpureEnc>(task).unwrap();
            let frac = self.read_frac(*place)?;
            let pred = ty_out.ref_to_pred(self.vcx, place_res.expr.expect_predicate(), frac);
            inv.push(pred);
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
                    let pred = p.reify(
                        self.vcx,
                        ty_out.ref_to_snap(place_res.expr.expect_predicate()),
                    );
                    match frac {
                        Some(frac) => pred.fold_with(&mut ScalePerms(frac)).unwrap_or(pred),
                        None => pred,
                    }
                }));
            }
        }

        for edge in state.borrow_pcg().graph().coupled_edges() {
            let mut let_bind = WandOldOuter::LetBind(Vec::new());
            let (wand, _) = self.coupled_wand(edge.value(), &mut let_bind)?;
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

    /// The fraction of its permission held for `place` in the current block,
    /// `None` for all of it. An invariant keeps only `read()` of the
    /// permission of a place the loop only reads; the rest stays in the
    /// frame, which preserves the place's value across the loop. A nested
    /// loop keeps half of what the enclosing one holds (not `read() * read()`,
    /// which would make permission checks nonlinear).
    pub(crate) fn read_frac(
        &mut self,
        place: Place<'vir>,
    ) -> EncodeResult<'vir, Option<vir::ExprPerm<'vir>>, E> {
        let (Some(block), Some(pres)) = (self.current_block, self.current_block_pres.as_ref())
        else {
            return Ok(None);
        };
        let read_loops = self
            .loop_analysis()
            .loops(block)
            .filter(|l| !pres.contains(&l.index()))
            .filter(|l| {
                self.loop_read_places
                    .get(l)
                    .is_some_and(|places| places.iter().any(|p| p.is_prefix_of(place)))
            })
            .count();
        if read_loops == 0 {
            return Ok(None);
        }
        let read = (self.deps.require_dep::<ReadPermEnc>(())?.read)();
        if read_loops == 1 {
            return Ok(Some(read));
        }
        let denominator = self
            .vcx
            .mk_const_expr(vir::ConstData::Int(1 << (read_loops - 1)))
            .downcast_ty::<vir::Int>();
        let half_powers = self
            .vcx
            .mk_bin_op_expr(
                vir::BinOpKind::FracPerm,
                self.vcx.mk_uint::<1>(),
                denominator,
            )
            .downcast_ty();
        Ok(Some(
            self.vcx
                .mk_bin_op_expr(vir::BinOpKind::PermMul, read, half_powers)
                .downcast_ty(),
        ))
    }

    pub(crate) fn encode_pcg_node<T: PcgLifetimeProjectionBaseLike<'vir>>(
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

/// Takes a fraction of every permission an assertion holds.
struct ScalePerms<'vir>(vir::ExprPerm<'vir>);

impl<'vir> ScalePerms<'vir> {
    fn scale(&self, perm: Option<vir::ExprPerm<'vir>>) -> vir::ExprPerm<'vir> {
        perm.map_or(self.0, |perm| {
            vir::with_vcx(|vcx| {
                vcx.mk_bin_op_expr(vir::BinOpKind::PermMul, perm, self.0)
                    .downcast_ty()
            })
        })
    }
}

impl<'vir> vir::Folder<'vir, (), !> for ScalePerms<'vir> {
    fn fold_expr(&mut self, e: vir::ExprDyn<'vir>) -> Option<vir::ExprDyn<'vir>> {
        vir::with_vcx(|vcx| match e.kind {
            vir::ExprKindGenData::PredicateApp(app) => Some(
                vcx.mk_predicate_app_expr(vcx.alloc(vir::PredicateAppGenData {
                    target: app.target,
                    args: app.args,
                    perm: Some(self.scale(app.perm)),
                }))
                .as_dyn(),
            ),
            vir::ExprKindGenData::AccField(acc) => Some(
                vcx.mk_acc_field_expr(acc.recv, acc.field, Some(self.scale(acc.perm)))
                    .as_dyn(),
            ),
            _ => e.super_fold_with(self),
        })
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
