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
    r#loop::{LoopId, PlaceUsages},
    pcg::{CapabilityKind, EvalStmtPhase, PcgNode},
    results::PcgBasicBlock,
    utils::{
        HasCompilerCtxt, HasPlace, Place, SnapshotLocation, maybe_old::MaybeLabelledPlace,
        maybe_remote::MaybeRemotePlace,
    },
};
use prusti_rustc_interface::{index::IndexVec, middle::mir};

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
    /// reference on its path.
    pub(crate) fn get_loop_inv(
        &mut self,
        cfpcs: &PcgBasicBlock<'_, 'vir>,
        entry_graph: &BorrowsGraph<'vir>,
        loop_place_usages: &PlaceUsages<'vir>,
        (loop_id, loop_head): (LoopId, mir::BasicBlock),
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

        // The places held at the head: the used places and the places on the
        // way to them, e.g. the reference `w.f` of a used `*w.f`, that are
        // initialized leaves of the owned PCG, or, behind a reference, that a
        // deref edge of the head's borrow graph dereferences or that are
        // leaves of it. A place the loop writes is still held if it is
        // initialized at the head.
        let entry_leaves = entry_graph.frozen_graph().leaf_nodes(self.pcg_ctxt());
        let entry_derefs = entry_graph
            .edges()
            .filter_map(|edge| match edge.kind() {
                BorrowPcgEdgeKind::Deref(deref) => deref.blocked_place().as_current_place(),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut held = loop_invariant_place_capabilities
            .iter()
            .filter(|(_, capability)| !capability.is_write())
            .map(|(place, _)| *place)
            .collect::<Vec<_>>();
        for usage in loop_place_usages.iter() {
            let mut place = Some(*usage.0);
            while let Some(current) = place {
                place = current.last_projection().map(|(parent, _)| parent);
                if held.contains(&current) {
                    continue;
                }
                let local_state = &state.owned()[current.local];
                if !local_state.is_allocated() {
                    break;
                }
                let is_owned_leaf = local_state
                    .expansions()
                    .leaf_places(ctxt)
                    .into_iter()
                    .any(|leaf| Place::from(leaf) == current);
                let held_owned = is_owned_leaf
                    && state
                        .computed_owned_capability(current, self.pcg_ctxt())
                        .is_some_and(|capability| !capability.is_write());
                let held_borrowed = entry_derefs.contains(&current)
                    || entry_leaves.iter().any(|leaf| {
                        matches!(leaf, PcgNode::Place(place) if place.as_current_place() == Some(current))
                    });
                if held_owned || held_borrowed {
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
        for (place, capability) in loop_invariant_place_capabilities.iter() {
            if !capability.is_write() {
                held_places
                    .entry(behind_shared_ref(*place))
                    .or_insert(*capability);
            }
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
        // The loop havocs the snapshots of the held places, which the keys of
        // the indirect predicates are read from. A place with lifetimes that
        // the loop keeps (see `loop_keeps`) keeps its snapshot from before
        // the loop.
        let entry = self.loop_entry_label(loop_id, loop_head);
        for place in &held {
            let (place_res, snap, _, ty_out) = self.encode_place_with_snap(*place)?;
            inv.push(ty_out.ref_to_pred(self.vcx, place_res.expr.expect_predicate(), None));
            if let Some(entry) = entry
                && place
                    .ty(ctxt)
                    .ty
                    .walk()
                    .any(|arg| arg.as_region().is_some())
                && self.loop_keeps(loop_id, *place)
            {
                let before = self.vcx.maybe_apply_label(snap, Some(entry));
                inv.push(self.vcx.mk_eq_expr(snap, before));
            }
        }
        inv.extend(self.indirect_loop_invariant(&cfpcs.statements[0].entry_state)?);

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

    /// The label at the end of the block from which the loop `loop_id` with
    /// head `head` is entered, if it is entered from only one.
    fn loop_entry_label(
        &self,
        loop_id: LoopId,
        head: mir::BasicBlock,
    ) -> Option<vir::OldLabel<'vir>> {
        let [entry] = self.body.basic_blocks.predecessors()[head]
            .iter()
            .filter(|&&block| !self.loop_analysis().in_loop(block, loop_id))
            .collect::<Vec<_>>()[..]
        else {
            return None;
        };
        Some(self.get_location_label(SnapshotLocation::After(*entry)))
    }

    /// Whether the loop `loop_id` keeps the snapshot of `place`, which has no
    /// dereference of a reference: no statement in the loop mutates a place
    /// containing it or a part of it, except a part behind a reference, which
    /// the snapshot does not include.
    fn loop_keeps(&self, loop_id: LoopId, place: Place<'vir>) -> bool {
        use mir::visit::Visitor;
        struct Mutated<'vir>(Vec<mir::Place<'vir>>);
        impl<'vir> Visitor<'vir> for Mutated<'vir> {
            fn visit_place(
                &mut self,
                place: &mir::Place<'vir>,
                context: mir::visit::PlaceContext,
                _location: mir::Location,
            ) {
                if context.is_mutating_use() {
                    self.0.push(*place);
                }
            }
        }
        let tcx = self.vcx.tcx();
        // Whether `place` dereferences a reference or raw pointer at or after
        // its `from`-th projection.
        let behind_ref = |place: mir::Place<'vir>, from: usize| {
            place.iter_projections().skip(from).any(|(base, elem)| {
                matches!(elem, mir::ProjectionElem::Deref) && !base.ty(self.body, tcx).ty.is_box()
            })
        };
        let place: mir::Place<'vir> = mir::Place {
            local: place.local,
            projection: tcx.mk_place_elems(place.projection),
        };
        if behind_ref(place, 0) {
            return false;
        }
        let mut mutated = Mutated(Vec::new());
        for (block, data) in self.body.basic_blocks.iter_enumerated() {
            if self.loop_analysis().in_loop(block, loop_id) {
                mutated.visit_basic_block_data(block, data);
            }
        }
        !mutated.0.into_iter().any(|mutated| {
            let n = mutated.projection.len().min(place.projection.len());
            mutated.local == place.local
                && mutated.projection[..n] == place.projection[..n]
                && !behind_ref(mutated, n)
        })
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
