use pcg::borrow_pcg::{GeneralizedLifetime as PcgLifetime, region_projection::PcgRegion};
use prusti_rustc_interface::{
    index::{Idx, IndexVec},
    middle::ty,
};
use task_encoder::{EncodeFullError, TaskEncoder, TaskEncoderDependencies};
use vir::CastType;

use super::{
    RustTyDecomposition,
    generics::{
        GArgsTyEnc, GParams,
        lifetime_position::{
            EnclosureGraph, GeneralizedLifetime, LifetimePosition, LifetimePositionEnc,
        },
    },
    indirect_pred::{
        IndirectPredEnc, IndirectPredRef, LifetimePositionBlock, if_nonempty, interval_nonempty,
    },
};

/// The predicates of the nodes on places of a type: one `p_T_ind` per
/// occurrence of the node's generalized lifetime in the type.
#[derive(Debug, Clone)]
pub struct NodePredicates<'vir> {
    graph: EnclosureGraph<'vir>,
    /// The lifetime of each position, against which the PCG's lifetimes are
    /// matched.
    lifetimes: IndexVec<LifetimePosition, GeneralizedLifetime<'vir>>,
    context: GParams<'vir>,
    ind: IndirectPredRef<'vir>,
    tys: &'vir [vir::ExprTyVal<'vir>],
    consts: &'vir [vir::ExprCSnap<'vir>],
}

impl<'vir> NodePredicates<'vir> {
    pub fn new<E: TaskEncoder>(
        ty: RustTyDecomposition<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<Self, EncodeFullError<'vir, E>> {
        let args = deps.require_dep::<GArgsTyEnc>(ty.args)?;
        let graph = deps.require_dep::<LifetimePositionEnc>(ty)?;
        Ok(Self {
            lifetimes: graph.positions().map(|p| graph.lifetime(p)).collect(),
            graph,
            context: ty.args.context(),
            ind: deps.require_ref::<IndirectPredEnc>(ty.ty)?,
            tys: args.get_ty(),
            consts: args.get_const(),
        })
    }

    /// Matches the PCG's lifetimes against the regions of `ty`, the type the
    /// predicates are for, rather than those of its decomposition, which
    /// erases inference regions such as those of a body's types.
    pub fn in_ty(mut self, ty: ty::Ty<'vir>) -> Self {
        self.lifetimes = self
            .graph
            .positions()
            .map(|p| self.graph.lifetime_in(p, ty, self.context))
            .collect();
        self
    }

    /// All positions of the type, merged on the snapshot `snap`.
    fn block(&self, snap: vir::ExprSnap<'vir>) -> LifetimePositionBlock<'vir, '_> {
        LifetimePositionBlock {
            graph: &self.graph,
            first: LifetimePosition::new(0),
            end: LifetimePosition::new(self.graph.positions().count()),
            ind: self.ind,
            tys: self.tys,
            consts: self.consts,
            k: snap,
        }
    }

    /// The predicates of the node of `lifetime` on a place with snapshot
    /// `snap`, outermost first. `in_state` puts each key, which extends
    /// `snap`, in the state the node refers to.
    pub fn of(
        &self,
        lifetime: PcgLifetime<'vir>,
        snap: vir::ExprSnap<'vir>,
        mut in_state: impl FnMut(vir::ExprSnap<'vir>) -> vir::ExprSnap<'vir>,
    ) -> Vec<vir::ExprBool<'vir>> {
        let block = self.block(snap);
        block
            .positions()
            .into_iter()
            .filter(|&p| is_position_of(self.lifetimes[p], lifetime))
            .map(|p| {
                let (lo, hi) = self.graph.interval(p);
                if_nonempty(
                    block.nonempty(p),
                    block.predicate(in_state(block.key(p)), lo, hi),
                )
            })
            .collect()
    }

