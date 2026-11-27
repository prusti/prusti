use rustc_hash::FxHashMap;

use pcg::{
    borrow_pcg::{
        GeneralizedLifetime,
        region_projection::{
            HasTy, LifetimeProjection, PcgLifetimeProjectionBase, PcgLifetimeProjectionBaseLike,
        },
    },
    r#loop::PlaceUsages,
    pcg::{CapabilityKind, EvalStmtPhase, PcgNode},
    results::PcgBasicBlock,
    utils::{
        HasCompilerCtxt, HasPlace, Place, PrefixRelation, maybe_old::MaybeLabelledPlace,
        maybe_remote::MaybeRemotePlace,
    },
};
use prusti_rustc_interface::middle::{mir, ty};

use task_encoder::{EncodeFullError, TaskEncoder};
use vir::Reify;

use crate::encoders::{
    EncodeResult, ImpureEncVisitor, TyUseImpureEnc,
    mir_shared::PureRvalueEnc,
    ty::{
        RustTyDecomposition,
        indirect::{IndirectPredicatesEnc, IndirectPredicatesEncOutputRef, PrustiPcgCtxt},
        use_impure::TyUseImpure,
    },
};

pub(crate) enum WandOldOuter<'vir> {
    LetBind(Vec<LetBind<'vir>>),
    Label(Option<&'vir str>),
}

impl<'vir: 'a, 'a, 'enc, E: TaskEncoder> ImpureEncVisitor<'vir, 'enc, E> {
    /// Calculate invariant at loop head, and the places whose permission it
    /// holds, which are the places the body invariants may read. A place
    /// behind a shared reference is held, and read, through the first shared
    /// reference on its path. `entry_label` labels the state in which the loop
    /// was entered.
    pub(crate) fn get_loop_inv(
        &mut self,
        cfpcs: &PcgBasicBlock<'_, 'vir>,
        loop_place_usages: &PlaceUsages<'vir>,
        entry_label: vir::CfgBlockLabelData<'vir>,
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
        // `reported` lists a place after the places it extends. Where the PCG unpacks a
        // reference at the head, the places behind it are encoded through the reference's
        // snapshot, so the invariant must hold the reference first.
        let mut reported = cfpcs.loop_invariant_place_capabilities(loop_place_usages, ctxt);
        // A place the loop writes has write capability in the loop, but the
        // PCG drops its value only right before the write, so at the loop head
        // it still has the capability it had before the loop. The invariant
        // uses that capability.
        for (place, capability) in reported.places.iter_mut() {
            if capability.is_write()
                && let Some(held) = state.place_capability(*place, self.pcg_ctxt())
            {
                if held
                    .partial_cmp(&CapabilityKind::Write)
                    .is_none_or(|order| order.is_lt())
                {
                    return Err(EncodeFullError::internal(
                        format!(
                            "{} is written in this loop but is only readable where the loop starts",
                            self.place_in_message(*place)
                        ),
                        vec![self.current_span()],
                    ));
                }
                *capability = held;
            }
        }

        // A place behind a shared reference has no predicate of its own: its value is
        // part of the snapshot of the first shared reference on its path from the local.
        // The invariant holds the predicate of that reference instead, only once even if
        // the loop reads several places behind it, and at the reference's own capability
        // if the PCG reports one for it (it is listed before the places behind it).
        let mut held_places: Vec<(Place<'vir>, CapabilityKind)> = Vec::new();
        for (&place, &capability) in &reported.places {
            if self.spec_blocks.spec_arms.is_spec_only_local(place.local) {
                continue;
            }
            if capability.is_write() {
                // A place without a value holds its `$uninit` token.
                inv.push(self.place_uninit(place)?);
                continue;
            }
            let place = place
                .iter_projections()
                .find_map(|(base, _)| {
                    let base = Place::from(base);
                    base.is_shared_ref(ctxt).then_some(base)
                })
                .unwrap_or(place);
            if !held_places.iter().any(|&(held, _)| held == place) {
                held_places.push((place, capability));
            }
        }

        // Viper checks the loop body in a state that holds only the invariant, so the
        // invariant keeps the value of a place the loop only reads by equating it with
        // its value when the loop was entered.
        let unchanged = |snap: vir::ExprSnap<'vir>| {
            self.vcx
                .mk_eq_expr(snap, self.vcx.mk_labelled_old_expr(snap, Some(entry_label)))
        };

        for &(held, capability) in &held_places {
            let place = held.with_inherent_region(ctxt);
            let (place_res, _snap, ty, ty_out) = self.encode_place_with_snap(place)?;
            let task = RustTyDecomposition::from_ty(ty.ty, self.def_id);
            let pred = ty_out.ref_to_pred(self.vcx, place_res.expr.expect_predicate(), None);
            inv.push(pred);
            // The pins and the indirect predicates are about the predicate the invariant
            // holds, not about a snapshot assembled from the place's parts, which
            // `encode_place_with_snap` may return.
            let snap = ty_out.ref_to_snap(place_res.expr.expect_predicate());
            if capability.is_read() {
                inv.push(unchanged(snap));
            }
            // If other held places lie behind `held` (the PCG unpacked a mutable reference
            // at the head), its indirect predicates would hold them a second time; they keep
            // their values as held places of their own.
            if held_places
                .iter()
                .any(|&(other, _)| PrefixRelation::is_strict_prefix_of(held, other))
            {
                continue;
            }
            // gets all the lifetime projections of the place
            // for example, if we add an invariant for x and x is a reference, then we also want to add
            // an invariant for *x
            let projs = place.generalized_lifetime_projections(ctxt);
            for proj in projs {
                let region = proj.region(ctxt);
                let indirect = self.indirect_predicates(task, region, ty.ty)?;
                // The referents of the lifetime projection keep their values as well if
                // the loop only reads them.
                let pinned = reported
                    .lifetime_projections
                    .get(&proj)
                    .is_some_and(|capability| capability.is_read());
                for predicate in &indirect.predicates {
                    inv.push(predicate.application.reify(self.vcx, snap));
                    if pinned {
                        inv.push(predicate.unchanged.reify(self.vcx, (snap, entry_label)));
                    }
                }
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

        Ok((inv, held_places.into_iter().collect()))
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
        let decomp = RustTyDecomposition::from_ty(ty.ty, self.def_id);
        let region = r.with_base(place).region(self.pcg_ctxt());
        let indirect = self.indirect_predicates(decomp, region, ty.ty)?;
        Ok(indirect
            .applications()
            .map(|expr| expr.reify(self.vcx, place_snap))
            .collect::<Vec<_>>())
    }

    /// Encodes the indirect predicates of the lifetime projection of `decomp`, the
    /// decomposition of `ty`, at `region`. If `region` is not among the lifetimes of
    /// `decomp` (e.g. because `ty` is an opaque type, a trait object or an unnormalized
    /// alias), nothing behind it is encoded and there are no indirect predicates. The
    /// exception is a type containing a closure, whose captures are encoded although the
    /// closure type has no capture regions; for it an unsupported feature is reported.
    fn indirect_predicates(
        &mut self,
        decomp: RustTyDecomposition<'vir>,
        region: GeneralizedLifetime<'vir>,
        ty: ty::Ty<'vir>,
    ) -> EncodeResult<'vir, IndirectPredicatesEncOutputRef<'vir>, E> {
        let Some(proj) = LifetimeProjection::new(decomp, region, None, PrustiPcgCtxt) else {
            if !ty
                .walk()
                .any(|arg| arg.as_type().is_some_and(|ty| ty.is_closure()))
            {
                return Ok(IndirectPredicatesEncOutputRef::default());
            }
            return Err(EncodeFullError::unsupported(
                format!("a closure capturing a reference (`{ty}`)"),
                vec![self.current_span()],
            ));
        };
        self.deps.require_dep::<IndirectPredicatesEnc>(proj)
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
