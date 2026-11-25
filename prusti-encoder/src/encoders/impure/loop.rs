use rustc_hash::FxHashMap;

use pcg::{
    borrow_pcg::{
        GeneralizedLifetime,
        edge::kind::BorrowPcgEdgeKind,
        graph::BorrowsGraph,
        region_projection::{
            ExtractRegionsCtxt, LifetimeProjection, PcgLifetimeProjectionBase,
            PcgLifetimeProjectionBaseLike,
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
use prusti_rustc_interface::index::IndexVec;

use task_encoder::TaskEncoder;

use crate::encoders::{
    EncodeResult, ImpureEncVisitor, TyUseImpureEnc,
    ty::{RustTyDecomposition, indirect_nodes::NodePredicates},
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
        entry_graph: &BorrowsGraph<'vir>,
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
        let loop_invariant_place_capabilities =
            cfpcs.loop_invariant_place_capabilities(loop_place_usages, ctxt);

        // Whether the head holds `place`: an initialized leaf of the owned PCG,
        // or, behind a reference, a place that a deref edge of the head's
        // borrow graph dereferences or a leaf of it. A place the loop writes is
        // still held if it is initialized at the head.
        let entry_leaves = entry_graph.frozen_graph().leaf_nodes(self.pcg_ctxt());
        let entry_derefs = entry_graph
            .edges()
            .filter_map(|edge| match edge.kind() {
                BorrowPcgEdgeKind::Deref(deref) => deref.blocked_place().as_current_place(),
                _ => None,
            })
            .collect::<Vec<_>>();
        let held_at_head = |place: Place<'vir>| {
            let local_state = &state.owned()[place.local];
            if !local_state.is_allocated() {
                return false;
            }
            let is_owned_leaf = local_state
                .expansions()
                .leaf_places(ctxt)
                .into_iter()
                .any(|leaf| Place::from(leaf) == place);
            let held_owned = is_owned_leaf
                && state
                    .computed_owned_capability(place, self.pcg_ctxt())
                    .is_some_and(|capability| !capability.is_write());
            held_owned
                || entry_derefs.contains(&place)
                || entry_leaves.iter().any(|leaf| {
                    matches!(leaf, PcgNode::Place(leaf) if leaf.as_current_place() == Some(place))
                })
        };
        // The places that the loop's usages need and the head holds: the
        // reported places, except those behind a mutable reference that the
        // head has not unpacked, whose permission the reference's indirect
        // predicate holds; and the places on the way to the used places, e.g.
        // the reference `w.f` of a used `*w.f`.
        let reported = loop_invariant_place_capabilities
            .places
            .iter()
            .map(|(place, capability)| (*place, *capability))
            .filter(|(place, capability)| !capability.is_write() && held_at_head(*place))
            .collect::<Vec<_>>();
        let mut held = reported.iter().map(|(place, _)| *place).collect::<Vec<_>>();
        for usage in loop_place_usages.iter() {
            let mut place = Some(*usage.0);
            while let Some(current) = place {
                place = current.last_projection().map(|(parent, _)| parent);
                if !held.contains(&current) && held_at_head(current) {
                    held.push(current);
                }
            }
        }
        // A place behind a shared reference has no predicate of its own: its value is
        // part of the snapshot of the first shared reference on its path from the local.
        // The invariant holds the predicate of that reference instead, only once even if
        // the loop reads several places behind it.
        let behind_shared_ref = |place: Place<'vir>| {
            place
                .iter_projections()
                .find_map(|(base, _)| {
                    let base = Place::from(base);
                    base.is_shared_ref(ctxt).then_some(base)
                })
                .unwrap_or(place)
        };
        let mut held_places = FxHashMap::default();
        for (place, capability) in &reported {
            held_places
                .entry(behind_shared_ref(*place))
                .or_insert(*capability);
        }
        let mut held =
            held.into_iter()
                .map(behind_shared_ref)
                .fold(Vec::new(), |mut held, place| {
                    if !held.contains(&place) {
                        held.push(place);
                    }
                    held
                });
        // A place's predicate is stated before those of the places behind it,
        // whose addresses are read from its snapshot.
        held.sort_by_key(|place| place.projection.len());
        // Viper checks the loop body in a state that holds only the invariant,
        // so the invariant keeps the value of a place the loop only reads by
        // equating it with its value when the loop was entered.
        for place in &held {
            let (place_res, _, _, ty_out) = self.encode_place_with_snap(*place)?;
            inv.push(ty_out.ref_to_pred(self.vcx, place_res.expr.expect_predicate(), None));
            if held_places
                .get(place)
                .is_some_and(|capability| capability.is_read())
            {
                // The snapshot of the predicate the invariant holds, not one
                // assembled from the place's parts.
                let snap = ty_out.ref_to_snap(place_res.expr.expect_predicate());
                let before = self.vcx.mk_labelled_old_expr(snap, Some(entry_label));
                inv.push(self.vcx.mk_eq_expr(snap, before));
            }
        }
        inv.extend(self.indirect_loop_invariant(&cfpcs.statements[0].entry_state)?);
        // The referents of a lifetime projection the loop only reads keep their
        // values as well: the place read through the predicates of its lifetime
        // is the same as when the loop was entered. The PCG reports such a
        // projection only for a place the loop does not write, whose snapshot,
        // the predicates' key, is kept above.
        for (proj, capability) in &loop_invariant_place_capabilities.lifetime_projections {
            let place = proj.base();
            // A place behind `place` that the head holds keeps its value as a
            // held place of its own; `place`'s predicates do not hold it then.
            if !capability.is_read()
                || !held.contains(&place)
                || held
                    .iter()
                    .any(|other| PrefixRelation::is_strict_prefix_of(place, *other))
                || place.is_shared_ref(ctxt)
                || place.projects_shared_ref(ctxt)
            {
                continue;
            }
            let (place_res, _, ty, ty_out) = self.encode_place_with_snap(place)?;
            let snap = ty_out.ref_to_snap(place_res.expr.expect_predicate());
            let nodes =
                NodePredicates::new(RustTyDecomposition::from_ty(ty.ty, self.def_id), self.deps)?
                    .in_ty(ty.ty);
            let lifetimes: IndexVec<_, GeneralizedLifetime<'vir>> =
                self.pcg_ctxt().extract_regions(ty.ty);
            let positions = nodes.positions_of(lifetimes[proj.region_idx()]);
            if positions.is_empty() {
                continue;
            }
            let read = nodes.deep_snap_at(snap, &positions);
            let before = self.vcx.mk_labelled_old_expr(read, Some(entry_label));
            inv.push(self.vcx.mk_eq_expr(read, before));
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

        Ok((inv, held_places))
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
        let place = match r.base().to_pcg_lifetime_projection_base() {
            PcgLifetimeProjectionBase::Place(p) => p,
            PcgLifetimeProjectionBase::Const(c) => todo!("{c:?}"),
        };
        let (_, snap, ty, _) = self.encode_place_with_snap(Self::get_place(place))?;
        let decomp = RustTyDecomposition::from_ty(ty.ty, self.def_id);
        let nodes = NodePredicates::new(decomp, self.deps).unwrap().in_ty(ty.ty);
        let lifetimes: IndexVec<_, GeneralizedLifetime<'vir>> =
            self.pcg_ctxt().extract_regions(ty.ty);
        let lifetime = lifetimes[r.region_idx()];
        // The keys extend the snapshot in the state the node refers to.
        Ok(nodes.of(lifetime, snap, |key| {
            self.configure_old(place, key, old_outer)
        }))
    }

    fn get_place(place: MaybeRemotePlace<'vir>) -> Place<'vir> {
        match place {
            MaybeRemotePlace::Local(MaybeLabelledPlace::Current(place)) => place,
            MaybeRemotePlace::Local(MaybeLabelledPlace::Labelled(place)) => place.place(),
            MaybeRemotePlace::Remote(r) => r.assigned_local().into(),
        }
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