    /// The predicates of all positions on a place with snapshot `snap`,
    /// outermost first.
    pub fn all(&self, snap: vir::ExprSnap<'vir>) -> Vec<vir::ExprBool<'vir>> {
        let block = self.block(snap);
        block
            .positions()
            .into_iter()
            .map(|p| {
                let (lo, hi) = self.graph.interval(p);
                if_nonempty(block.nonempty(p), block.predicate(block.key(p), lo, hi))
            })
            .collect()
    }

    /// For a type parameter, the statements giving a copy of the value
    /// `snap` its predicates (see `IndirectPredRef::copy`); none for other
    /// types.
    pub fn copy(&self, snap: vir::ExprSnap<'vir>) -> Vec<vir::Stmt<'vir>> {
        let Some(copy) = self.ind.copy else {
            return Vec::new();
        };
        let block = self.block(snap);
        block
            .positions()
            .into_iter()
            .map(|p| {
                let (lo, hi) = self.graph.interval(p);
                copy(block.key(p), self.tys, self.consts, lo, hi).alloc()
            })
            .collect()
    }

    /// Whether the node of `lifetime` has any predicates.
    pub fn has_positions(&self, lifetime: PcgLifetime<'vir>) -> bool {
        self.graph
            .positions()
            .any(|p| is_position_of(self.lifetimes[p], lifetime))
    }

    /// The positions of the type, each after the positions enclosing it.
    pub fn positions(&self) -> Vec<LifetimePosition> {
        self.graph.positions_outermost_first().collect()
    }

    /// The positions of the node of `lifetime`, each after the positions
    /// enclosing it.
    pub fn positions_of(&self, lifetime: PcgLifetime<'vir>) -> Vec<LifetimePosition> {
        self.positions()
            .into_iter()
            .filter(|&p| is_position_of(self.lifetimes[p], lifetime))
            .collect()
    }

    /// The key of position `p` on a place with snapshot `snap`.
    pub fn key(&self, p: LifetimePosition, snap: vir::ExprSnap<'vir>) -> vir::ExprSnap<'vir> {
        self.block(snap).key(p)
    }

    /// The key of position `p` on a place with snapshot `snap`, with the data
    /// at the enclosing positions taken from `from`, another key of `p`.
    pub fn graft_enclosing(
        &self,
        p: LifetimePosition,
        snap: vir::ExprSnap<'vir>,
        from: vir::ExprSnap<'vir>,
    ) -> vir::ExprSnap<'vir> {
        self.block(snap).graft_enclosing(p, from)
    }

    /// `value` with every reference collapsed, the shallow snapshot.
    /// Whether the predicates have a body (see `IndirectPredRef::has_body`).
    pub fn has_body(&self) -> bool {
        self.ind.has_body
    }

    pub fn shallow(&self, value: vir::ExprSnap<'vir>) -> vir::ExprSnap<'vir> {
        if self.graph.is_empty() {
            return value;
        }
        self.ind.shallow.call()(value, self.tys, self.consts)
    }

    /// The key of position `p` on a place with the value `value`, which
    /// includes the referents: its shallow snapshot with the values at the
    /// positions enclosing `p` taken from `value`.
    pub fn key_of_value(
        &self,
        p: LifetimePosition,
        value: vir::ExprSnap<'vir>,
    ) -> vir::ExprSnap<'vir> {
        self.graft_enclosing(p, self.shallow(value), value)
    }

    /// The predicate of position `p` on a place with snapshot `snap`.
    pub fn predicate(
        &self,
        p: LifetimePosition,
        snap: vir::ExprSnap<'vir>,
    ) -> vir::PredicateApp<'vir> {
        self.predicate_with_key(p, self.key(p, snap))
    }

    /// The condition under which the predicate of position `p` holds
    /// anything, `None` where it always does (see
    /// [`interval_nonempty`](super::indirect_pred::interval_nonempty)).
    pub fn nonempty(&self, p: LifetimePosition) -> Option<vir::ExprBool<'vir>> {
        interval_nonempty(&self.graph, p)
    }

    /// The predicate of position `p` with key `key`.
    pub fn predicate_with_key(
        &self,
        p: LifetimePosition,
        key: vir::ExprSnap<'vir>,
    ) -> vir::PredicateApp<'vir> {
        let (lo, hi) = self.graph.interval(p);
        (self.ind.predicate)(key, self.tys, self.consts, lo, hi)(None)
    }

    /// For an array, the predicate of position `p` of the element `value` at
    /// `index`, which the array's predicate of `p` holds.
    pub fn element_predicate(
        &self,
        p: LifetimePosition,
        index: vir::ExprCSnap<'vir>,
        value: vir::ExprSnap<'vir>,
    ) -> Option<vir::PredicateApp<'vir>> {
        let (lo, hi) = self.graph.interval(p);
        let element = self.ind.element?;
        Some(element(
            index,
            value.downcast_ty(),
            self.tys,
            self.consts,
            lo,
            hi,
        )(None))
    }

    /// The positions of the generic argument `arg`, in the order of the
    /// argument type's own positions.
    pub fn arg_positions(&self, arg: usize) -> Vec<LifetimePosition> {
        self.graph.arg_positions(arg)
    }

    pub fn interval(&self, p: LifetimePosition) -> (vir::ExprInt<'vir>, vir::ExprInt<'vir>) {
        self.graph.interval(p)
    }

    pub fn deep_snap(&self, snap: vir::ExprSnap<'vir>) -> vir::ExprSnap<'vir> {
        let block = self.block(snap);
        block.fold(&block.positions())
    }

    /// `snap` extended through the predicates of `positions` and of the
    /// positions enclosing them, outermost first.
    pub fn deep_snap_at(
        &self,
        snap: vir::ExprSnap<'vir>,
        positions: &[LifetimePosition],
    ) -> vir::ExprSnap<'vir> {
        let block = self.block(snap);
        let included = block
            .positions()
            .into_iter()
            .filter(|p| {
                positions
                    .iter()
                    .any(|q| q == p || self.graph.ancestors(*q).contains(p))
            })
            .collect::<Vec<_>>();
        block.fold(&included)
    }

    /// `snap` extended through the predicates of its positions other than
    /// `excluded`, outermost first.
    pub fn deep_snap_except(
        &self,
        snap: vir::ExprSnap<'vir>,
        excluded: &[LifetimePosition],
    ) -> vir::ExprSnap<'vir> {
        let block = self.block(snap);
        let positions = block
            .positions()
            .into_iter()
            .filter(|p| !excluded.contains(p))
            .collect::<Vec<_>>();
        block.fold(&positions)
    }
}

/// Whether a position of `position`'s lifetime belongs to the node of the PCG
/// lifetime `lifetime`. Positions are matched by lifetime, not by the PCG's
/// lifetime index, which counts each lifetime once.
fn is_position_of<'tcx>(position: GeneralizedLifetime<'tcx>, lifetime: PcgLifetime<'tcx>) -> bool {
    match (position, lifetime) {
        (GeneralizedLifetime::Region(r), PcgLifetime::Region(l)) => PcgRegion::from(r) == l,
        // The decomposition of a type parameter or alias has it as its only
        // argument.
        (GeneralizedLifetime::RegionsIn(param), PcgLifetime::RegionsIn(opaque)) => {
            vir::with_vcx(|vcx| param.args.args()[0].expect_ty() == opaque.ty(vcx.tcx()))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use pcg::borrow_pcg::GeneralizedLifetime as PcgLifetime;
    use prusti_rustc_interface::{middle::ty, span::DUMMY_SP};
    use task_encoder::TaskEncoder;

    use super::NodePredicates;
    use crate::encoders::ty::{
        RustTyDecomposition,
        generics::{GArgsTyEnc, lifetime_position::LifetimePositionEnc},
        indirect_pred::IndirectPredEnc,
        pure::TyPureEnc,
        test_util::{simplified, with_type_alias},
    };

    /// Calls `f` with the node predicates of the type alias `X` defined in
    /// `src`, the generalized lifetime named `name` in `X`, which is a region
    /// or a type parameter, and the snapshot `x` of a place of type `X`.
    fn with_node<R: Send>(
        src: &str,
        name: &str,
        f: impl for<'vir> FnOnce(&NodePredicates<'vir>, PcgLifetime<'vir>, vir::ExprSnap<'vir>) -> R
        + Send,
    ) -> R {
        let name = name.to_string();
        with_type_alias(src, move |ty: RustTyDecomposition<'_>| {
            let (_, _, graph) = LifetimePositionEnc::encode(ty, true, DUMMY_SP)
                .unwrap()
                .unwrap();
            let (ind, _, _) = IndirectPredEnc::encode(ty.ty, true, DUMMY_SP)
                .unwrap()
                .unwrap();
            let (_, _, args) = GArgsTyEnc::encode(ty.args, true, DUMMY_SP)
                .unwrap()
                .unwrap();
            let (pure, _, _) = TyPureEnc::encode(ty.ty, true, DUMMY_SP).unwrap().unwrap();
            let nodes = NodePredicates {
                lifetimes: graph.positions().map(|p| graph.lifetime(p)).collect(),
                context: ty.args.context(),
                graph,
                ind,
                tys: args.get_ty(),
                consts: args.get_const(),
            };
            let lifetime = ty
                .args
                .args()
                .iter()
                .flat_map(|arg| arg.walk())
                .find_map(|arg| match arg.kind() {
                    ty::GenericArgKind::Lifetime(region) if region.to_string() == name => {
                        Some(PcgLifetime::Region(region.into()))
                    }
                    ty::GenericArgKind::Type(arg_ty) if arg_ty.to_string() == name => {
                        Some(PcgLifetime::RegionsIn(arg_ty.try_into().unwrap()))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("`X` has no lifetime or parameter `{name}`"));
            let x = vir::with_vcx(|vcx| vcx.mk_local_ex(vcx.mk_local_decl("x", pure.snapshot)));
            f(&nodes, lifetime, x)
        })
    }

    /// The predicates of the node of `name` on a place `x` of type `X`.
    fn node(src: &str, name: &str) -> Vec<String> {
        with_node(src, name, |nodes, lifetime, x| {
            nodes
                .of(lifetime, x, |key| key)
                .into_iter()
                .map(|pred| format!("{:?}", simplified(pred)))
                .collect()
        })
    }

    #[test]
    fn nested_mutable_references() {
        let src = "type X<'a, 'b> = &'a mut &'b mut i32;";
        assert_eq!(
            node(src, "'a"),
            ["p_Ref_mutable_ind(x, s_Ref_mutable_type(s_Int_i32_type()), 0, 1)"],
        );
        assert_eq!(
            node(src, "'b"),
            [
                "p_Ref_mutable_ind(extend_Ref_mutable(x, s_Ref_mutable_type(s_Int_i32_type()), 0, 1), s_Ref_mutable_type(s_Int_i32_type()), 1, 2)"
            ],
        );
    }

    #[test]
    fn repeated_lifetime() {
        let src = "struct Pair<'c, 'd>(&'c mut i32, &'d mut i32);
                   type X<'a, 'b> = (&'a mut i32, Pair<'a, 'b>);";
        assert_eq!(
            node(src, "'a"),
            [
                "p_2_Tuple_ind(x, s_Ref_mutable_type(s_Int_i32_type()), s_Pair_type(), 0, 1)",
                "p_2_Tuple_ind(x, s_Ref_mutable_type(s_Int_i32_type()), s_Pair_type(), 1, 2)",
            ],
        );
    }

    #[test]
    fn regions_in_type_parameter() {
        assert_eq!(
            node("type X<'a, T> = &'a mut T;", "T"),
            ["p_Ref_mutable_ind(extend_Ref_mutable(x, T$1, 0, 1), T$1, 1, (1) + (arity(T$1)))"],
        );
    }

    #[test]
    fn shared_reference() {
        assert_eq!(node("type X<'a, 'b> = &'a &'b mut i32;", "'a"), [""; 0]);
    }

    #[test]
    fn deep_snap() {
        let deep_snap = with_node(
            "type X<'a, 'b> = &'a mut &'b mut i32;",
            "'a",
            |nodes, _, x| format!("{:?}", simplified(nodes.deep_snap(x))),
        );
        assert_eq!(
            deep_snap,
            "extend_Ref_mutable(extend_Ref_mutable(x, s_Ref_mutable_type(s_Int_i32_type()), 0, 1), s_Ref_mutable_type(s_Int_i32_type()), 1, 2)",
        );
    }
}
