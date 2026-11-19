use itertools::Itertools;
use pcg::{
    PcgOutput,
    action::{BorrowPcgAction, PcgAction, PcgActions},
    borrow_pcg::{
        FunctionData, GeneralizedLifetime,
        action::BorrowPcgActionKind,
        borrow_pcg_edge::{BlockedNode, BorrowPcgEdge},
        borrow_pcg_expansion::BorrowPcgExpansion,
        edge::{
            abstraction::{AbstractionEdge, FunctionCallOrLoop},
            borrow_flow::BorrowFlowEdgeKind,
            kind::BorrowPcgEdgeKind,
        },
        edge_data::{EdgeData, NodeReplacement},
        graph::BorrowsGraph,
        region_projection::{
            ExtractRegionsCtxt, LifetimeProjection, LifetimeProjectionLabel,
            PcgLifetimeProjectionBase, PlaceOrConst,
        },
        state::BorrowsState,
        unblock_graph::{BorrowPcgUnblockAction, UnblockGraph},
        validity_conditions::BranchChoices,
    },
    coupling::PcgCoupledEdgeKind,
    free_pcs::{RepackGuide, RepackOp},
    r#loop::{LoopAnalysis, LoopId},
    pcg::{CapabilityKind, EvalStmtPhase, Pcg, PcgNode, PcgSuccessor},
    results::{PcgBasicBlock, PcgLocation},
    utils::{
        AnalysisLocation, CompilerCtxt, HasPlace, Place, PrefixRelation, SnapshotLocation,
        display::DisplayWithCtxt, maybe_old::MaybeLabelledPlace, maybe_remote::MaybeRemotePlace,
    },
};
use prusti_interface::PrustiError;
use prusti_rustc_interface::{
    abi,
    data_structures::graph::Successors,
    index::{Idx, IndexVec},
    middle::{
        mir,
        ty::{self, TyKind},
    },
    span::{Span, Spanned, def_id::DefId},
};
use rustc_hash::{FxHashMap, FxHashSet};
use task_encoder::{EncodeFullError, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, CompType, LocalDeclData};

use crate::encoders::{
    self, MirBuiltinUseCastEnc, MirBuiltinUseCastTask, MirPureEnc, MirPureEncTask, PrustiBuiltin,
    PureKind, TyUseImpureEnc, WandEnc, WandEncTask,
    impure::{WandOldOuter, assert_error_message},
    mir_fn::{CallTaskDescription, RustSignature, SpecBlockKind, SpecBlocks},
    mir_shared::{EncodeResult, PureRvalueEnc, RustcIntrinsic, unsupported_opaque},
    ty::{
        RustFieldAddress, RustTyDecomposition, RustTySpecial,
        generics::{GParams, lifetime_position::LifetimePosition},
        indirect_nodes::NodePredicates,
        use_impure::TyUseImpure,
        use_pure::{TyUsePure, TyUsePureEnc},
    },
};

use super::WandEncOutput;

#[derive(Clone, Copy)]
struct FromToVar<'vir> {
    decl: vir::LocalDeclBool<'vir>,
    expr: vir::ExprBool<'vir>,
}

impl<'vir> FromToVar<'vir> {
    fn new(vcx: &'vir vir::VirCtxt<'vir>, from: mir::BasicBlock, to: mir::BasicBlock) -> Self {
        let decl = vcx.mk_local_decl(
            vir::vir_format!(vcx, "_from_bb{}_to_bb{}", from.index(), to.index()),
            vir::TYPE_BOOL,
        );
        let expr = vcx.mk_local_ex(decl);
        Self { decl, expr }
    }
}

#[derive(Default)]
pub(crate) struct FromToVars<'vir>(FxHashMap<(mir::BasicBlock, mir::BasicBlock), FromToVar<'vir>>);

impl<'vir> FromToVars<'vir> {
    pub(crate) fn decls(&self) -> impl Iterator<Item = &'vir LocalDeclData<'vir, vir::Bool>> {
        self.0.values().map(|v| v.decl)
    }

    fn set_from_to_flag_stmt(
        &mut self,
        vcx: &'vir vir::VirCtxt<'vir>,
        from: mir::BasicBlock,
        to: mir::BasicBlock,
    ) -> vir::Stmt<'vir> {
        let var = self.get_or_create(vcx, from, to);
        vcx.mk_pure_assign_stmt(var.expr, vcx.mk_bool::<true>())
    }

    fn get_or_create(
        &mut self,
        vcx: &'vir vir::VirCtxt<'vir>,
        from: mir::BasicBlock,
        to: mir::BasicBlock,
    ) -> FromToVar<'vir> {
        *self
            .0
            .entry((from, to))
            .or_insert_with(|| FromToVar::new(vcx, from, to))
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum EdgeAction {
    Add,
    Remove,
}

impl EdgeAction {
    pub(crate) fn is_add(self) -> bool {
        matches!(self, EdgeAction::Add)
    }
    pub(crate) fn is_remove(self) -> bool {
        matches!(self, EdgeAction::Remove)
    }
}
pub(crate) enum FoldOrUnfold {
    Fold,
    Unfold,
}

impl FoldOrUnfold {
    pub(crate) fn for_action(action: EdgeAction) -> Self {
        match action {
            EdgeAction::Add => FoldOrUnfold::Unfold,
            EdgeAction::Remove => FoldOrUnfold::Fold,
        }
    }

    /// The operation for an edge that joins its sources into its target,
    /// rather than splitting its base into its targets.
    pub(crate) fn for_joining_action(action: EdgeAction) -> Self {
        match action {
            EdgeAction::Add => FoldOrUnfold::Fold,
            EdgeAction::Remove => FoldOrUnfold::Unfold,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum LocationLabelPrefix {
    Before,
    BeforeRefReassignment,
}

impl LocationLabelPrefix {
    pub(crate) fn to_str(self) -> &'static str {
        match self {
            LocationLabelPrefix::Before => "before",
            LocationLabelPrefix::BeforeRefReassignment => "before_ref_reassignment",
        }
    }
}

pub struct ImpureEncVisitor<'vir, 'enc, E: TaskEncoder>
where
    'vir: 'enc,
{
    pub vcx: &'vir vir::VirCtxt<'vir>,
    pub deps: &'enc mut TaskEncoderDependencies<'vir, E>,
    pub def_id: DefId,
    pub local_decls: &'enc mir::LocalDecls<'vir>,
    pub fpcs_analysis: PcgOutput<'enc, 'vir>,
    pub local_defs: crate::encoders::MirLocalDefEncOutput<'vir>,
    pub spec_blocks: SpecBlocks,
    pub body: &'enc mir::Body<'vir>,

    pub wands: WandEncOutput<'vir>,

    pub tmp_ctr: usize,
    pub label_ctr: usize,
    pub call_labels: FxHashMap<mir::BasicBlock, (&'vir str, &'vir str)>,
    /// Blocks whose call terminator created no wand to apply on expiry: it was
    /// not encoded as an impure method call (intrinsics, `prusti_contracts`
    /// builtins), or it calls the stub of a trait method.
    pub wandless_calls: FxHashSet<mir::BasicBlock>,
    pub from_to_vars: FromToVars<'vir>,

    // for the current basic block
    pub current_fpcs: Option<PcgBasicBlock<'enc, 'vir>>,
    pub current_block: Option<mir::BasicBlock>,
    pub current_block_pres: Option<Vec<usize>>,
    pub current_block_succs: Option<FxHashMap<mir::BasicBlock, vir::CfgBlockLabel<'vir>>>,
    pub current_block_label: Option<vir::CfgBlockLabel<'vir>>,
    pub current_stmts: Option<Vec<vir::Stmt<'vir>>>,
    pub current_terminator: Option<vir::TerminatorStmt<'vir>>,
    /// The keys of the lifetime-projection nodes whose predicates have been
    /// created so far (see `fold_or_unfold_node`).
    pub indirect_predicate_state: IndirectPredicateState<'vir>,
    /// The `FromExpansion` future edges from a part of their target, as
    /// (source, target): what a future node that becomes current consists of
    /// (see `track_future_node_renames`). Taken from the PCG at the entry of
    /// each block and updated by the block's actions.
    pub future_edges: Vec<(NodeLp<'vir>, NodeLp<'vir>)>,
    /// The statements that start the block being encoded: the copies of key
    /// variables at a join (see `IndirectPredicateState::enter_block`).
    pub block_entry_stmts: Vec<vir::Stmt<'vir>>,

    pub encoded_blocks: Vec<vir::CfgBlock<'vir>>, // TODO: use IndexVec ?
}

/// A lifetime-projection node of a local place.
pub(crate) type NodeLp<'vir> = LifetimeProjection<'vir, MaybeLabelledPlace<'vir>>;

/// The keys of one node's indirect predicates.
#[derive(Clone)]
pub(crate) struct NodeKeys<'vir> {
    snap: vir::ExprSnap<'vir>,
    keys: FxHashMap<LifetimePosition, vir::ExprSnap<'vir>>,
}

/// The encoder's state of the nodes' indirect predicates at the current
/// program point: their keys, and the folds deferred so far. Package scripts
/// use their own layer of keys on top.
#[derive(Default)]
pub(crate) struct IndirectPredicateState<'vir> {
    method: KeyLayer<'vir>,
    package: Option<KeyLayer<'vir>>,
    /// The number of key variables so far, to name new ones uniquely.
    var_count: usize,
    /// The nodes made current from future nodes whose predicates are not
    /// folded yet: their parts may still be lent when the node becomes
    /// current, so it is folded when it is used next.
    pub(crate) deferred_folds: FxHashSet<NodeLp<'vir>>,
    /// The method's state at the end of each encoded CFG edge.
    edge_ends: FxHashMap<(mir::BasicBlock, mir::BasicBlock), EdgeEnd<'vir>>,
    /// The method's state at the terminator, while an edge's actions are
    /// encoded.
    terminator: Option<EdgeEnd<'vir>>,
}

type KeyMap<'vir> = FxHashMap<NodeLp<'vir>, Option<NodeKeys<'vir>>>;

/// What starting a block found (see `IndirectPredicateState::enter_block`).
pub(crate) struct BlockEntry<'vir> {
    /// For each predecessor, the assignments `(new, old)` from its key
    /// variables into the block's.
    pub(crate) copies: Vec<(
        mir::BasicBlock,
        Vec<(vir::ExprSnap<'vir>, vir::ExprSnap<'vir>)>,
    )>,
    /// The nodes whose fold some predecessors defer and others do not.
    pub(crate) disagreeing: Vec<NodeLp<'vir>>,
}

/// The method's state at the end of a CFG edge.
#[derive(Clone)]
struct EdgeEnd<'vir> {
    keys: KeyMap<'vir>,
    deferred_folds: FxHashSet<NodeLp<'vir>>,
}

/// The keys of the nodes of a method body or a package script, and the
/// variables declared for them there.
#[derive(Default)]
struct KeyLayer<'vir> {
    /// `None` for a node whose keys were removed. In a package script, this
    /// hides the method's keys.
    keys: KeyMap<'vir>,
    decls: Vec<vir::LocalDecl<'vir, vir::Snap>>,
}

impl<'vir> NodeKeys<'vir> {
    /// Whether both are held by the same variables.
    fn same_vars(&self, other: &Self) -> bool {
        std::ptr::eq(self.snap, other.snap)
            && self.keys.len() == other.keys.len()
            && self
                .keys
                .iter()
                .all(|(p, key)| other.keys.get(p).is_some_and(|k| std::ptr::eq(*k, *key)))
    }
}

impl<'vir> IndirectPredicateState<'vir> {
    fn layer(&mut self) -> &mut KeyLayer<'vir> {
        self.package.as_mut().unwrap_or(&mut self.method)
    }

    pub(crate) fn get(&self, node: NodeLp<'vir>) -> Option<&NodeKeys<'vir>> {
        match self
            .package
            .as_ref()
            .and_then(|package| package.keys.get(&node))
        {
            Some(keys) => keys.as_ref(),
            None => self.method.keys.get(&node)?.as_ref(),
        }
    }

    /// The keys of `node` held by variables of the current layer: only these
    /// may be assigned there.
    pub(crate) fn get_in_layer(&mut self, node: NodeLp<'vir>) -> Option<&NodeKeys<'vir>> {
        self.layer().keys.get(&node)?.as_ref()
    }

    pub(crate) fn insert(&mut self, node: NodeLp<'vir>, keys: NodeKeys<'vir>) {
        self.layer().keys.insert(node, Some(keys));
    }

    pub(crate) fn remove(&mut self, node: NodeLp<'vir>) {
        self.layer().keys.insert(node, None);
    }

    /// A new key variable, declared in the current layer.
    pub(crate) fn new_var(
        &mut self,
        vcx: &'vir vir::VirCtxt<'vir>,
        prefix: &str,
        ty: vir::Type<'vir, vir::Snap>,
    ) -> vir::ExprSnap<'vir> {
        let name = vir::vir_format!(vcx, "{prefix}{}", self.var_count);
        self.var_count += 1;
        let local = vir::vir_local_decl! { vcx; [name] : [ty] };
        self.layer().decls.push(local);
        vcx.mk_local_ex(local)
    }

    fn edge_end(&self) -> EdgeEnd<'vir> {
        EdgeEnd {
            keys: self.method.keys.clone(),
            deferred_folds: self.deferred_folds.clone(),
        }
    }

    fn restore(&mut self, end: EdgeEnd<'vir>) {
        self.method.keys = end.keys;
        self.deferred_folds = end.deferred_folds;
    }

    /// Starts a block with its predecessors' state; the predecessors not
    /// encoded yet are back edges of loops. A node keeps the keys any of them
    /// holds: a node only some of them hold is only used on paths through
    /// these, as the PCG's edges of that node are conditioned on them. Where
    /// they hold a node's keys in different variables, the node gets new ones,
    /// which the returned copies assign from each predecessor's.
    pub(crate) fn enter_block(
        &mut self,
        vcx: &'vir vir::VirCtxt<'vir>,
        block: mir::BasicBlock,
        preds: impl IntoIterator<Item = mir::BasicBlock>,
    ) -> BlockEntry<'vir> {
        let ends = preds
            .into_iter()
            .filter_map(|pred| Some((pred, self.edge_ends.get(&(pred, block))?.clone())))
            .collect::<Vec<_>>();
        let mut nodes = Vec::new();
        for (_, end) in &ends {
            for (node, node_keys) in &end.keys {
                if node_keys.is_some() && !nodes.contains(node) {
                    nodes.push(*node);
                }
            }
        }
        let mut keys = KeyMap::default();
        let mut copies: Vec<(
            mir::BasicBlock,
            Vec<(vir::ExprSnap<'vir>, vir::ExprSnap<'vir>)>,
        )> = Vec::new();
        for node in nodes {
            let held = ends
                .iter()
                .filter_map(|(pred, end)| Some((*pred, end.keys.get(&node)?.as_ref()?)))
                .collect::<Vec<_>>();
            let first = held[0].1;
            if held.iter().all(|(_, node_keys)| node_keys.same_vars(first)) {
                keys.insert(node, Some(first.clone()));
                continue;
            }
            if held.iter().any(|(_, node_keys)| {
                node_keys.keys.len() != first.keys.len()
                    || node_keys.keys.keys().any(|p| !first.keys.contains_key(p))
            }) {
                continue;
            }
            let joined = NodeKeys {
                snap: self.new_var(vcx, "_ks", first.snap.ty()),
                keys: first
                    .keys
                    .iter()
                    .map(|(&p, key)| (p, self.new_var(vcx, "_k", key.ty())))
                    .collect(),
            };
            for &(pred, node_keys) in &held {
                let mut assigns = vec![(joined.snap, node_keys.snap)];
                assigns.extend(joined.keys.iter().map(|(p, &var)| (var, node_keys.keys[p])));
                match copies.iter_mut().find(|(other, _)| *other == pred) {
                    Some((_, existing)) => existing.extend(assigns),
                    None => copies.push((pred, assigns)),
                }
            }
            keys.insert(node, Some(joined));
        }
        let mut deferred = ends
            .first()
            .map(|(_, end)| end.deferred_folds.clone())
            .unwrap_or_default();
        let mut disagreeing = FxHashSet::default();
        for (_, end) in ends.iter().skip(1) {
            disagreeing.extend(deferred.symmetric_difference(&end.deferred_folds).copied());
            deferred.retain(|node| end.deferred_folds.contains(node));
        }
        self.restore(EdgeEnd {
            keys,
            deferred_folds: deferred,
        });
        BlockEntry {
            copies,
            disagreeing: disagreeing.into_iter().collect(),
        }
    }

    /// Ends a block: its state ends each edge to `succs` without own PCG
    /// actions.
    pub(crate) fn leave_block(
        &mut self,
        block: mir::BasicBlock,
        succs: impl IntoIterator<Item = mir::BasicBlock>,
    ) {
        for succ in succs {
            if !self.edge_ends.contains_key(&(block, succ)) {
                let end = self.edge_end();
                self.edge_ends.insert((block, succ), end);
            }
        }
    }

    /// Starts encoding the PCG actions of a terminator's edge.
    pub(crate) fn enter_edge(&mut self) {
        self.terminator = Some(self.edge_end());
    }

    /// Ends encoding the PCG actions of the edge from `from` to `to`: saves
    /// the state for `to`, and goes back to the state at the terminator.
    pub(crate) fn leave_edge(&mut self, from: mir::BasicBlock, to: mir::BasicBlock) {
        let end = self.edge_end();
        self.edge_ends.insert((from, to), end);
        let terminator = self.terminator.take().unwrap();
        self.restore(terminator);
    }

    /// Package scripts only remove edges, so they never contain a package.
    pub(crate) fn enter_package(&mut self) {
        assert!(self.package.is_none(), "nested package script");
        self.package = Some(KeyLayer::default());
    }

    /// Drops the package script's keys; returns its variables to declare.
    pub(crate) fn leave_package(&mut self) -> Vec<vir::LocalDecl<'vir, vir::Snap>> {
        self.package.take().unwrap().decls
    }

    /// The method body's variables to declare.
    pub(crate) fn method_decls(&self) -> &[vir::LocalDecl<'vir, vir::Snap>] {
        &self.method.decls
    }

    /// The nodes with keys in the method body.
    pub(crate) fn method_nodes(&self) -> impl Iterator<Item = NodeLp<'vir>> + '_ {
        self.method
            .keys
            .iter()
            .filter(|(_, keys)| keys.is_some())
            .map(|(node, _)| *node)
    }
}

/// A node that an edge splits a node into or joins into it, with the part of
/// the node's place it is.
#[derive(Clone, Copy)]
pub(crate) struct Child<'vir> {
    node: NodeLp<'vir>,
    part: Part,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Field(abi::FieldIdx),
    Variant,
    Referent,
}

impl<'vir> Child<'vir> {
    /// A node of a field, variant or referent of `base`'s place.
    fn of_place(base: NodeLp<'vir>, node: NodeLp<'vir>) -> Option<Self> {
        let base_place = base.base().place();
        let place = node.base().place();
        if place.local != base_place.local
            || place.projection.len() != base_place.projection.len() + 1
        {
            return None;
        }
        let part = match place.projection.last()? {
            mir::ProjectionElem::Field(field, _) => Part::Field(*field),
            mir::ProjectionElem::Downcast(..) => Part::Variant,
            mir::ProjectionElem::Deref => Part::Referent,
            _ => return None,
        };
        Some(Self { node, part })
    }
}

/// Whether a loop invariant states `node`'s predicates: a node with an old
/// label stands for a snapshot from before the loop, which the loop does not
/// maintain, so its predicates, if held, are framed around the loop.
fn maintained_by_loop(node: NodeLp<'_>) -> bool {
    node.base().is_current()
        && match node.label() {
            None => true,
            Some(LifetimeProjectionLabel::Location(location)) => {
                matches!(location, SnapshotLocation::Loop(_))
            }
            Some(LifetimeProjectionLabel::Future) => false,
        }
}

/// The node of a lifetime projection of a local place.
fn local_node<'vir>(
    projection: LifetimeProjection<'vir, PcgLifetimeProjectionBase<'vir>>,
) -> Option<NodeLp<'vir>> {
    match projection.base() {
        PlaceOrConst::Place(MaybeRemotePlace::Local(place)) => Some(projection.with_base(place)),
        _ => None,
    }
}

/// Represents the translation of a MIR place. If the place crosses a shared
/// reference, then we will no longer have a predicate for the `address` Ref,
/// but we do also have the snapshot available.
pub(crate) struct PlaceExpr<'vir> {
    address: vir::ExprRef<'vir>,
    metadata: Option<vir::ExprSnap<'vir>>,
    snap: Option<vir::ExprSnap<'vir>>,
}

impl<'vir> PlaceExpr<'vir> {
    /// Expects the encoded place to not be behind a shared ref
    pub(crate) fn expect_predicate(&self) -> vir::ExprRef<'vir> {
        assert!(self.snap.is_none());
        self.address
    }
}

pub(crate) struct EncodePlaceResult<'vir> {
    pub(crate) expr: PlaceExpr<'vir>,
    pub(crate) ty: mir::PlaceTy<'vir>,
}

macro_rules! comment {
    ($self:tt, $($arg:tt)*) => { $self.comment(
        vir::vir_format!($self.vcx, $($arg)*),
    ) };
}

/// Snapshots of the current statement's operands, captured between the
/// `PreOperands` and `PostOperands` repacks by
/// [ImpureEncVisitor::capture_operand_snaps] and passed (as the
/// [PureRvalueEnc::EncodePlaceCtxt]) only to the encoding of the statement's
/// effect; all other operand reads (terminators, repack guides) pass `None`.
type OperandSnaps<'vir> = Option<FxHashMap<mir::Place<'vir>, vir::ExprSnap<'vir>>>;

struct EncodedRvalue<'vir> {
    /// A snapshot of the rvalue. This snapshot is guaranteed to be well-formed
    /// in the state *before* the Rvalue has been assigned to a place. For
    /// example, in the statement `let rx = &mut x`, the snapshot encoding of
    /// `&mut x` relies on capabilities to `x` as they are *before* the
    /// assignment occurs. This is because the encoding of the assignment itself
    /// requires a snapshot of `&mut x`, and the snapshot constructor for
    /// &mut expects a snapshot of the borrowed place which can only be created
    /// when the capabilities are in the pre-assign state.
    expr: vir::ExprSnap<'vir>,

    /// Additional statements necessary to obtain the predicate for the assigned
    /// place of this Rvalue *after* the assignment. Such folds are necessary if
    /// make_generic / make_concrete type casts are necessary to move the capabilities
    /// from the state before the assignment to the predicate of the assigned place.
    ///
    /// For example, for the statement `let rx: &mut u32 = &mut x`, this should
    /// be called with the place corresponding to `rx`, and it will fold the
    /// type predicate for `rx` (in this case, this will make the `u32`
    /// permission held conceptually in *rx generic by calling
    /// `make_generic_u32(*rx)`
    ///
    /// Note that after executing these statements, the snapshot in `expr` is
    /// no-longer well-formed.
    post_assign_folds: Option<Box<dyn FnOnce(vir::ExprRef<'vir>) -> Vec<vir::Stmt<'vir>> + 'vir>>,
}

impl<'vir> EncodedRvalue<'vir> {
    fn post_fold_stmts(self, lhs_place: vir::ExprRef<'vir>) -> Vec<vir::Stmt<'vir>> {
        match self.post_assign_folds {
            Some(f) => f(lhs_place),
            None => Vec::new(),
        }
    }
}

impl<'vir> From<vir::ExprSnap<'vir>> for EncodedRvalue<'vir> {
    fn from(expr: vir::ExprSnap<'vir>) -> Self {
        Self {
            expr,
            post_assign_folds: None,
        }
    }
}

/// The resources that a package script takes from the state outside the
/// package (see `pcs_package_script`), collected while unblocking with
/// `actions`.
pub(crate) struct Footprint<'a, 'vir> {
    actions: &'a [BorrowPcgUnblockAction<'vir>],
    label: &'vir str,
    pub(crate) parts: Vec<vir::ExprBool<'vir>>,
}

impl<'vir, 'enc, E: TaskEncoder> ImpureEncVisitor<'vir, 'enc, E> {
    pub(crate) fn pcg_ctxt(&self) -> CompilerCtxt<'enc, 'vir> {
        self.fpcs_analysis.ctxt()
    }

    // TODO: make `pub(super)`
    pub(crate) fn stmt(&mut self, stmt: vir::Stmt<'vir>) {
        self.current_stmts.as_mut().unwrap().push(stmt);
    }

    fn stmts(&mut self, stmts: impl IntoIterator<Item = vir::Stmt<'vir>>) {
        for stmt in stmts {
            self.stmt(stmt);
        }
    }

    fn comment(&mut self, msg: &'vir str) {
        self.stmt(self.vcx.mk_comment_stmt(msg));
    }

    fn ty_use_impure(&mut self, ty: ty::Ty<'vir>) -> TyUseImpure<'vir> {
        let ty_task = RustTyDecomposition::from_ty(ty, self.def_id);
        self.deps.require_dep::<TyUseImpureEnc>(ty_task).unwrap()
    }

    fn encode_rvalue(
        &mut self,
        rvalue: &mir::Rvalue<'vir>,
        span: Span,
        operand_snaps: &OperandSnaps<'vir>,
    ) -> EncodeResult<'vir, EncodedRvalue<'vir>, E> {
        let rvalue_ty = rvalue.ty(self.local_decls, self.vcx.tcx());
        match rvalue {
            mir::Rvalue::Use(op, _) => Ok(self.encode_operand_snap(op, operand_snaps)?.into()),
            mir::Rvalue::Cast(cast_kind, operand, ty) => {
                assert_eq!(*ty, rvalue_ty);
                let (stmt, cast) =
                    self.encode_cast_snap(rvalue_ty, *cast_kind, operand, operand_snaps)?;
                self.stmts(stmt);
                Ok(cast.into())
            }
            mir::Rvalue::BinaryOp(op, box (l, r)) => Ok(self
                .encode_binop_snap(rvalue_ty, *op, l, r, operand_snaps, span)?
                .into()),

            mir::Rvalue::UnaryOp(unop, operand) => Ok(self
                .encode_unary_op_snap(rvalue_ty, *unop, operand, operand_snaps)?
                .into()),

            mir::Rvalue::Aggregate(box mir::AggregateKind::Array(..), elements) => Ok(self
                .encode_array_snap(rvalue_ty, elements, operand_snaps)?
                .into()),

            mir::Rvalue::Aggregate(
                box kind @ (mir::AggregateKind::Adt(..)
                | mir::AggregateKind::Tuple
                | mir::AggregateKind::Closure(..)),
                fields,
            ) => {
                // The indirect predicates of the fields are folded into those
                // of the new value by the PCG's aggregate edges.
                Ok(self
                    .encode_aggregate_snap(rvalue_ty, kind, fields, operand_snaps)?
                    .into())
            }

            mir::Rvalue::Repeat(operand, len) => Ok(self
                .encode_repeat_snap(rvalue_ty, operand, *len, operand_snaps)?
                .into()),

            mir::Rvalue::Discriminant(place) => {
                let e_rvalue_ty = self.ty_use_pure(rvalue_ty);
                let place_ty = place.ty(self.local_decls, self.vcx.tcx());
                let ty = self.ty_use_impure(place_ty.ty);
                if ty.specifics.is_opaque() {
                    return Err(unsupported_opaque(place_ty.ty, span));
                }
                let place_expr = self.encode_place(Place::from(*place))?.expr;

                Ok(match ty
                    .get_enumlike()
                    .filter(|_| place_ty.variant_index.is_none())
                {
                    Some(el) => {
                        if let Some(snap) = place_expr.snap {
                            let ty = self.ty_use_pure(place_ty.ty).expect_enumlike();
                            ty.snap_to_discr_snap(snap.downcast_ty())
                        } else {
                            let place_expr = place_expr.expect_predicate();
                            self.vcx.mk_unfolding_expr(
                                ty.ref_to_pred_app(place_expr, Some(self.vcx.mk_wildcard())),
                                el.discr_ty()
                                    .ref_to_snap(el.discr(place_expr))
                                    .downcast_ty(),
                            )
                        }
                    }
                    None => {
                        // mir::Rvalue::Discriminant documents "Returns zero for types without discriminant"
                        let zero = self.vcx.mk_uint::<0>();
                        e_rvalue_ty
                            .expect_primitive()
                            .prim_to_snap(zero.upcast_ty())
                    }
                }
                .upcast_ty()
                .into())
            }

            mir::Rvalue::Ref(_reg, _kind, place) => Ok(match rvalue_ty.kind() {
                TyKind::Ref(.., ty::Mutability::Not) => {
                    let (address, snap, _, _) = self.encode_place_with_snap((*place).into())?;
                    let metadata = address.expr.metadata;
                    let metadata =
                        metadata.unwrap_or_else(|| self.expect_thin_ptr_metadata(rvalue_ty));
                    let inner = self.ty_use_pure(rvalue_ty).expect_immref();
                    inner
                        .prim_to_snap(address.expr.address, metadata, snap)
                        .upcast_ty()
                        .into()
                }
                TyKind::Ref(.., ty::Mutability::Mut) => {
                    let p_rvalue_ty = self.ty_use_impure(rvalue_ty);
                    let place_expr = self.encode_place(Place::from(*place))?;

                    let metadata = place_expr.expr.metadata;
                    let metadata =
                        metadata.unwrap_or_else(|| self.expect_thin_ptr_metadata(rvalue_ty));
                    let inner = p_rvalue_ty.expect_mutref();
                    let place_ref = place_expr.expr.expect_predicate();
                    EncodedRvalue {
                        expr: inner.prim_to_snap_assign(place_ref, metadata).upcast_ty(),
                        post_assign_folds: Some(Box::new(move |lhs_place| {
                            p_rvalue_ty.fold(None, lhs_place, None, None, None)
                        })),
                    }
                }
                _ => unreachable!(),
            }),

            mir::Rvalue::RawPtr(_, place) => {
                let place_expr = self.encode_place(Place::from(*place))?;
                let metadata = place_expr.expr.metadata;
                let metadata = metadata.unwrap_or_else(|| self.expect_thin_ptr_metadata(rvalue_ty));
                let raw = self.ty_use_pure(rvalue_ty).expect_raw();
                Ok(raw
                    .prim_to_snap(place_expr.expr.address, metadata)
                    .upcast_ty()
                    .into())
            }

            _ => Err(EncodeFullError::unsupported(
                format!("unsupported rvalue {rvalue:?}"),
                vec![span],
            )),
        }
    }

    /// Do the same as [self.pcs_succ] but instead of adding the statements to [self.current_stmts] return them instead.
    /// TODO: clean this up
    fn collect_pcs_succ<'a>(
        &mut self,
        cfpcs: &PcgLocation<'_, 'vir>,
        pcs: &'a PcgSuccessor<'_, 'vir>,
    ) -> EncodeResult<'vir, Vec<vir::Stmt<'vir>>, E> {
        let current_stmts = self.current_stmts.take();
        self.current_stmts = Some(Vec::new());
        let res = self.pcs_succ(cfpcs, pcs);
        let new_stmts = self.current_stmts.take().unwrap();
        self.current_stmts = current_stmts;
        res.map(|()| new_stmts)
    }

    /// Emit the repacks of the terminator edge to `target` ([Self::pcs_succ]).
    /// The succ is found by block: the target is usually the terminator's
    /// only non-unwind successor, but for a ghost switch the (second)
    /// `ghost_erased` successor is skipped.
    fn pcs_succ_to(&mut self, target: mir::BasicBlock) -> EncodeResult<'vir, (), E> {
        let current_fpcs = self.current_fpcs.take().unwrap();
        let cfpcs = current_fpcs.statements.last().unwrap();
        let succ = current_fpcs
            .terminator
            .succs
            .iter()
            .find(|succ| succ.block() == target)
            .unwrap();
        let res = self.pcs_succ(cfpcs, succ);
        self.current_fpcs = Some(current_fpcs);
        res
    }

    /// [Self::collect_pcs_succ] for the `idx`-th successor: a `SwitchInt`
    /// edge, identified by index since several values may share a target.
    fn collect_pcs_succ_at(
        &mut self,
        idx: usize,
        target: mir::BasicBlock,
    ) -> EncodeResult<'vir, Vec<vir::Stmt<'vir>>, E> {
        let current_fpcs = self.current_fpcs.take().unwrap();
        let cfpcs = current_fpcs.statements.last().unwrap();
        let succ = &current_fpcs.terminator.succs[idx];
        assert_eq!(succ.block(), target);
        let res = self.collect_pcs_succ(cfpcs, succ);
        self.current_fpcs = Some(current_fpcs);
        res
    }

    pub(crate) fn block<Err>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<(), Err>,
    ) -> Result<Vec<vir::Stmt<'vir>>, Err> {
        let current_stmts = self.current_stmts.take();
        self.current_stmts = Some(Vec::new());
        f(self)?;
        let new_stmts = self.current_stmts.take().unwrap();
        self.current_stmts = current_stmts;
        Ok(new_stmts)
    }

    pub(crate) fn unfold(
        &mut self,
        base: MaybeLabelledPlace<'vir>,
        label: Option<&'vir str>,
    ) -> EncodeResult<'vir, (), E> {
        self.fold_or_unfold(base, FoldOrUnfold::Unfold, None, label)?;
        Ok(())
    }

    /// The index argument for folding or unfolding an array-typed place,
    /// if the expansion is guided by an indexing projection.
    fn expansion_index(&mut self, guide: RepackGuide) -> Option<vir::ExprCSnap<'vir>> {
        match guide {
            RepackGuide::Index(index_local, _) => {
                let mut index = self
                    .encode_operand_snap(&mir::Operand::Copy(index_local.local().into()), &None)
                    .unwrap();
                if let Some(label) = index_local.label() {
                    index = self.vcx.mk_old(index, self.get_location_label(label));
                }
                Some(index.downcast_ty())
            }
            RepackGuide::ConstantIndex(constant_index, _) => {
                // `from_end` indexes from the runtime length of a slice; it
                // is always false for an array, the only supported subject.
                let mir::PlaceElem::ConstantIndex {
                    offset,
                    from_end: false,
                    ..
                } = constant_index.into()
                else {
                    todo!("from-end `ConstantIndex` (into a slice)")
                };
                Some(self.usize_const_snap(offset))
            }
            _ => None,
        }
    }

    /// Returns what a fold consumes besides the places the predicate is
    /// expanded into (see `fold_frame`), `None` if the place is not
    /// (un)folded.
    pub(crate) fn fold_or_unfold(
        &mut self,
        base: MaybeLabelledPlace<'vir>,
        fold_or_unfold: FoldOrUnfold,
        expansion: Option<&BorrowPcgExpansion>,
        label: Option<&'vir str>,
    ) -> EncodeResult<'vir, Option<Vec<vir::ExprBool<'vir>>>, E> {
        let place = base.place();
        let label = if let MaybeLabelledPlace::Labelled(snap) = base {
            Some(self.get_location_label(snap.at()))
        } else {
            label.map(vir::OldLabel::Label)
        };

        // We don't want to unfold because for immutable refs we only use snapshot read/writes
        if place.is_shared_ref(self.pcg_ctxt()) || place.projects_shared_ref(self.pcg_ctxt()) {
            return Ok(None);
        }

        let ref_p = self.encode_place(place)?;

        let place_ty = ref_p.ty;
        let ref_p = self
            .vcx
            .maybe_apply_label(ref_p.expr.expect_predicate(), label);
        let data = self.ty_use_impure(place_ty.ty);
        if data.specifics.is_opaque() {
            return Err(unsupported_opaque(place_ty.ty, self.current_span()));
        }

        let index = expansion
            .and_then(|expansion| expansion.guide())
            .and_then(|guide| self.expansion_index(guide));

        let (stmts, frame) = match fold_or_unfold {
            FoldOrUnfold::Unfold => (
                data.unfold(place_ty.variant_index, ref_p, index, None, label),
                Vec::new(),
            ),
            FoldOrUnfold::Fold => (
                data.fold(place_ty.variant_index, ref_p, index, None, label),
                data.fold_frame(place_ty.variant_index, ref_p, index),
            ),
        };
        self.stmts(stmts);
        Ok(Some(frame))
    }

    fn branch_choices_cond(&mut self, choices: &BranchChoices) -> vir::ExprBool<'vir> {
        let successors = choices.successors(self.body);
        let from = choices.from();
        let conj = successors
            .iter()
            .map(|to| self.from_to_vars.get_or_create(self.vcx, from, *to).expr)
            .collect::<Vec<_>>();
        // Control flow must continue from `choices.from()` to any one of the `successors`
        self.vcx.mk_disj(self.vcx.alloc_slice(&conj))
    }

    /// The conjuncts of the condition under which `edge` is valid.
    fn edge_condition(&mut self, edge: &BorrowPcgEdge<'vir>) -> Vec<vir::ExprBool<'vir>> {
        let conditions = edge.conditions();

        // For each block `b` where the edge is only valid if control flow
        // continues from `b` to a specified subset of its successors, `cond`
        // contains the corresponding VIR expression.
        conditions
            .all_branch_choices()
            .map(|choices| self.branch_choices_cond(choices))
            .collect()
    }

    /// Handles the edge under its condition, also for the parts of the
    /// footprint it adds. A `footprint` is given in a package script.
    fn pcs_handle_edge(
        &mut self,
        borrows_state: &BorrowsState<'_, 'vir>,
        edge: &BorrowPcgEdge<'vir>,
        edge_action: EdgeAction,
        edge_to_loop: bool,
        to_skip: &mut Vec<mir::BasicBlock>,
        mut footprint: Option<&mut Footprint<'_, 'vir>>,
    ) -> EncodeResult<'vir, (), E> {
        let cond_conjuncts = self.edge_condition(edge);
        // For each block `b` where the edge validity depends on the successor taken from `b`,
        // every successor must be valid.
        let cond = self.vcx.mk_conj(self.vcx.alloc_slice(&cond_conjuncts));
        let parts = footprint
            .as_ref()
            .map_or(0, |footprint| footprint.parts.len());
        let stmts = self.block(|self_| {
            self_.pcs_handle_edge_conditionless(
                borrows_state,
                edge,
                edge_action,
                edge_to_loop,
                to_skip,
                footprint.as_deref_mut(),
            )
        })?;
        if let Some(footprint) = footprint
            && !cond_conjuncts.is_empty()
            && footprint.parts.len() > parts
        {
            let added = self.vcx.mk_conj(&footprint.parts.split_off(parts));
            let guarded = self
                .vcx
                .mk_bin_op_expr(vir::BinOpKind::Implies, cond, added);
            footprint.parts.push(guarded.downcast_ty());
        }
        if stmts.is_empty()
            || stmts
                .iter()
                .all(|stmt| matches!(stmt.kind, vir::StmtKindData::Comment(_)))
            || cond_conjuncts.is_empty()
        {
            self.stmts(stmts);
            return Ok(());
        }
        let stmts = self.vcx.alloc_slice(&stmts);
        self.stmt(self.vcx.mk_if_stmt(cond, stmts, &[]));
        Ok(())
    }

    fn expect_node_vars(&self, node: NodeLp<'vir>) -> EncodeResult<'vir, NodeKeys<'vir>, E> {
        self.indirect_predicate_state
            .get(node)
            .cloned()
            .ok_or_else(|| {
                self.unsupported(
                    format!(
                        "INTERNAL ERROR: no key for the predicates of {}",
                        node.to_short_string(self.pcg_ctxt())
                    ),
                    self.current_span(),
                )
            })
    }

    /// Assigns the keys of `node`'s predicates to its variables in the
    /// current layer.
    fn set_node_vars(
        &mut self,
        node: NodeLp<'vir>,
        snap: vir::ExprSnap<'vir>,
        keys: Vec<(LifetimePosition, vir::ExprSnap<'vir>)>,
    ) {
        let existing = self.indirect_predicate_state.get_in_layer(node).cloned();
        let (snap_var, mut key_vars) = match existing {
            Some(vars) => (vars.snap, vars.keys),
            None => (self.new_node_var("_ks", snap.ty()), FxHashMap::default()),
        };
        self.stmt(self.vcx.mk_pure_assign_stmt(snap_var, snap));
        for (p, key) in keys {
            let var = match key_vars.get(&p) {
                Some(var) => *var,
                None => {
                    let var = self.new_node_var("_k", key.ty());
                    key_vars.insert(p, var);
                    var
                }
            };
            self.stmt(self.vcx.mk_pure_assign_stmt(var, key));
        }
        self.indirect_predicate_state.insert(
            node,
            NodeKeys {
                snap: snap_var,
                keys: key_vars,
            },
        );
    }

    fn new_node_var(
        &mut self,
        prefix: &str,
        ty: vir::Type<'vir, vir::Snap>,
    ) -> vir::ExprSnap<'vir> {
        self.indirect_predicate_state.new_var(self.vcx, prefix, ty)
    }

    /// Moves the keys of renamed nodes to their new names. Any other keys
    /// under the old or new name are stale, so they are dropped.
    fn move_renamed_node_vars(&mut self, renamed: &[NodeReplacement<'vir>]) {
        let renamed = renamed
            .iter()
            .filter_map(|replacement| match (replacement.from(), replacement.to()) {
                (PcgNode::LifetimeProjection(from), PcgNode::LifetimeProjection(to)) => {
                    Some((local_node(from)?, local_node(to)?))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let moved = renamed
            .iter()
            .map(|&(from, to)| (to, self.indirect_predicate_state.get(from).cloned()))
            .collect::<Vec<_>>();
        for &(from, to) in &renamed {
            self.indirect_predicate_state.remove(from);
            self.indirect_predicate_state.remove(to);
        }
        for (to, vars) in moved {
            if let Some(vars) = vars {
                self.indirect_predicate_state.insert(to, vars);
            }
        }
    }

    /// Keys the predicates of the nodes of `locals`' places anew, by their
    /// snapshots extended in the current state, e.g. after a call that gave
    /// them back.
    fn rekey_nodes_of(&mut self, locals: &[mir::Local]) -> EncodeResult<'vir, (), E> {
        let nodes = self
            .indirect_predicate_state
            .method_nodes()
            .filter(|node| locals.contains(&node.base().place().local))
            .collect::<Vec<_>>();
        for node in nodes {
            let snap = self.indirect_predicate_state.get(node).unwrap().snap;
            let (_, predicates, positions) = self.node_positions(node)?;
            let keys = positions
                .iter()
                .map(|&p| (p, predicates.key(p, snap)))
                .collect();
            self.set_node_vars(node, snap, keys);
        }
        Ok(())
    }

    /// Applies a PCG action's renames to `future_edges` and `deferred_folds`.
    /// A node made current is folded from its `FromExpansion` sources at its next use.
    fn track_future_node_renames(
        &mut self,
        renamed: &[NodeReplacement<'vir>],
    ) -> EncodeResult<'vir, (), E> {
        let renamed = renamed
            .iter()
            .filter_map(|replacement| match (replacement.from(), replacement.to()) {
                (PcgNode::LifetimeProjection(from), PcgNode::LifetimeProjection(to)) => {
                    Some((local_node(from)?, local_node(to)?))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for &(from, to) in &renamed {
            if from.label() != Some(LifetimeProjectionLabel::Future) || to.label().is_some() {
                continue;
            }
            if self.future_edges.iter().any(|(_, target)| *target == from) {
                self.indirect_predicate_state.deferred_folds.insert(from);
            }
        }
        for edge in &mut self.future_edges {
            for &(from, to) in &renamed {
                if edge.0 == from {
                    edge.0 = to;
                }
                if edge.1 == from {
                    edge.1 = to;
                }
            }
        }
        for &(from, to) in &renamed {
            if self.indirect_predicate_state.deferred_folds.remove(&from) {
                self.indirect_predicate_state.deferred_folds.insert(to);
            }
        }
        Ok(())
    }

    /// Folds the predicates of `node` if it was made current from a future
    /// node and is used for the first time, from its `FromExpansion` edges'
    /// sources.
    fn fold_if_unfolded_current_node(&mut self, node: NodeLp<'vir>) -> EncodeResult<'vir, (), E> {
        if !self.indirect_predicate_state.deferred_folds.remove(&node) {
            return Ok(());
        }
        let children = self
            .future_edges
            .iter()
            .filter(|(_, target)| *target == node)
            .filter_map(|(source, _)| Child::of_place(node, *source))
            .collect::<Vec<_>>();
        self.fold_or_unfold_node(node, &children, FoldOrUnfold::Fold)
    }

    /// The borrow graph on entry to `block`, before the first statement's
    /// actions: every edge into the block carries it. `None` for the start
    /// block.
    pub(crate) fn block_entry_graph(
        &mut self,
        block: mir::BasicBlock,
    ) -> Option<BorrowsGraph<'vir>> {
        for &pred in &self.body.basic_blocks.predecessors()[block] {
            let Ok(Some(pred_pcg)) = self.fpcs_analysis.get_all_for_bb(pred) else {
                continue;
            };
            if let Some(succ) = pred_pcg
                .terminator
                .succs
                .iter()
                .find(|succ| succ.block() == block)
            {
                return Some(succ.entry_graph().clone());
            }
        }
        None
    }

    /// The `FromExpansion` future edges of `graph` whose source is a part of
    /// their target.
    fn future_edges_of(graph: &BorrowsGraph<'vir>) -> Vec<(NodeLp<'vir>, NodeLp<'vir>)> {
        graph
            .edges()
            .filter_map(|edge| match edge.kind() {
                BorrowPcgEdgeKind::BorrowFlow(borrow_flow)
                    if borrow_flow.kind().is_future_from_expansion() =>
                {
                    let target = borrow_flow.short();
                    let source = local_node(borrow_flow.long())?;
                    Child::of_place(target, source).map(|_| (source, target))
                }
                _ => None,
            })
            .collect()
    }

    /// The parts of a loop invariant for the indirect predicates at the loop
    /// head with borrow graph `graph`: the predicates of the leaf nodes, keyed by
    /// their key variables, and, for each node whose place is in `held`, that
    /// its keys agree with the place's current snapshot. The loop havocs these
    /// snapshots and the key variables its body assigns; the data behind
    /// references in a key need not be stated, since a fold rebuilds it from
    /// the children.
    pub(crate) fn indirect_loop_invariant(
        &mut self,
        graph: &BorrowsGraph<'vir>,
        held: &[Place<'vir>],
    ) -> EncodeResult<'vir, Vec<vir::ExprBool<'vir>>, E> {
        let mut inv = Vec::new();
        let leaves = graph.frozen_graph().leaf_nodes(self.pcg_ctxt());
        for leaf in leaves {
            let PcgNode::LifetimeProjection(node) = leaf else {
                continue;
            };
            let Some(node) = local_node(node.into()) else {
                continue;
            };
            let place = node.base().place();
            if !maintained_by_loop(node)
                || place.is_shared_ref(self.pcg_ctxt())
                || place.projects_shared_ref(self.pcg_ctxt())
            {
                continue;
            }
            let (_, nodes, positions) = self.node_positions(node)?;
            if positions.is_empty() {
                continue;
            }
            let keys = self.expect_node_vars(node)?;
            for p in positions {
                if let Some(&key) = keys.keys.get(&p) {
                    inv.push(
                        self.vcx
                            .mk_predicate_app_expr(nodes.predicate_with_key(p, key)),
                    );
                }
            }
        }
        let nodes_of_held = self
            .indirect_predicate_state
            .method_nodes()
            .filter(|&node| {
                maintained_by_loop(node)
                    && matches!(node.base(), MaybeLabelledPlace::Current(place) if held.contains(&place))
            })
            .collect::<Vec<_>>();
        for node in nodes_of_held {
            let keys = self.expect_node_vars(node)?;
            let (ty, nodes, _) = self.node_positions(node)?;
            let place = self
                .encode_place(node.base().place())?
                .expr
                .expect_predicate();
            let snap = self.ty_use_impure(ty).ref_to_snap(place);
            inv.push(self.vcx.mk_eq_expr(keys.snap, snap));
            for (&p, &key) in &keys.keys {
                inv.push(
                    self.vcx
                        .mk_eq_expr(key, nodes.graft_enclosing(p, snap, key)),
                );
            }
        }
        Ok(inv)
    }

    /// The type of `node`'s place, the predicates of its type, and the
    /// positions of `node`'s lifetime, outermost first.
    fn node_positions(
        &mut self,
        node: NodeLp<'vir>,
    ) -> EncodeResult<'vir, (ty::Ty<'vir>, NodePredicates<'vir>, Vec<LifetimePosition>), E> {
        let ty = self.encode_place(node.base().place())?.ty.ty;
        let nodes = NodePredicates::new(RustTyDecomposition::from_ty(ty, self.def_id), self.deps)?
            .in_ty(ty);
        let lifetimes: IndexVec<_, GeneralizedLifetime<'vir>> = self.pcg_ctxt().extract_regions(ty);
        let positions = nodes.positions_of(lifetimes[node.region_idx()]);
        Ok((ty, nodes, positions))
    }

    /// Assigns the keys of `node` from the current state, where its place's
    /// predicates are held.
    fn assign_current_node_vars(&mut self, node: NodeLp<'vir>) -> EncodeResult<'vir, (), E> {
        let (ty, nodes, positions) = self.node_positions(node)?;
        let place = self
            .encode_place(node.base().place())?
            .expr
            .expect_predicate();
        let snap = self.ty_use_impure(ty).ref_to_snap(place);
        let keys = positions.iter().map(|&p| (p, nodes.key(p, snap))).collect();
        self.set_node_vars(node, snap, keys);
        Ok(())
    }

    /// The snapshot of `child`: the part of `base`'s snapshot or keys it is.
    fn child_snapshot(
        &mut self,
        base: NodeLp<'vir>,
        base_ty: ty::Ty<'vir>,
        base_vars: &NodeKeys<'vir>,
        base_keys: &[(LifetimePosition, vir::ExprSnap<'vir>)],
        child: Child<'vir>,
    ) -> EncodeResult<'vir, vir::ExprSnap<'vir>, E> {
        match child.part {
            Part::Field(field) => {
                let variant = match base.base().place().projection.last() {
                    Some(mir::ProjectionElem::Downcast(_, variant)) => Some(*variant),
                    _ => None,
                };
                let pure = self.ty_use_pure(base_ty);
                Ok(pure.expect_variant_opt(variant).fields[field.index()]
                    .read(base_vars.snap.downcast_ty()))
            }
            Part::Variant => Ok(base_vars.snap),
            // The node of a referent's lifetime is one of the reference's
            // nested lifetimes, whose predicates hold the referent's.
            Part::Referent => {
                let Some(&(_, key)) = base_keys.first() else {
                    return Err(self.unsupported(
                        format!(
                            "INTERNAL ERROR: no key of {} holds its referent",
                            base.to_short_string(self.pcg_ctxt())
                        ),
                        self.current_span(),
                    ));
                };
                Ok(self
                    .ty_use_pure(base_ty)
                    .expect_mutref()
                    .value_access(key.downcast_ty()))
            }
        }
    }

    /// `whole` with `child`'s part replaced by `part`: the inverse of
    /// `child_snapshot`.
    fn with_child_part(
        &mut self,
        base: NodeLp<'vir>,
        base_ty: ty::Ty<'vir>,
        whole: vir::ExprSnap<'vir>,
        child: Child<'vir>,
        part: vir::ExprSnap<'vir>,
    ) -> vir::ExprSnap<'vir> {
        match child.part {
            Part::Field(field) => {
                let variant = match base.base().place().projection.last() {
                    Some(mir::ProjectionElem::Downcast(_, variant)) => Some(*variant),
                    _ => None,
                };
                self.ty_use_pure(base_ty)
                    .expect_variant_opt(variant)
                    .with_field(whole.downcast_ty(), field, part)
                    .upcast_ty()
            }
            Part::Variant => part,
            Part::Referent => self
                .ty_use_pure(base_ty)
                .expect_mutref()
                .with_value(whole.downcast_ty(), part)
                .upcast_ty(),
        }
    }

    /// The snapshot and keys of `base` with each child's part replaced by the
    /// child's current one. A child's key changes while `base` is unfolded
    /// when what encloses it changes, e.g. a callee redirects a reference.
    fn keys_from_children(
        &mut self,
        base: NodeLp<'vir>,
        base_ty: ty::Ty<'vir>,
        mut snap: vir::ExprSnap<'vir>,
        mut keys: Vec<(LifetimePosition, vir::ExprSnap<'vir>)>,
        children: &[Child<'vir>],
    ) -> EncodeResult<
        'vir,
        (
            vir::ExprSnap<'vir>,
            Vec<(LifetimePosition, vir::ExprSnap<'vir>)>,
        ),
        E,
    > {
        for &child in children {
            let Some(child_keys) = self.indirect_predicate_state.get(child.node).cloned() else {
                continue;
            };
            // As in `child_snapshot`, a referent's snapshot is not a part of
            // the reference's.
            if child.part != Part::Referent {
                snap = self.with_child_part(base, base_ty, snap, child, child_keys.snap);
            }
            let child_positions = if child.part == Part::Variant {
                keys.iter().map(|&(p, _)| p).collect()
            } else {
                self.node_positions(child.node)?.2
            };
            for (q, (_, key)) in child_positions.iter().zip(keys.iter_mut()) {
                if let Some(&child_key) = child_keys.keys.get(q) {
                    *key = self.with_child_part(base, base_ty, key, child, child_key);
                }
            }
        }
        Ok((snap, keys))
    }

    /// Cast the predicates of the generic fields or the generic referent of
    /// `base`'s place, held by `base`'s predicates, to concrete after they are
    /// unfolded, or to generic before they are folded. They are keyed by the
    /// keys of the `children`'s nodes; the fields of an enum's variant, whose
    /// nodes the variant's node splits into later, by those of their
    /// snapshots. For an enum, only the variant the value is in holds them.
    fn cast_children(
        &mut self,
        ty: ty::Ty<'vir>,
        snap: vir::ExprSnap<'vir>,
        keys: &[(LifetimePosition, vir::ExprSnap<'vir>)],
        nodes: &NodePredicates<'vir>,
        children: &[Child<'vir>],
        to_generic: bool,
    ) -> EncodeResult<'vir, (), E> {
        let data = self.ty_use_impure(ty);
        let child_keys = |visitor: &Self, part: Part| {
            children
                .iter()
                .find(|child| child.part == part)
                .and_then(|child| visitor.indirect_predicate_state.get(child.node))
                .map(|vars| {
                    let mut keys = vars.keys.iter().map(|(q, k)| (*q, *k)).collect::<Vec<_>>();
                    keys.sort_by_key(|(q, _)| q.index());
                    keys
                })
        };
        let order = |mut casts: Vec<vir::Stmt<'vir>>| {
            // Enclosed positions are cast to generic first, while the
            // enclosing ones are still concrete.
            if to_generic {
                casts.reverse();
            }
            casts
        };
        if let ty::Ref(_, _, mir::Mutability::Mut) = ty.kind() {
            let Some(referent) = data.expect_mutref().referent() else {
                return Ok(());
            };
            let Some(keys) = child_keys(self, Part::Referent) else {
                return Ok(());
            };
            let casts = keys
                .into_iter()
                .map(|(q, key)| referent.cast_at(key, q, to_generic))
                .collect();
            self.stmts(order(casts));
            return Ok(());
        }
        let pure = self.ty_use_pure(ty);
        if data.get_structlike().is_none() && data.get_enumlike().is_none() {
            return Ok(());
        }
        let snap = snap.downcast_ty();
        let variants = match pure.get_enumlike() {
            Some(enumlike) => enumlike
                .variants
                .iter()
                .enumerate()
                .map(|(v, variant)| {
                    let discr = enumlike.snap_to_discr_snap(snap);
                    (
                        Some(abi::VariantIdx::from_usize(v)),
                        Some(self.vcx.mk_eq_expr(discr, variant.data.discr)),
                    )
                })
                .collect(),
            None => vec![(None, None)],
        };
        for (variant, is_variant) in variants {
            let fields = data.expect_variant_opt(variant).fields.iter();
            let pure_fields = pure.expect_variant_opt(variant).fields.iter();
            let mut casts = Vec::new();
            for (idx, (field, pure_field)) in fields.zip(pure_fields).enumerate() {
                let Some(indirect) = field.indirect else {
                    continue;
                };
                let field_idx = abi::FieldIdx::from_usize(idx);
                let field_keys = match child_keys(self, Part::Field(field_idx)) {
                    Some(keys) => keys,
                    None => {
                        // The field's type is a type parameter, whose
                        // positions in `ty` are those of its argument.
                        let arg = match ty.kind() {
                            ty::Adt(adt, _) => {
                                let field_def = &adt
                                    .variant(variant.unwrap_or(abi::FIRST_VARIANT))
                                    .fields[field_idx];
                                let ty::Param(param) =
                                    self.vcx.tcx().type_of(field_def.did).skip_binder().kind()
                                else {
                                    unreachable!(
                                        "a field with indirect predicates has a type parameter's type"
                                    )
                                };
                                param.index as usize
                            }
                            // A tuple's arguments are its fields' types.
                            ty::Tuple(_) => idx,
                            other => unreachable!("fields of generic type in {other:?}"),
                        };
                        let field_ty =
                            mir::PlaceTy::field_ty(self.vcx.tcx(), ty, variant, field_idx)
                                .skip_normalization();
                        let field_nodes = NodePredicates::new(
                            RustTyDecomposition::from_ty(field_ty, self.def_id),
                            self.deps,
                        )?
                        .in_ty(field_ty);
                        let field_snap = pure_field.read(snap);
                        let arg_positions = nodes.arg_positions(arg);
                        keys.iter()
                            .filter_map(|&(p, _)| {
                                let q = arg_positions.iter().position(|&a| a == p)?;
                                let q = LifetimePosition::new(q);
                                Some((q, field_nodes.key(q, field_snap)))
                            })
                            .collect()
                    }
                };
                casts.extend(order(
                    field_keys
                        .into_iter()
                        .map(|(q, key)| indirect.cast_at(key, q, to_generic))
                        .collect(),
                ));
            }
            match is_variant {
                Some(is_variant) if !casts.is_empty() => {
                    let casts = self.vcx.alloc_slice(&casts);
                    self.stmt(self.vcx.mk_if_stmt(is_variant, casts, &[]));
                }
                _ => self.stmts(casts),
            }
        }
        Ok(())
    }

    /// Unfold the predicates of the node `base` out of its place, when an edge
    /// splits the node into the `children`'s nodes of its fields, variant or
    /// referent, or fold them back, when an edge joins those into it. Those
    /// of generic fields and referents are cast to concrete after the unfold
    /// and to generic before the fold. The unfold uses the keys `base`'s
    /// predicates were folded with, and gives the children their keys; the
    /// fold keys `base`'s predicates anew, by the contents they have now.
    fn fold_or_unfold_node(
        &mut self,
        base: NodeLp<'vir>,
        children: &[Child<'vir>],
        fold_or_unfold: FoldOrUnfold,
    ) -> EncodeResult<'vir, (), E> {
        if base.label() == Some(LifetimeProjectionLabel::Future) {
            return Err(self.unsupported(
                format!(
                    "INTERNAL ERROR: {} is folded or unfolded while it is a future node",
                    base.to_short_string(self.pcg_ctxt())
                ),
                self.current_span(),
            ));
        }
        let place = base.base();
        if place.place().is_shared_ref(self.pcg_ctxt())
            || place.place().projects_shared_ref(self.pcg_ctxt())
        {
            return Ok(());
        }
        // The node of an enum's variant holds the enum's predicates, which
        // the edge between the enum's node and the variant's moves.
        let is_variant = matches!(
            place.place().projection.last(),
            Some(mir::ProjectionElem::Downcast(..))
        );
        let (ty, nodes, positions) = self.node_positions(base)?;
        match fold_or_unfold {
            FoldOrUnfold::Unfold => {
                self.fold_if_unfolded_current_node(base)?;
                let vars = self.expect_node_vars(base)?;
                let keys = positions
                    .iter()
                    .map(|p| vars.keys.get(p).map(|key| (*p, *key)))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        self.unsupported(
                            format!(
                                "INTERNAL ERROR: no key for a position of {}",
                                base.to_short_string(self.pcg_ctxt())
                            ),
                            self.current_span(),
                        )
                    })?;
                if !is_variant {
                    // Innermost first: the keys read the enclosing predicates.
                    let unfolds = keys
                        .iter()
                        .rev()
                        .map(|&(p, key)| self.vcx.mk_unfold_stmt(nodes.predicate_with_key(p, key)))
                        .collect::<Vec<_>>();
                    self.stmts(unfolds);
                }
                for &child in children {
                    let child_snap = self.child_snapshot(base, ty, &vars, &keys, child)?;
                    // A child's predicate is keyed by its part of the base's
                    // key, as in the body of the base's predicate. The child's
                    // positions of the lifetime correspond to the base's, in
                    // order.
                    let child_keys = if child.part == Part::Variant {
                        keys.clone()
                    } else {
                        let (_, _, child_positions) = self.node_positions(child.node)?;
                        if child_positions.len() != keys.len() {
                            return Err(self.unsupported(
                                format!(
                                    "INTERNAL ERROR: the positions of {} do not match those of {}",
                                    child.node.to_short_string(self.pcg_ctxt()),
                                    base.to_short_string(self.pcg_ctxt())
                                ),
                                self.current_span(),
                            ));
                        }
                        let mut child_keys = Vec::new();
                        for (&q, &(p, key)) in child_positions.iter().zip(&keys) {
                            let part = NodeKeys {
                                snap: key,
                                keys: FxHashMap::default(),
                            };
                            let child_key =
                                self.child_snapshot(base, ty, &part, &[(p, key)], child)?;
                            child_keys.push((q, child_key));
                        }
                        child_keys
                    };
                    self.set_node_vars(child.node, child_snap, child_keys);
                }
                if !is_variant {
                    self.cast_children(ty, vars.snap, &keys, &nodes, children, false)?;
                }
            }
            FoldOrUnfold::Fold => {
                if is_variant {
                    return Ok(());
                }
                // A node's predicates are folded back with its keys rebuilt
                // from its children's; a node that a joining edge creates gets
                // keys from the current state, where its place is held.
                let vars = match self.indirect_predicate_state.get(base) {
                    Some(vars) => vars.clone(),
                    None if matches!(place, MaybeLabelledPlace::Current(_))
                        && base.label().is_none() =>
                    {
                        self.assign_current_node_vars(base)?;
                        self.expect_node_vars(base)?
                    }
                    None => self.expect_node_vars(base)?,
                };
                let stored = positions
                    .iter()
                    .filter_map(|p| vars.keys.get(p).map(|key| (*p, *key)))
                    .collect();
                let (snap, rebuilt) =
                    self.keys_from_children(base, ty, vars.snap, stored, children)?;
                self.set_node_vars(base, snap, rebuilt);
                let vars = self.expect_node_vars(base)?;
                let keys = positions
                    .iter()
                    .map(|p| vars.keys.get(p).map(|key| (*p, *key)))
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        self.unsupported(
                            format!(
                                "INTERNAL ERROR: no key for a position of {}",
                                base.to_short_string(self.pcg_ctxt())
                            ),
                            self.current_span(),
                        )
                    })?;
                self.cast_children(ty, vars.snap, &keys, &nodes, children, true)?;
                let folds = keys
                    .iter()
                    .map(|&(p, key)| self.vcx.mk_fold_stmt(nodes.predicate_with_key(p, key)))
                    .collect::<Vec<_>>();
                self.stmts(folds);
            }
        }
        Ok(())
    }

    fn pcs_handle_edge_conditionless(
        &mut self,
        borrows_state: &BorrowsState<'_, 'vir>,
        edge: &BorrowPcgEdge<'vir>,
        edge_action: EdgeAction,
        edge_to_loop: bool,
        to_skip: &mut Vec<mir::BasicBlock>,
        mut footprint: Option<&mut Footprint<'_, 'vir>>,
    ) -> EncodeResult<'vir, (), E> {
        // In a package script, current places are read in the state before the
        // package.
        let label = footprint.as_ref().map(|footprint| footprint.label);
        match edge.kind() {
            BorrowPcgEdgeKind::Borrow(borrow) if borrow.is_mut() && edge_action.is_remove() => {
                // For a borrow e.g. let x = &mut y; the capability to `y` is
                // folded into the Rvalue `&mut y` that is stored in `x`. This
                // reverses that effect
                self.unfold(borrow.assigned_ref(), label)?;
            }
            BorrowPcgEdgeKind::BorrowFlow(borrow_flow)
                if let BorrowFlowEdgeKind::Assignment(assignment_data) = borrow_flow.kind()
                    && let Some(mir::CastKind::PointerCoercion(
                        ty::adjustment::PointerCoercion::Unsize,
                        _,
                    )) = assignment_data.cast_kind()
                    && edge_action.is_remove() =>
            {
                let kind = assignment_data.cast_kind().unwrap();

                // For an unsize operation `let slice = &mut array;` the PCG
                // will keep track of the connection between the two places;
                // during the unsize operation we call a method to transter
                // permissions from one to the other, when the slice expires,
                // we need to undo the unsize operation.
                let long = borrow_flow.long();
                let short = borrow_flow.short();
                let PlaceOrConst::Place(src) = long.base() else {
                    unreachable!();
                };
                let src = src.as_local_place().unwrap();
                let dst = short.base();

                let ctxt = CompilerCtxt::new(self.body, self.vcx.tcx(), ());
                let src_ty = src.ty(ctxt).ty;
                let dst_ty = dst.ty(ctxt).ty;
                // An undo is only needed when processing coercions `&mut T -> &mut U` when T: Unsize<U>:
                // The mutable borrow ends and we need to return the permission to the original
                // `&mut T`. For every other CoerceUnsized destination (`&T`, raw pointers,
                // `Box<T>`, `Rc<T>`, `Arc<T>`, ...), there is no mutable borrow expiring and
                // nothing to undo. (For shared references the slice cannot have modified the
                // array it unsized from, so there is likewise nothing to undo.)
                if !matches!(dst_ty.kind(), ty::TyKind::Ref(_, _, ty::Mutability::Mut)) {
                    return Ok(());
                }
                // Since we only undo unsize coercions for &mut T -> &mut U when T: Unsize<U>,
                // we only consider the edge with idx=0 on both src and dst. This
                // corresponds to the lifetime projections of the mutable references.
                // The single undo accounts for the entire unsize coercion.
                // All other BorrowFlow(Unsize) edges in the PCG are bookkeeping, for example:
                //   - same-index (e.g. `&mut [&mut T; N] -> &mut [&mut T]`
                //     emits a redundant idx=1 -> idx=1 edge
                //   - cross-index (e.g. `&mut T -> &mut dyn Trait + 'b`
                //     emits an idx=0 -> idx=1 edge).
                if long.region_idx().index() != 0 || short.region_idx().index() != 0 {
                    return Ok(());
                }

                let rvalue_ty = RustTyDecomposition::from_ty(dst_ty, self.context());
                let operand_ty = RustTyDecomposition::from_ty(src_ty, self.context());
                let cast_output =
                    self.deps()
                        .require_dep::<MirBuiltinUseCastEnc>(MirBuiltinUseCastTask::new(
                            rvalue_ty, kind, operand_ty,
                        ))?;
                let src_place = src.place();
                let src_label = if let MaybeLabelledPlace::Labelled(snap) = src {
                    Some(self.get_location_label(snap.at()))
                } else {
                    label.map(vir::OldLabel::Label)
                };
                let src_snap = self.encode_place_with_snap(src_place)?.1.downcast_ty();
                let src_enc = self.vcx.maybe_apply_label(src_snap, src_label);
                let undo = cast_output.undo(src_enc);
                self.stmts(undo);
                return Ok(());
            }
            // An aggregate edge joins the nodes of the parts into the node of
            // the value, e.g. when the value is constructed or its place is
            // packed, and removing it splits them again.
            BorrowPcgEdgeKind::Aggregate(aggregate) => {
                let target = aggregate.target();
                let sources = aggregate
                    .sources()
                    .iter()
                    .filter_map(|source| Child::of_place(target, local_node(*source)?))
                    .collect::<Vec<_>>();
                self.fold_or_unfold_node(
                    target,
                    &sources,
                    FoldOrUnfold::for_joining_action(edge_action),
                )?;
            }
            // A future node is its labelled source once it becomes current, so
            // it shares the source's key variables.
            BorrowPcgEdgeKind::BorrowFlow(borrow_flow)
                if borrow_flow.kind().is_future_to_self() =>
            {
                if edge_action.is_add()
                    && let Some(source) = local_node(borrow_flow.long())
                    && let Some(keys) = self.indirect_predicate_state.get(source).cloned()
                {
                    self.indirect_predicate_state
                        .insert(borrow_flow.short(), keys);
                }
            }
            // A node made current from a future node consists of the nodes
            // of its `FromExpansion` edges (see `track_future_node_renames`);
            // removing such an edge splits it again.
            BorrowPcgEdgeKind::BorrowFlow(borrow_flow)
                if borrow_flow.kind().is_future_from_expansion() =>
            {
                let target = borrow_flow.short();
                // A borrower's edge, e.g. `y↓'a -> x↓'a FUTURE` for
                // `y = &mut x`, is no part of the target: what it holds is
                // given back to the target's labelled source.
                if let Some(source) = local_node(borrow_flow.long())
                    && Child::of_place(target, source).is_some()
                {
                    if edge_action.is_add() {
                        self.future_edges.push((source, target));
                    } else {
                        // The node is split into the sources of all its
                        // `FromExpansion` edges at the first one's removal,
                        // unless it was never folded from them.
                        if target.label() != Some(LifetimeProjectionLabel::Future)
                            && !self
                                .indirect_predicate_state
                                .deferred_folds
                                .contains(&target)
                        {
                            let children = self
                                .future_edges
                                .iter()
                                .filter(|(_, edge_target)| *edge_target == target)
                                .filter_map(|(source, _)| Child::of_place(target, *source))
                                .collect::<Vec<_>>();
                            self.fold_or_unfold_node(target, &children, FoldOrUnfold::Unfold)?;
                            self.indirect_predicate_state.deferred_folds.insert(target);
                        }
                        self.future_edges.retain(|edge| *edge != (source, target));
                        if !self
                            .future_edges
                            .iter()
                            .any(|(_, edge_target)| *edge_target == target)
                        {
                            self.indirect_predicate_state.deferred_folds.remove(&target);
                        }
                    }
                }
            }
            // An equals edge from a referent's node to a reborrow's node of the
            // same lifetime, e.g. `*x↓'b -> y↓'b` for `y = &mut *x`, moves the
            // referent's predicates into the reborrow's while it lives.
            BorrowPcgEdgeKind::BorrowFlow(borrow_flow)
                if matches!(
                    borrow_flow.kind(),
                    BorrowFlowEdgeKind::BorrowOutlives {
                        regions_equal: true
                    }
                ) =>
            {
                // The source is the referent of the target's place.
                let sources = local_node(borrow_flow.long())
                    .map(|node| Child {
                        node,
                        part: Part::Referent,
                    })
                    .into_iter()
                    .collect::<Vec<_>>();
                self.fold_or_unfold_node(
                    borrow_flow.short(),
                    &sources,
                    FoldOrUnfold::for_joining_action(edge_action),
                )?;
            }
            BorrowPcgEdgeKind::Deref(deref) => {
                let frame = self.fold_or_unfold(
                    deref.blocked_place(),
                    FoldOrUnfold::for_action(edge_action),
                    None,
                    label,
                )?;
                if let Some(footprint) = footprint
                    && let Some(frame) = frame
                {
                    self.fold_footprint(footprint, edge, frame)?;
                }
            }
            BorrowPcgEdgeKind::BorrowPcgExpansion(expansion)
                if let PcgNode::LifetimeProjection(base) = expansion.base() =>
            {
                let children = expansion
                    .expansion()
                    .into_iter()
                    .filter_map(|child| match child {
                        PcgNode::LifetimeProjection(child) => Child::of_place(base, child),
                        PcgNode::Place(_) => None,
                    })
                    .collect::<Vec<_>>();
                self.fold_or_unfold_node(base, &children, FoldOrUnfold::for_action(edge_action))?;
            }
            BorrowPcgEdgeKind::BorrowPcgExpansion(expansion)
                if let PcgNode::Place(base) = expansion.base() =>
            {
                let frame = self.fold_or_unfold(
                    base,
                    FoldOrUnfold::for_action(edge_action),
                    Some(expansion),
                    label,
                )?;
                if let Some(footprint) = footprint
                    && let Some(frame) = frame
                {
                    self.fold_footprint(footprint, edge, frame)?;
                }
            }
            BorrowPcgEdgeKind::Coupled(PcgCoupledEdgeKind(FunctionCallOrLoop::FunctionCall(
                call_edge,
            ))) => {
                if edge_action.is_add() {
                    // The wand will be introduced by the method call itself.
                    return Ok(());
                }
                let call = call_edge.metadata();
                // We may be encoding multiple edges as a single wand, skip
                // further edge removals. This is a hack to get around the fact
                // that Viper doesn't support hyperwands.
                if to_skip.contains(&call.location().block) {
                    return Ok(());
                }
                to_skip.push(call.location().block);
                // TODO: this applies *all* the wands for the referenced
                //   function call; instead we should figure out which
                //   wand it is based on the edge info.
                // TODO: closures
                let wands = self.deps.require_dep::<WandEnc>(WandEncTask {
                    data: call.function_data().unwrap(),
                })?;
                let bb = &self.body[call.location().block];
                let terminator = bb.terminator.as_ref().unwrap();
                match &terminator.kind {
                    mir::TerminatorKind::Call {
                        func,
                        args,
                        destination,
                        ..
                    } => {
                        // Some calls create no wands, e.g. a Viper function
                        // application (see `wandless_calls`).
                        if self.wandless_calls.contains(&call.location().block) {
                            return Ok(());
                        }
                        let (_, caller_substs) = self.get_call_data(func);

                        let (_, dest_snap, _, _) =
                            self.encode_place_with_snap((*destination).into())?;
                        let wand_args =
                            std::iter::once(Ok(dest_snap))
                                .chain(args.iter().map(|operand| {
                                    self.encode_operand_snap_immediate(&operand.node)
                                }))
                                .collect::<EncodeResult<'vir, Vec<_>, E>>()?;
                        let (label_pre, label_post) = self.call_labels[&call.location().block];
                        let call_ctx = self.gargs(caller_substs);
                        let wands =
                            wands.call_wands(&wand_args, label_pre, label_post, call_ctx, self);
                        for wand in wands {
                            if let Some(footprint) = &mut footprint {
                                footprint.parts.push(self.vcx.mk_wand_expr(wand));
                            }
                            self.stmt(self.vcx.mk_apply_stmt(wand));
                        }
                    }
                    _ => unreachable!(),
                }
            }
            BorrowPcgEdgeKind::Abstraction(at @ AbstractionEdge::Loop(_)) => {
                self.pcs_handle_wand(
                    borrows_state,
                    edge_action.is_add(),
                    &at.clone().into_singleton_coupled_edge(),
                    label,
                    edge_to_loop,
                    footprint,
                )?;
            }
            other => comment!(self, "(ignoring) {edge_action:?} {other:?}"),
        }
        comment!(
            self,
            "(PCG) handled edge {edge_action:?}: {}",
            edge.to_short_string(self.pcg_ctxt())
        );
        Ok(())
    }

    fn pcs_unblock_actions(
        &mut self,
        borrows_state: &BorrowsState<'_, 'vir>,
        footprint: &mut Footprint<'_, 'vir>,
    ) -> EncodeResult<'vir, (), E> {
        let mut to_skip = Vec::new();
        let actions = footprint.actions;
        for action in actions {
            self.pcs_handle_edge(
                borrows_state,
                action.edge(),
                EdgeAction::Remove,
                false,
                &mut to_skip,
                Some(footprint),
            )?;
        }
        Ok(())
    }

    /// The proof script of a package giving back the nodes of `rhs`, each with
    /// the resources it is given back as: unblocking the nodes, after asserting
    /// the footprint of the wand. Statements in the script only see resources
    /// in the package state, and a resource of the state outside is only moved
    /// there when consumed. Asserting the footprint first moves it there before
    /// any statement, which may need it earlier (e.g. to read the address of a
    /// `Box` that is folded).
    pub(crate) fn pcs_package_script<N: Into<BlockedNode<'vir>>>(
        &mut self,
        borrows_state: &BorrowsState<'_, 'vir>,
        rhs: Vec<(N, Vec<vir::ExprBool<'vir>>)>,
        label: &'vir str,
    ) -> EncodeResult<'vir, Vec<vir::Stmt<'vir>>, E> {
        let rhs = rhs
            .into_iter()
            .map(|(node, resources)| (node.into(), resources))
            .collect::<Vec<(BlockedNode<'vir>, _)>>();
        // The nodes are unblocked together: an edge blocking several of them,
        // e.g. an aggregate edge, is removed once.
        let ug = UnblockGraph::for_nodes(
            rhs.iter().map(|(node, _)| *node),
            borrows_state,
            self.pcg_ctxt(),
        );
        let actions = ug.actions(self.pcg_ctxt()).unwrap();
        let mut footprint = Footprint {
            actions: &actions,
            label,
            parts: Vec::new(),
        };
        for (node, resources) in rhs {
            // Where nothing unblocks the node, it is given back as it is.
            if !resources.is_empty() {
                let resources = self.vcx.mk_conj(&resources);
                let part = self.unless_unblocked(&footprint, node, None, resources);
                footprint.parts.extend(part);
            }
        }
        self.indirect_predicate_state.enter_package();
        let unblock =
            self.block(|visitor| visitor.pcs_unblock_actions(borrows_state, &mut footprint));
        let decls = self.indirect_predicate_state.leave_package();
        let unblock = unblock?;
        let mut script = decls
            .into_iter()
            .map(|decl| self.vcx.mk_local_decl_stmt(decl, None))
            .collect::<Vec<_>>();
        if !footprint.parts.is_empty() {
            script.push(self.vcx.mk_assert_stmt(self.vcx.mk_conj(&footprint.parts)));
        }
        script.extend(unblock);
        Ok(script)
    }

    /// Adds what folding along `edge` takes from the state outside the
    /// package: the `frame` of the fold (see `fold_or_unfold`), and the
    /// children of the edge, unless (or except under the conditions where) the
    /// footprint's actions unblock them.
    fn fold_footprint(
        &mut self,
        footprint: &mut Footprint<'_, 'vir>,
        edge: &BorrowPcgEdge<'vir>,
        frame: Vec<vir::ExprBool<'vir>>,
    ) -> EncodeResult<'vir, (), E> {
        footprint.parts.extend(frame);
        for child in edge.blocked_by_nodes(self.pcg_ctxt()) {
            if !matches!(child, PcgNode::Place(_)) {
                continue;
            }
            let mut resources = Vec::new();
            let mut old_outer = WandOldOuter::Label(Some(footprint.label));
            self.encode_pcg_node(&child, &mut resources, &mut old_outer)?;
            let resources = self.vcx.mk_conj(&resources);
            let part = self.unless_unblocked(footprint, child.into(), Some(edge), resources);
            footprint.parts.extend(part);
        }
        Ok(())
    }

    /// `resource` under the conditions where none of the footprint's actions
    /// unblock `node`, `None` if one always does (or does whenever `edge` is
    /// valid).
    fn unless_unblocked(
        &mut self,
        footprint: &Footprint<'_, 'vir>,
        node: BlockedNode<'vir>,
        edge: Option<&BorrowPcgEdge<'vir>>,
        resource: vir::ExprBool<'vir>,
    ) -> Option<vir::ExprBool<'vir>> {
        let ctxt = self.pcg_ctxt();
        let mut unblocked_under = Vec::new();
        for action in footprint.actions {
            let unblocking = action.edge();
            if !unblocking.blocks_node(node, ctxt) {
                continue;
            }
            let cond_conjuncts = self.edge_condition(unblocking);
            if cond_conjuncts.is_empty()
                || edge.is_some_and(|edge| unblocking.conditions() == edge.conditions())
            {
                return None;
            }
            unblocked_under.push(self.vcx.mk_conj(&cond_conjuncts));
        }
        if unblocked_under.is_empty() {
            return Some(resource);
        }
        let unblocked = self.vcx.mk_disj(&unblocked_under);
        let outside = self
            .vcx
            .mk_unary_op_expr(vir::UnOpKind::Not, unblocked.upcast_ty())
            .downcast_ty();
        Some(
            self.vcx
                .mk_bin_op_expr(vir::BinOpKind::Implies, outside, resource)
                .downcast_ty(),
        )
    }

    fn pcg_actions(
        &mut self,
        pcg: &Pcg<'_, 'vir>,
        actions: &PcgActions<'vir>,
        edge_to_loop: bool,
    ) -> EncodeResult<'vir, (), E> {
        for action in actions.iter() {
            match action {
                PcgAction::Borrow(action) => self.borrow_action(pcg, action, edge_to_loop)?,
                PcgAction::Owned(action) => self.pcg_repack(action.kind())?,
            }
        }
        Ok(())
    }

    /// Emit the repacks of `phase` at `location`.
    fn pcg_phase_actions(
        &mut self,
        location: mir::Location,
        phase: EvalStmtPhase,
    ) -> EncodeResult<'vir, (), E> {
        if phase == EvalStmtPhase::PostMain {
            let label = self.post_main_label(location, self.current_block_pres.as_ref().unwrap());
            self.stmt(self.vcx.mk_label_stmt(label));
        }
        let current_fpcs = self.current_fpcs.take().unwrap();
        let cfpcs = &current_fpcs.statements[location.statement_index];
        let result = self.pcg_actions(&cfpcs.states[phase], &cfpcs.actions(phase), false);
        self.current_fpcs = Some(current_fpcs);
        result
    }

    fn borrow_action(
        &mut self,
        pcg: &Pcg<'_, 'vir>,
        action: &BorrowPcgAction<'vir>,
        edge_to_loop: bool,
    ) -> EncodeResult<'vir, (), E> {
        let mut to_skip = Vec::new();
        let renamed = match action.kind() {
            BorrowPcgActionKind::LabelPlace(label) => label.renamed_nodes(),
            BorrowPcgActionKind::LabelLifetimeProjection(label) => label.renamed_nodes(),
            BorrowPcgActionKind::Restore(restore) => restore.renamed_nodes(),
            _ => &[],
        };
        self.move_renamed_node_vars(renamed);
        self.track_future_node_renames(renamed)?;
        match action.kind() {
            //Restore(RestoreCapability<'tcx>),
            //MakePlaceOld(Place<'tcx>),
            //SetLatest(Place<'tcx>, Location),
            //AddRegionProjectionMember(RegionProjectionMember<'tcx>, PathConditions),
            BorrowPcgActionKind::RemoveEdge(edge) => self.pcs_handle_edge(
                pcg.borrow_pcg(),
                edge,
                EdgeAction::Remove,
                edge_to_loop,
                &mut to_skip,
                None,
            ),
            BorrowPcgActionKind::AddEdge { edge } => self.pcs_handle_edge(
                pcg.borrow_pcg(),
                edge,
                EdgeAction::Add,
                edge_to_loop,
                &mut to_skip,
                None,
            ),
            BorrowPcgActionKind::Weaken(weaken)
                if matches!(
                    weaken.from_cap(),
                    CapabilityKind::Exclusive | CapabilityKind::ShallowExclusive
                ) && matches!(weaken.to_cap(), None | Some(CapabilityKind::Write)) =>
            {
                self.pcg_weaken(weaken.place(), weaken.is_for_storage_dead())
            }
            //RenamePlace {
            //    old: MaybeLabelledPlace<'tcx>,
            //    new: MaybeLabelledPlace<'tcx>,
            //},
            other => {
                comment!(self, "(ignoring) {other:?}");
                Ok(())
            }
        }
    }

    fn pcg_repack(&mut self, repack_op: &RepackOp<'vir>) -> EncodeResult<'vir, (), E> {
        comment!(self, "[PCG] {repack_op:?}");

        fn should_ignore(repack_op: &RepackOp<'_>) -> bool {
            match repack_op {
                RepackOp::RegainLoanedCapability(..) => true,
                RepackOp::Weaken(weaken) => {
                    weaken.from_cap().is_exclusive()
                        && matches!(
                            weaken.to_cap(),
                            CapabilityKind::Read | CapabilityKind::ShallowExclusive
                        )
                }
                RepackOp::StorageDead(..) => true,
                _ => false,
            }
        }

        match repack_op {
            RepackOp::Expand(_) | RepackOp::Collapse(_) => {
                let (place, capability_kind) = match repack_op {
                    RepackOp::Expand(expand) => (expand.from(), expand.capability()),
                    RepackOp::Collapse(collapse) => (collapse.to(), collapse.capability()),
                    _ => unreachable!(),
                };
                if matches!(capability_kind, CapabilityKind::Write) {
                    // Collapsing an already exhaled place is a no-op
                    // TODO: unless it's through a Ref I imagine?
                    //assert!(matches!(repack_op, RepackOp::Collapse(..)));
                    if !matches!(repack_op, RepackOp::Collapse(..)) {
                        comment!(self, "expected RepackOp::Collapse but got {repack_op:?}");
                    }
                    return Ok(());
                }
                let place_enc = self.encode_place(place)?;
                let place_ty = place_enc.ty;
                let place_enc = place_enc.expr.expect_predicate();
                let data = self.ty_use_impure(place_ty.ty);
                if data.specifics.is_opaque() {
                    return Err(unsupported_opaque(place_ty.ty, self.current_span()));
                }
                // TODO: guide should be implemented on `RepackOp`
                let guide = match repack_op {
                    pcg::free_pcs::RepackOp::Expand(expand) => expand.guide(),
                    pcg::free_pcs::RepackOp::Collapse(collapse) => collapse.guide(),
                    _ => None,
                };
                let index = guide.and_then(|guide| self.expansion_index(guide));
                if matches!(repack_op, pcg::free_pcs::RepackOp::Expand(..)) {
                    self.stmts(data.unfold(place_ty.variant_index, place_enc, index, None, None));
                } else if matches!(repack_op, pcg::free_pcs::RepackOp::Collapse(..)) {
                    self.stmts(data.fold(place_ty.variant_index, place_enc, index, None, None));
                } else {
                    unreachable!()
                }
                Ok(())
            }
            RepackOp::Weaken(weaken)
                if matches!(
                    weaken.from_cap(),
                    CapabilityKind::Exclusive | CapabilityKind::ShallowExclusive
                ) && weaken.to_cap().is_write() =>
            {
                self.pcg_weaken(weaken.place(), weaken.is_for_storage_dead())
            }
            other => {
                if should_ignore(other) {
                    self.stmt(self.vcx.mk_comment_stmt(vir::vir_format!(
                        self.vcx,
                        "ignored repack op: {other:?}"
                    )));
                    Ok(())
                } else {
                    Err(EncodeFullError::internal(
                        format!("unexpected PCG operation {other:?}"),
                        vec![self.current_span()],
                    ))
                }
            }
        }
    }

    fn pcg_weaken(
        &mut self,
        place: Place<'vir>,
        for_storage_dead: bool,
    ) -> EncodeResult<'vir, (), E> {
        let place_ty = place.ty(self.pcg_ctxt());
        assert!(place_ty.variant_index.is_none());

        // Skip the exhale for StorageDead-triggered weakens, since the place may
        // have already been moved/consumed and no longer hold permissions.
        // Temporary workaround until https://github.com/prusti/pcg/issues/137
        // is resolved.
        if for_storage_dead {
            comment!(
                self,
                "Weaken to Write for {:?} (skipped exhale: StorageDead)",
                place
            );
            return Ok(());
        }

        let place_ty_out = self.ty_use_impure(place_ty.ty);

        let place_enc = self.encode_place(place)?;
        comment!(self, "exhale due to weaken to Write");
        self.stmt(self.vcx.mk_exhale_stmt(place_ty_out.ref_to_pred(
            self.vcx,
            place_enc.expr.expect_predicate(),
            None,
        )));
        Ok(())
    }

    fn loop_analysis(&self) -> &LoopAnalysis {
        self.fpcs_analysis.analysis().loop_analysis()
    }

    fn loop_head_of(&mut self, block: mir::BasicBlock) -> Option<LoopId> {
        self.loop_analysis().loop_head_of(block)
    }

    fn pcs_succ<'a>(
        &mut self,
        cfpcs: &PcgLocation<'_, 'vir>,
        succ: &'a PcgSuccessor<'_, 'vir>,
    ) -> EncodeResult<'vir, (), E> {
        let post_main = &cfpcs.states[EvalStmtPhase::PostMain];
        let edge_to_loop = self.loop_head_of(succ.block()).is_some();
        self.indirect_predicate_state.enter_edge();
        let result = self.pcg_actions(post_main, succ.actions(), edge_to_loop);
        self.indirect_predicate_state
            .leave_edge(self.current_block.unwrap(), succ.block());
        result
    }

    fn finish_terminator(&mut self, location: mir::Location) -> EncodeResult<'vir, (), E> {
        comment!(self, "PCG (T) {}", EvalStmtPhase::PostMain);
        self.pcg_phase_actions(location, EvalStmtPhase::PostMain)?;
        let label = self.block_end_label(location.block, self.current_block_pres.as_ref().unwrap());
        self.stmt(self.vcx.mk_label_stmt(label));
        Ok(())
    }

    /// Ends the encoding of a terminator whose only (non-unwind) successor is
    /// `target`.
    fn goto_single_succ(
        &mut self,
        location: mir::Location,
        target: mir::BasicBlock,
    ) -> EncodeResult<'vir, vir::TerminatorStmt<'vir>, E> {
        self.finish_terminator(location)?;
        self.pcs_succ_to(target)?;
        let set_flag = self.set_from_to_flag(location.block, target);
        self.stmt(set_flag);
        Ok(self
            .vcx
            .mk_goto_stmt(self.current_block_succs.as_ref().unwrap()[&target]))
    }

    fn encode_operand(
        &mut self,
        operand: &mir::Operand<'vir>,
    ) -> EncodeResult<'vir, vir::ExprRef<'vir>, E> {
        let ty = operand.ty(self.local_decls, self.vcx.tcx());
        let (encode_place_result, ty_out) = match operand {
            &mir::Operand::Move(source) => {
                return Ok(self
                    .encode_place(Place::from(source))?
                    .expr
                    .expect_predicate());
            }
            &mir::Operand::Copy(_source) => {
                let ty_out = self.ty_use_impure(ty);
                (self.encode_operand_snap(operand, &None)?, ty_out)
            }
            mir::Operand::Constant(box constant) => {
                let ty_out = self.ty_use_impure(ty);
                let constant = self.encode_constant_snap(constant)?;
                (constant.upcast_ty(), ty_out)
            }
            mir::Operand::RuntimeChecks(_) => {
                unreachable!("RuntimeChecks is only produced by LowerIntrinsics")
            }
        };
        let tmp_exp: vir::ExprRef<'vir> = self.new_tmp(vir::TYPE_REF);
        self.stmt(ty_out.apply_method_assign(self.vcx, tmp_exp, encode_place_result));
        Ok(tmp_exp)
    }

    /// Encodes the snapshot of an operand. This should not be used for encoding
    /// regular mir statements/terminators as it doesn't match the semantics.
    fn encode_operand_snap_immediate(
        &mut self,
        operand: &mir::Operand<'vir>,
    ) -> EncodeResult<'vir, vir::ExprSnap<'vir>, E> {
        match operand {
            &mir::Operand::Move(source) | &mir::Operand::Copy(source) => {
                Ok(self.encode_place_with_snap(Place::from(source))?.1)
            }
            mir::Operand::Constant(box constant) => {
                Ok(self.encode_constant_snap(constant)?.upcast_ty())
            }
            mir::Operand::RuntimeChecks(_) => todo!(),
        }
    }

    /// Read `operand`'s snapshot into a fresh temporary at the current
    /// program point; a `Move` operand additionally consumes its place,
    /// exhaling the place's predicate.
    fn capture_operand_snap(
        &mut self,
        operand: &mir::Operand<'vir>,
    ) -> EncodeResult<'vir, vir::ExprSnap<'vir>, E> {
        match operand {
            &mir::Operand::Copy(place) | &mir::Operand::Move(place) => {
                let (result, snap_val, _, ty_out) =
                    self.encode_place_with_snap(Place::from(place))?;
                let tmp = self.new_tmp(ty_out.snapshot());
                self.stmt(self.vcx.mk_pure_assign_stmt(tmp, snap_val));
                if matches!(operand, mir::Operand::Move(_)) {
                    self.stmt(self.vcx.mk_exhale_stmt(ty_out.ref_to_pred(
                        self.vcx,
                        result.expr.expect_predicate(),
                        None,
                    )));
                }
                Ok(tmp)
            }
            mir::Operand::Constant(box constant) => {
                Ok(self.encode_constant_snap(constant)?.upcast_ty())
            }
            mir::Operand::RuntimeChecks(_) => todo!(),
        }
    }

    /// Read `statement`'s operands into an [OperandSnaps] map; see the call
    /// site in [Self::visit_statement].
    fn capture_operand_snaps(
        &mut self,
        statement: &mir::Statement<'vir>,
        location: mir::Location,
    ) -> EncodeResult<'vir, OperandSnaps<'vir>, E> {
        use mir::visit::Visitor;
        struct OperandCollector<'vir>(Vec<mir::Operand<'vir>>);
        impl<'vir> Visitor<'vir> for OperandCollector<'vir> {
            fn visit_operand(&mut self, operand: &mir::Operand<'vir>, _location: mir::Location) {
                if operand.place().is_some() {
                    self.0.push(operand.clone());
                }
            }
        }
        let mut collector = OperandCollector(Vec::new());
        collector.visit_statement(statement, location);
        let mut snaps = FxHashMap::default();
        for operand in &collector.0 {
            let place = operand.place().unwrap();
            if snaps.contains_key(&place) {
                continue;
            }
            let snap = self.capture_operand_snap(operand)?;
            snaps.insert(place, snap);
        }
        Ok(Some(snaps))
    }

    pub(crate) fn encode_place(
        &mut self,
        place: Place<'vir>,
    ) -> EncodeResult<'vir, EncodePlaceResult<'vir>, E> {
        let mut place_ty = mir::PlaceTy::from_ty(self.local_decls[place.local].ty);
        let mut result = PlaceExpr {
            address: self.local_defs[place.local].local_ex,
            metadata: None,
            snap: None,
        };
        // TODO: factor this out (duplication with pure encoder)?
        for (place, elem) in place.iter_projections() {
            result = self.encode_place_element(place.into(), elem, result)?;
            place_ty = place_ty.projection_ty(self.vcx.tcx(), elem);
        }
        Ok(EncodePlaceResult {
            expr: result,
            ty: place_ty,
        })
    }

    pub(crate) fn encode_place_with_snap(
        &mut self,
        place: Place<'vir>,
    ) -> EncodeResult<
        'vir,
        (
            EncodePlaceResult<'vir>,
            vir::ExprSnap<'vir>,
            mir::PlaceTy<'vir>,
            TyUseImpure<'vir>,
        ),
        E,
    > {
        let ty = (*place).ty(self.local_decls, self.vcx.tcx());
        assert!(ty.variant_index.is_none());

        let ty_out = self.ty_use_impure(ty.ty);
        let result = self.encode_place(place)?;
        let snap = result
            .expr
            .snap
            .unwrap_or_else(|| ty_out.ref_to_snap(result.expr.address));
        Ok((result, snap, ty, ty_out))
    }

    fn encode_place_element(
        &mut self,
        place: Place<'vir>,
        elem: mir::PlaceElem<'vir>,
        expr: PlaceExpr<'vir>,
    ) -> EncodeResult<'vir, PlaceExpr<'vir>, E> {
        let place_ty = place.ty(self.pcg_ctxt());
        Ok(match elem {
            mir::ProjectionElem::Field(field_idx, _) => {
                let e_ty = self.ty_use_impure(place_ty.ty);
                if e_ty.specifics.is_opaque() {
                    return Err(unsupported_opaque(place_ty.ty, self.current_span()));
                }
                let field_access = e_ty.expect_variant_opt(place_ty.variant_index);
                // Only the last field of a struct/tuple can be unsized (a DST's
                // tail); it then shares the containing value's pointer metadata.
                let is_last_field = field_idx.index() + 1 == field_access.fields.len();
                PlaceExpr {
                    address: field_access[field_idx].field_ref(expr.address),
                    metadata: if is_last_field { expr.metadata } else { None },
                    snap: expr.snap.map(|snap| {
                        let e_ty = self.ty_use_pure(place_ty.ty);
                        let field_access = e_ty.expect_variant_opt(place_ty.variant_index);
                        field_access[field_idx].read(snap.downcast_ty())
                    }),
                }
            }

            mir::ProjectionElem::Index(idx) => {
                let e_ty = self.ty_use_impure(place_ty.ty).expect_array();
                let idx = self
                    .encode_place_with_snap(mir::Place::from(idx).into())?
                    .1
                    .downcast_ty();
                PlaceExpr {
                    address: e_ty.ref_to_index_ref(expr.address, idx),
                    metadata: None,
                    snap: expr.snap.map(|snap| {
                        let e_ty = self.ty_use_pure(place_ty.ty).expect_array();
                        e_ty.index(snap.downcast_ty(), idx)
                    }),
                }
            }
            // `from_end` indexes from the runtime length of a slice; it is
            // always false for an array, the only supported subject.
            mir::ProjectionElem::ConstantIndex {
                offset,
                from_end: false,
                ..
            } => {
                let e_ty = self.ty_use_impure(place_ty.ty).expect_array();
                let idx = self.usize_const_snap(offset);
                PlaceExpr {
                    address: e_ty.ref_to_index_ref(expr.address, idx),
                    metadata: None,
                    snap: expr.snap.map(|snap| {
                        let e_ty = self.ty_use_pure(place_ty.ty).expect_array();
                        e_ty.index(snap.downcast_ty(), idx)
                    }),
                }
            }
            // TODO: should all variants start at the same `Ref`?
            mir::ProjectionElem::Downcast(..) => PlaceExpr {
                address: expr.address,
                metadata: None,
                snap: expr.snap,
            },
            mir::ProjectionElem::Deref => {
                assert!(place_ty.variant_index.is_none());
                let e_ty = self.ty_use_impure(place_ty.ty);
                match place_ty.ty.kind() {
                    ty::TyKind::Adt(adt, _) if adt.is_box() => match expr.snap {
                        // With a snapshot at hand (e.g. behind a shared
                        // reference, where no permission to the box's fields
                        // is held), everything is read out of it.
                        Some(snap) => {
                            let data = self.ty_use_pure(place_ty.ty).expect_structlike();
                            let snap = snap.downcast_ty();
                            PlaceExpr {
                                address: data.box_address_access(snap),
                                metadata: Some(data.box_metadata_access(snap)),
                                snap: Some(data.box_value_access(snap)),
                            }
                        }
                        // Otherwise the address and metadata are read out of
                        // the (folded) `Unique` field's predicate; the box
                        // itself is already unfolded here.
                        None => {
                            let data = e_ty.expect_structlike();
                            PlaceExpr {
                                address: data.box_address(expr.address),
                                metadata: Some(data.box_metadata(expr.address)),
                                snap: None,
                            }
                        }
                    },
                    ty::TyKind::Ref(_, _, ty::Mutability::Not) => {
                        let snap = expr
                            .snap
                            .unwrap_or_else(|| e_ty.ref_to_snap(expr.address))
                            .downcast_ty();
                        let p_ty = self.ty_use_pure(place_ty.ty).expect_immref();
                        PlaceExpr {
                            address: p_ty.addr_access(snap),
                            metadata: Some(p_ty.metadata_access(snap)),
                            snap: Some(p_ty.value_access(snap)),
                        }
                    }
                    ty::TyKind::Ref(_, _, ty::Mutability::Mut) => {
                        let ref_snap = e_ty.ref_to_snap(expr.address).downcast_ty();
                        let p_ty = self.ty_use_pure(place_ty.ty).expect_mutref();
                        PlaceExpr {
                            address: p_ty.deref_access(ref_snap),
                            metadata: Some(p_ty.metadata_access(ref_snap)),
                            snap: None,
                        }
                    }
                    ty::TyKind::RawPtr(..) => {
                        return Err(EncodeFullError::unsupported(
                            format!("dereference of the raw pointer `{}`", place_ty.ty),
                            vec![self.current_span()],
                        ));
                    }
                    _ => unreachable!(),
                }
            }
            _ => todo!("Unsupported ProjectionElem {:?}", elem),
        })
    }

    fn get_call_data(&self, func: &mir::Operand<'vir>) -> (DefId, ty::GenericArgsRef<'vir>) {
        let func_ty = func.ty(self.body, self.vcx.tcx());
        RustSignature::get_def_id_and_caller_substs(func_ty)
    }

    fn new_tmp<T: CompType>(&mut self, ty: vir::Type<'vir, T>) -> vir::Expr<'vir, T> {
        let name = vir::vir_format!(self.vcx, "_tmp{}", self.tmp_ctr);
        let local = vir::vir_local_decl! { self.vcx; [name] : [ty] };
        self.tmp_ctr += 1;
        self.stmt(self.vcx.mk_local_decl_stmt(local, None));
        self.vcx.mk_local_ex(local)
    }

    pub(crate) fn new_label(&mut self, base: &str) -> &'vir str {
        let name = vir::vir_format!(self.vcx, "{base}{}", self.label_ctr);
        self.label_ctr += 1;
        self.stmt(self.vcx.mk_label_stmt(name));
        name
    }

    pub(crate) fn get_location_label(&self, at: SnapshotLocation) -> vir::OldLabel<'vir> {
        let pres = self.labelled_block_pres(at.location().block);
        match at {
            SnapshotLocation::BeforeJoin(bb) | SnapshotLocation::Loop(bb) => {
                vir::OldLabel::Block(*self.vcx.mk_block_label(bb.as_usize(), pres))
            }
            SnapshotLocation::After(bb) => vir::OldLabel::Label(self.block_end_label(bb, &pres)),
            // The values only change at the statement's effect, so the
            // snapshots before the phases preceding it are those before the
            // statement.
            SnapshotLocation::Before(at)
                if at == AnalysisLocation::new(at.location(), EvalStmtPhase::PostMain) =>
            {
                vir::OldLabel::Label(self.post_main_label(at.location(), &pres))
            }
            SnapshotLocation::Before(at) => vir::OldLabel::Label(self.location_label(
                LocationLabelPrefix::Before,
                at.location(),
                &pres,
            )),
            SnapshotLocation::BeforeRefReassignment(location) => vir::OldLabel::Label(
                self.location_label(LocationLabelPrefix::BeforeRefReassignment, location, &pres),
            ),
        }
    }

    /// The pre-loop prefixes of the copy of `block` that precedes the current
    /// block (see the comment in [Self::visit_body]): the current block's
    /// prefixes, restricted to the loops that contain `block`. A loop that
    /// contains `block` but not the current block has been exited, and its
    /// last iteration is taken to be the one after the loop head was hit.
    fn labelled_block_pres(&self, block: mir::BasicBlock) -> Vec<usize> {
        let Some(current_pres) = self.current_block_pres.as_ref() else {
            return Vec::new();
        };
        let block_loops = self
            .loop_analysis()
            .loops(block)
            .map(|l| l.index())
            .collect::<FxHashSet<_>>();
        current_pres
            .iter()
            .copied()
            .filter(|l| block_loops.contains(l))
            .collect()
    }

    /// Creates a label for the state after executing all statements and the
    /// terminator of the block, but BEFORE any PCG operations for the
    /// terminator edge are applied (i.e. for the join into the target block).
    /// In particular, places that will become inaccessible in the target block due to
    /// conditional moves are still accessible at the point where this label is inserted.
    /// `loop_pres` is used to generate a unique label when `block` is encoded multiple times
    /// (see the comment in [Self::visit_body]).
    fn block_end_label(&self, block: mir::BasicBlock, loop_pres: &[usize]) -> &'vir str {
        let pres = Self::loop_pres_suffix(loop_pres);
        vir::vir_format!(self.vcx, "_after_{}{pres}", block.index())
    }

    fn loop_pres_suffix(loop_pres: &[usize]) -> String {
        loop_pres.iter().map(|l| format!("_pre{l}")).collect()
    }

    pub(crate) fn location_label(
        &self,
        prefix: LocationLabelPrefix,
        location: mir::Location,
        loop_pres: &[usize],
    ) -> &'vir str {
        let pres = Self::loop_pres_suffix(loop_pres);
        vir::vir_format!(
            self.vcx,
            "_{}_{}{pres}_{}",
            prefix.to_str(),
            location.block.index(),
            location.statement_index
        )
    }

    /// The label after the effect of the statement at `location`, before its
    /// `PostMain` actions.
    fn post_main_label(&self, location: mir::Location, loop_pres: &[usize]) -> &'vir str {
        vir::vir_format!(
            self.vcx,
            "{}_post_main",
            self.location_label(LocationLabelPrefix::Before, location, loop_pres)
        )
    }

    fn new_before_label(&mut self, location: mir::Location) {
        let label = self.location_label(
            LocationLabelPrefix::Before,
            location,
            self.current_block_pres.as_ref().unwrap(),
        );
        self.stmt(self.vcx.mk_label_stmt(label));
    }

    fn set_from_to_flag(&mut self, from: mir::BasicBlock, to: mir::BasicBlock) -> vir::Stmt<'vir> {
        self.from_to_vars.set_from_to_flag_stmt(self.vcx, from, to)
    }

    pub fn visit_body(&mut self, body: &mir::Body<'vir>) -> EncodeResult<'vir, (), E> {
        /// A work-queue item, min-ordered by the block's reverse-postorder
        /// index (ties broken by insertion order): every non-back-edge
        /// predecessor of a block is then encoded before the block itself,
        /// in particular both branches before their join. The borrow-expiry
        /// handling relies on this: a call's `call_labels`/`wandless_calls`
        /// entry must exist by the time an expiring borrow references the
        /// call's block. `heads_hit` is payload, not part of the order.
        struct WorkItem {
            rpo_idx: usize,
            seq: usize,
            block: mir::BasicBlock,
            heads_hit: FxHashSet<LoopId>,
        }
        impl PartialEq for WorkItem {
            fn eq(&self, other: &Self) -> bool {
                (self.rpo_idx, self.seq) == (other.rpo_idx, other.seq)
            }
        }
        impl Eq for WorkItem {}
        impl PartialOrd for WorkItem {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }
        impl Ord for WorkItem {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                // Reversed so that the max-heap `BinaryHeap` pops the minimum.
                (other.rpo_idx, other.seq).cmp(&(self.rpo_idx, self.seq))
            }
        }

        let rpo_idx: FxHashMap<mir::BasicBlock, usize> = body
            .basic_blocks
            .reverse_postorder()
            .iter()
            .enumerate()
            .map(|(idx, block)| (*block, idx))
            .collect();
        let mut next_seq = 0..;
        let mut queue = std::collections::BinaryHeap::new();
        let mut visited = FxHashSet::default();

        // In Prusti, we want to be able to support body invariants, i.e.,
        // functional specifications for loops, placed at a point further
        // *within* the loop body. When encoding such invariants into Viper, we
        // choose to unroll the first part of the loop (before the invariant is
        // reached), then encode the rest of the loop followed by the repeated
        // first part of the loop as an actual Viper loop with an invariant
        // annotation. This means that the part of the loop before the body
        // invariant is reached is encoded in duplicate, thus we need to ensure
        // basic block labels etc are *not* emitted in duplicate. As a result,
        // we keep track of a "pre-loop" set: the set of loops we have entered
        // (in the CFG) but whose body invariants we have not yet reached.
        //
        // The pre-loop set is a set because of pathological cases like this:
        // ```rust
        // while { // (OUT)
        //     loop { // (IN)
        //         // (maybe break;) ... (A)
        //         body_invariant!(...);
        //         // ...
        //     }
        //     // ...
        // } {
        //     // ...
        //     body_invariant!(...);
        //     // ...
        // }
        // ```
        // Here, the loop guard of the outer `while` loop contains a loop as
        // well. The "(A)" code fragment must be encoded multiple times:
        // * when entering (OUT) and (IN);
        // * when entering (OUT), for an arbitrary iteration of (IN);
        // * for an arbitrary iteration of (OUT); entering (IN); and
        // * for an arbitrary iteration of (OUT) and (IN).
        // The pre-loop sets for the four instances are, respectively:
        // * {OUT, IN},
        // * {OUT},
        // * {IN}, and
        // * {}.
        //
        // Loop heads and body invariants: loops as identified by the PCG loop
        // analysis have a loop head at the very first basic block, i.e., the
        // basic block that backedges jump to. For Prusti, we want to instead
        // consider the body invariant, if declared, to be the loop head, as
        // explained above. We only fall back to the PCG loop head if the body
        // invariant is not declared (i.e., we will only emit a permission
        // invariant, not a functional specification).
        //
        // The overall approach in this method is to walk basic blocks of the
        // CFG using a queue. Each item in the queue is a pair (basic block.
        // loop heads hit), where the latter is a set of loop heads that were
        // already crossed on this path. The pre-loop set is then the set of
        // loops the current block is in minus loop heads hit.

        // keep track of which loops we have already entered (on a particular
        // path); a LoopId is in this set as soon as the loop head (or the body
        // invariant for a loop with one) is entered
        let mut start_heads = FxHashSet::default();
        let start_loops = self
            .loop_analysis()
            .loops(mir::START_BLOCK)
            .collect::<FxHashSet<_>>();
        if !start_loops.is_empty() {
            // the start block is either not part of a loop, or else it is in
            // exactly one loop, of which it *may* be the head (if the body
            // invariant is omitted or coincides with the start block)
            assert_eq!(start_loops.len(), 1);
            let start_loop = *start_loops.iter().next().unwrap();
            if self.spec_blocks.loop_head_at[&start_loop] == mir::START_BLOCK {
                start_heads.insert(start_loop);
            }
        }
        queue.push(WorkItem {
            rpo_idx: rpo_idx[&mir::START_BLOCK],
            seq: next_seq.next().unwrap(),
            block: mir::START_BLOCK,
            heads_hit: start_heads,
        });

        while let Some(WorkItem {
            block,
            mut heads_hit,
            ..
        }) = queue.pop()
        {
            let in_loops = self.loop_analysis().loops(block).collect::<FxHashSet<_>>();

            heads_hit.retain(|l| in_loops.contains(l));

            // is this a loop head?
            if let Some(loop_spec) = self.spec_blocks.loop_specs.get(&block)
                && !heads_hit.insert(loop_spec.loop_id)
            {
                // we already walked over this loop head, so the full loop
                // iteration was already emitted on this path
                continue;
            }

            let pre_loops = in_loops
                .iter()
                .copied()
                .filter(|l| !heads_hit.contains(l))
                .sorted()
                .collect::<Vec<_>>();

            if !visited.insert((block, pre_loops.clone())) {
                continue;
            }

            self.current_block = Some(block);
            // Allocate label for current block and its successors with the
            // correct pre-loop prefixes.
            self.current_block_pres = Some(pre_loops.iter().map(|l| l.index()).collect());
            self.current_block_succs = Some(
                body.basic_blocks
                    .successors(block)
                    // Successors can have a different prefix than the current
                    // block; the successor can be:
                    // * part of a loop the current block is not in;
                    // * the loop head of a loop the current block is in; and/or
                    // * not part of a loop the current block is in.
                    .map(|succ| {
                        // TODO: this does a lot of duplicate work
                        //   maybe some two-pass approach would be nicer?
                        let succ_in_loops =
                            self.loop_analysis().loops(succ).collect::<FxHashSet<_>>();
                        let mut succ_heads_hit = heads_hit.clone();
                        succ_heads_hit.retain(|l| succ_in_loops.contains(l));
                        // An edge to a loop head enters the loop unless the path
                        // already went through that head, i.e. it is a back edge.
                        let mut enters_loop = false;
                        if let Some(loop_spec) = self.spec_blocks.loop_specs.get(&succ) {
                            enters_loop = succ_heads_hit.insert(loop_spec.loop_id);
                        }
                        let succ_pre_loops = succ_in_loops
                            .iter()
                            .copied()
                            .filter(|l| !succ_heads_hit.contains(l))
                            .map(|l| l.index())
                            .sorted();
                        let label = if enters_loop {
                            self.vcx
                                .mk_loop_entry_label(succ.as_usize(), succ_pre_loops)
                        } else {
                            self.vcx.mk_block_label(succ.as_usize(), succ_pre_loops)
                        };
                        (succ, label)
                    })
                    .collect(),
            );

            let entry = self.indirect_predicate_state.enter_block(
                self.vcx,
                block,
                body.basic_blocks.predecessors()[block].iter().copied(),
            );
            let disagreeing = entry.disagreeing;
            // The copies run at the start of the block, each on the paths from
            // its predecessor.
            self.block_entry_stmts = entry
                .copies
                .into_iter()
                .map(|(pred, assigns)| {
                    let taken = self.from_to_vars.get_or_create(self.vcx, pred, block).expr;
                    let assigns = assigns
                        .into_iter()
                        .map(|(var, value)| self.vcx.mk_pure_assign_stmt(var, value))
                        .collect::<Vec<_>>();
                    self.vcx
                        .mk_if_stmt(taken, self.vcx.alloc_slice(&assigns), &[])
                })
                .collect();
            // Cleanup blocks are not encoded (see `visit_basic_block_data`).
            if let Some(node) = disagreeing.first()
                && !body[block].is_cleanup
            {
                return Err(self.unsupported(
                    format!(
                        "INTERNAL ERROR: the predecessors of {block:?} disagree on whether {} is folded",
                        node.to_short_string(self.pcg_ctxt())
                    ),
                    body.source_info(mir::Location { block, statement_index: 0 }).span,
                ));
            }
            self.future_edges = self
                .block_entry_graph(block)
                .map(|graph| Self::future_edges_of(&graph))
                .unwrap_or_default();
            self.visit_basic_block_data(block, &body[block])?;
            self.indirect_predicate_state
                .leave_block(block, body.basic_blocks.successors(block));
            self.current_block = None;
            self.current_block_pres = None;
            self.current_block_succs = None;

            for successor in body.basic_blocks.successors(block) {
                // Specification-only arms are not encoded at all; their
                // switches are encoded as jumps to the live target.
                if self.spec_blocks.spec_arms.blocks.contains(&successor) {
                    continue;
                }
                queue.push(WorkItem {
                    rpo_idx: rpo_idx[&successor],
                    seq: next_seq.next().unwrap(),
                    block: successor,
                    heads_hit: heads_hit.clone(),
                });
            }
        }
        Ok(())
    }

    fn encode_spec_block(
        &mut self,
        spec_block: mir::BasicBlock,
        pcg_state: &Pcg<'_, 'vir>,
        place_capabilities: Option<&FxHashMap<Place<'vir>, CapabilityKind>>,
    ) -> EncodeResult<'vir, vir::ExprBool<'vir>, E> {
        let enc_output = self.deps.require_dep::<MirPureEnc>(MirPureEncTask {
            encoding_depth: 0,
            def_id: self.def_id,
            gargs: GParams::from(self.def_id).identity_args(),
            kind: PureKind::SpecBlock(spec_block),
        })?;
        let mut locals: FxHashMap<mir::Local, vir::ExprSnap<'vir>> = FxHashMap::default();
        for &local in &enc_output.inputs {
            let place: Place = local.into();
            let place_ty = RustTyDecomposition::from_ty(place.ty(self.pcg_ctxt()).ty, self.def_id);
            // The places the local is unpacked to; computed once per local.
            let local_state = &pcg_state.owned()[local];
            let leaf_places: Vec<Place<'vir>> = if local_state.is_allocated() {
                local_state
                    .expansions()
                    .leaf_places(self.pcg_ctxt())
                    .into_iter()
                    .map(Into::into)
                    .collect()
            } else {
                Vec::new()
            };
            let snap = self.encode_partial_place_snap(
                place,
                place_ty,
                pcg_state,
                &leaf_places,
                place_capabilities,
            )?;
            locals.insert(local, snap);
        }
        let expr = enc_output.reify(self.vcx.alloc(locals)).downcast_ty();
        Ok(expr)
    }

    fn encode_partial_place_snap(
        &mut self,
        place: Place<'vir>,
        place_ty: RustTyDecomposition<'vir>,
        pcg_state: &Pcg<'_, 'vir>,
        leaf_places: &[Place<'vir>],
        place_capabilities: Option<&FxHashMap<Place<'vir>, CapabilityKind>>,
    ) -> EncodeResult<'vir, vir::ExprSnap<'vir>, E> {
        let pure_ty = self.deps.require_dep::<TyUsePureEnc>(place_ty)?;

        // Check if place is unpacked
        if let Some(struct_data) = pure_ty.get_structlike()
            && leaf_places
                .iter()
                .any(|leaf| place.is_strict_prefix_of(*leaf))
        {
            let rust_struct = place_ty.ty.expect_structlike();
            let is_box = place_ty.ty.special == RustTySpecial::Box;
            let field_places = place.expand_field(None, self.pcg_ctxt()).unwrap();
            let mut field_snaps = Vec::with_capacity(rust_struct.fields.len());
            for field_data in rust_struct.fields.iter() {
                let field_ty = field_data.decompose_normalize(place_ty.args);
                let field_snap = match field_data.address {
                    // The value of a `Box`, stored at its pointer.
                    RustFieldAddress::Dynamic => self.encode_partial_place_snap(
                        place.project_deref(self.pcg_ctxt()),
                        field_ty,
                        pcg_state,
                        leaf_places,
                        place_capabilities,
                    )?,
                    // The PCG unpacks a `Box` only through its pointer, so
                    // its other fields are held whole once it is unpacked.
                    RustFieldAddress::Constant if is_box => {
                        self.encode_place_with_snap(field_places[field_data.fid.index()])?
                            .1
                    }
                    RustFieldAddress::Constant => self.encode_partial_place_snap(
                        field_places[field_data.fid.index()],
                        field_ty,
                        pcg_state,
                        leaf_places,
                        place_capabilities,
                    )?,
                };
                field_snaps.push(field_snap);
            }

            return Ok(struct_data.field_snaps_to_snap(field_snaps).upcast_ty());
        }

        // Check if we have capability to this place
        let moved_out = if let Some(capabilities) = place_capabilities {
            // for loop invariants we take the calculated capabilities of the invariant
            capabilities.get(&place).is_none_or(|c| c.is_write())
        } else {
            // for other specblocks we take the owned capabilities
            let Some(capability) = pcg_state.computed_owned_capability(place, self.pcg_ctxt())
            else {
                return Err(EncodeFullError::internal(
                    format!("no capability for `{place:?}` when encoding a specification"),
                    vec![self.current_span()],
                ));
            };
            capability.is_write()
        };

        if moved_out {
            // we do not own this place or it is moved out -> use temporary snap
            let snapshot_ty = pure_ty.snapshot;
            return Ok(self.new_tmp(snapshot_ty));
        }

        Ok(self.encode_place_with_snap(place)?.1)
    }

    fn visit_basic_block_data(
        &mut self,
        block: mir::BasicBlock,
        data: &mir::BasicBlockData<'vir>,
    ) -> EncodeResult<'vir, (), E> {
        // Taken first: a block that is not encoded drops them.
        let entry_stmts = std::mem::take(&mut self.block_entry_stmts);
        let current_block_label = self.vcx.mk_block_label(
            block.as_usize(),
            self.current_block_pres.as_ref().unwrap().iter().copied(),
        );

        // We are verifying the absence of panics, so cleanup block should never
        // be reached, or even referenced.
        if data.is_cleanup {
            self.encoded_blocks.push(
                self.vcx.mk_cfg_block(
                    current_block_label,
                    &[],
                    &[],
                    self.vcx
                        .mk_dummy_stmt(vir::vir_format!(self.vcx, "cleanup block")),
                ),
            );
            return Ok(());
        }

        // Specification-only arms are unreachable after their switches are
        // encoded as jumps to the live target; emit nothing. The specs
        // themselves are encoded separately (assertions at the block they
        // are attached to, loop invariants at the loop head, closure specs
        // as the closure's contract).
        if self.spec_blocks.spec_arms.blocks.contains(&block) {
            return Ok(());
        }

        self.deps().check_cycle()?;

        self.current_stmts = Some(Vec::with_capacity(
            data.statements.len(), // TODO: not exact?
        ));
        self.stmts(entry_stmts);
        self.current_block_label = Some(current_block_label);
        let cfpcs = self.fpcs_analysis.get_all_for_bb(block).unwrap().unwrap();

        // Calculate invariant at the body invariant, if specified, or at the
        // loop head by default
        let mut invariant = None;
        if let Some(loop_spec) = self.spec_blocks.loop_specs.get(&block) {
            let loop_place_usages = self
                .fpcs_analysis
                .analysis()
                .loop_place_usages(loop_spec.loop_id)
                .clone();
            let invariants = loop_spec.invariants.clone();
            let loop_id = loop_spec.loop_id;
            // The edges entering the loop go to an empty block `bb_entry_N` that jumps to
            // the head; the invariant refers to its label for the state in which the loop
            // was entered. The label must match the one `visit_body` gives the successors
            // of those edges.
            let entry_label = self.vcx.mk_loop_entry_label(
                block.as_usize(),
                self.current_block_pres.as_ref().unwrap().iter().copied(),
            );
            self.encoded_blocks.push(self.vcx.mk_cfg_block(
                entry_label,
                &[],
                &[],
                self.vcx.mk_goto_stmt(current_block_label),
            ));
            // The permission part of the invariant, and errors encoding it, refer to the
            // loop head.
            let head_span = self
                .body
                .source_info(loop_spec.original_head_block.start_location())
                .span;
            // The borrow graph on entry to the block, before the first
            // statement's actions: every edge into the block carries it.
            let entry_graph = self.block_entry_graph(block).unwrap();
            let (permissions, place_capabilities) = self.vcx.with_span(head_span, |_| {
                self.get_loop_inv(&cfpcs, &entry_graph, &loop_place_usages, self.pcg_ctxt())
            })?;
            let pcg_state = &cfpcs.statements[0].states[EvalStmtPhase::PreOperands];
            // The Viper loop havocs the edge flags set in the loop, including
            // those set between the loop's first block and the body invariant,
            // which guards of PCG actions further in the loop still refer to.
            let loop_branch_choices = pcg_state
                .borrow_pcg()
                .validity_conditions()
                .all_branch_choices()
                .filter(|choices| self.loop_analysis().in_loop(choices.from(), loop_id))
                .collect::<Vec<_>>();
            let branch_choices = loop_branch_choices
                .into_iter()
                .map(|choices| self.branch_choices_cond(choices))
                .collect::<Vec<_>>();
            let functional = invariants
                .into_iter()
                .map(|(spec_block, span)| {
                    let expr =
                        self.encode_spec_block(spec_block, pcg_state, Some(&place_capabilities))?;
                    self.vcx.with_span(span, |vcx| {
                        let error_msg = "loop invariant might not hold after a loop iteration that \
                                         preserves the loop condition.";
                        vcx.handle_error("invariant.not.preserved:assertion.false", move |_| {
                            Some(vec![PrustiError::verification(error_msg, span.into())])
                        });
                        self.vcx.with_span(span, |vcx| {
                            let error_msg =
                                "loop invariant might not hold in the first loop iteration.";
                            vcx.handle_error(
                                "invariant.not.established:assertion.false",
                                move |_| {
                                    Some(vec![PrustiError::verification(error_msg, span.into())])
                                },
                            );
                            Ok(expr.realloc_span())
                        })
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            invariant = Some(
                self.vcx.alloc_slice(
                    &permissions
                        .into_iter()
                        .chain(branch_choices)
                        .chain(functional)
                        .collect::<Vec<_>>(),
                ),
            );
        }

        self.current_fpcs = Some(cfpcs);

        assert!(self.current_terminator.is_none());
        if block == mir::START_BLOCK {
            // The predicates of the arguments' nodes, the targets of the
            // `InitialBorrows` edges, are held from the precondition.
            let arguments = self.current_fpcs.as_ref().unwrap().statements[0].states
                [EvalStmtPhase::PreOperands]
                .borrow_pcg()
                .graph()
                .edges()
                .filter_map(|edge| match edge.kind() {
                    BorrowPcgEdgeKind::BorrowFlow(flow)
                        if matches!(flow.kind(), BorrowFlowEdgeKind::InitialBorrows) =>
                    {
                        Some(flow.short())
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            for argument in arguments {
                self.assign_current_node_vars(argument)?;
            }
        }
        for (index, statement) in data.statements.iter().enumerate() {
            let location = mir::Location {
                block,
                statement_index: index,
            };
            self.visit_statement(statement, location)?;
        }
        let location = mir::Location {
            block,
            statement_index: data.statements.len(),
        };
        self.visit_terminator(data.terminator(), location)?;

        if let Some(specs) = self.spec_blocks.specs_for.get(&block).cloned() {
            for spec in specs {
                if matches!(spec.kind, SpecBlockKind::LoopInvariant) {
                    continue;
                }
                let current_fpcs = self.current_fpcs.take().unwrap();
                let pcg_state =
                    &current_fpcs.statements.last().unwrap().states[EvalStmtPhase::PostMain];
                let spec_expr = self.encode_spec_block(spec.block, pcg_state, None);
                self.current_fpcs = Some(current_fpcs);
                let spec_expr = spec_expr?;
                let span = spec.span;
                match spec.kind {
                    SpecBlockKind::Assert => {
                        self.vcx.with_span(span, |vcx| {
                            let error_msg = "the asserted expression might not hold";
                            vcx.handle_error("exhale.failed:assertion.false", move |_| {
                                Some(vec![PrustiError::verification(error_msg, span.into())])
                            });
                            self.stmt(self.vcx.mk_exhale_stmt(spec_expr));
                        });
                    }
                    SpecBlockKind::Assume => {
                        self.stmt(self.vcx.mk_inhale_stmt(spec_expr));
                    }
                    SpecBlockKind::Refute => {
                        self.vcx.with_span(span, |vcx| {
                            let error_msg =
                                "the refuted expression holds in all cases or could not be reached";
                            vcx.handle_error("refute.failed:refutation.true", move |_| {
                                Some(vec![PrustiError::verification(error_msg, span.into())])
                            });
                            self.stmt(self.vcx.mk_refute_stmt(spec_expr));
                        });
                    }
                    SpecBlockKind::LoopInvariant => {
                        // nothing to do: loop invariants are handled elsewhere
                    }
                }
            }
        }

        let stmts = self.current_stmts.take().unwrap();
        let terminator = self.current_terminator.take().unwrap();
        self.encoded_blocks.push(self.vcx.mk_cfg_block(
            self.current_block_label.take().unwrap(),
            invariant.unwrap_or_default(),
            self.vcx.alloc_slice(&stmts),
            terminator,
        ));
        Ok(())
    }

    fn visit_statement(
        &mut self,
        statement: &mir::Statement<'vir>,
        location: mir::Location,
    ) -> EncodeResult<'vir, (), E> {
        self.vcx.with_span(statement.source_info.span, |_| {
            self.deps().check_cycle()?;

            self.new_before_label(location);

            comment!(self, "[MIR] {location:?}: {statement:?}");

            self.pcg_phase_actions(location, EvalStmtPhase::PreOperands)?;

            // Read the statement's operands at their temporal position,
            // between the `PreOperands` and `PostOperands` repacks. An
            // operand's place may alias the destination, whose predicate the
            // `PreMain` weaken exhales before the effect is encoded (`x /= y`
            // lowers to `x = Div(copy x, move y)`).
            let captured = self.capture_operand_snaps(statement, location)?;

            self.pcg_phase_actions(location, EvalStmtPhase::PostOperands)?;
            self.pcg_phase_actions(location, EvalStmtPhase::PreMain)?;

            // Assignments to the locals only serving specification-only arms
            // (necessarily scaffolding stores) are not encoded, since the
            // locals are not declared.
            if let mir::StatementKind::Assign(box (dest, _)) = &statement.kind
                && let Some(local) = dest.as_local()
                && self.spec_blocks.spec_arms.spec_only_locals.contains(&local)
            {
                return Ok(());
            }

            let span = statement.source_info.span;

            match &statement.kind {
                mir::StatementKind::Assign(box (dest, rvalue)) => {
                    // What are we assigning to?
                    let proj_enc = self
                        .encode_place(Place::from(*dest))?
                        .expr
                        .expect_predicate();

                    // The snapshot of the value that we are assigning.
                    let rval_enc = self.encode_rvalue(rvalue, span, &captured)?;

                    let dest_ty = dest.ty(self.local_decls, self.vcx.tcx());
                    assert!(dest_ty.variant_index.is_none());
                    let dest_ty_out = self.ty_use_impure(dest_ty.ty);
                    let method_assign_app =
                        dest_ty_out.apply_method_assign(self.vcx, proj_enc, rval_enc.expr);
                    self.stmt(method_assign_app);
                    self.stmts(rval_enc.post_fold_stmts(proj_enc));
                }

                // no-ops
                mir::StatementKind::StorageLive(..)
                | mir::StatementKind::StorageDead(..)
                | mir::StatementKind::FakeRead(_)
                | mir::StatementKind::PlaceMention(_)
                | mir::StatementKind::AscribeUserType(..)
                | mir::StatementKind::Coverage(_)
                | mir::StatementKind::ConstEvalCounter
                | mir::StatementKind::Nop
                | mir::StatementKind::BackwardIncompatibleDropHint { .. } => {}

                mir::StatementKind::Intrinsic(intrinsic_kind) => {
                    return Err(EncodeFullError::unsupported(
                        format!("unsupported intrinsic statement {intrinsic_kind:?}"),
                        vec![span],
                    ));
                }
                mir::StatementKind::SetDiscriminant { .. } => unreachable!(
                    "the statement kind {:?} is not allowed in the MIR analysis phase",
                    statement.kind
                ),
            }

            self.pcg_phase_actions(location, EvalStmtPhase::PostMain)?;
            Ok(())
        })
    }

    fn visit_terminator(
        &mut self,
        terminator: &mir::Terminator<'vir>,
        location: mir::Location,
    ) -> EncodeResult<'vir, (), E> {
        self.vcx.with_span(terminator.source_info.span, |_| {
            self.encode_terminator(terminator, location)
        })
    }

    fn encode_terminator(
        &mut self,
        terminator: &mir::Terminator<'vir>,
        location: mir::Location,
    ) -> EncodeResult<'vir, (), E> {
        self.deps().check_cycle()?;

        self.new_before_label(location);
        comment!(self, "[MIR] {location:?}: {:?}", terminator.kind);
        let span = terminator.source_info.span;

        // `PostMain` is not emitted here: a terminator's operands are read
        // after these phases' repacks (e.g. a `SwitchInt` discriminant that
        // projects into an aggregate is read in the branch condition, relying
        // on the `PreOperands` unfolds), while the `PostMain` actions re-pack
        // owned places for the CFG join and thus may fold those operands'
        // places away. [Self::finish_terminator] emits them after the operands
        // have been read, then records a snapshot before the edge repacks;
        // every terminator arm with a successor must call it. Terminators
        // without successors have no successor state to re-pack for.
        for phase in [
            EvalStmtPhase::PreOperands,
            EvalStmtPhase::PostOperands,
            EvalStmtPhase::PreMain,
        ] {
            comment!(self, "PCG (T) {phase}");
            self.pcg_phase_actions(location, phase)?;
        }

        // A specification-only arm's `if false` switch is encoded as an
        // unconditional jump to the live target (the continuation, or the
        // inline ghost body); the arm itself is not encoded.
        let spec_goto = self
            .spec_blocks
            .spec_arms
            .switches
            .get(&location.block)
            .map(|live_target| mir::TerminatorKind::Goto {
                target: *live_target,
            });
        let terminator = match spec_goto.as_ref().unwrap_or(&terminator.kind) {
            mir::TerminatorKind::Goto { target }
            | mir::TerminatorKind::FalseUnwind {
                real_target: target,
                ..
            }
            | mir::TerminatorKind::FalseEdge {
                real_target: target,
                ..
            }
            | mir::TerminatorKind::Drop { target, .. } => {
                // A `Drop`'s semantics (releasing the dropped place's
                // permission via a weaken exhale) are carried by the PCG
                // statements, so only its goto remains to be encoded here.
                self.goto_single_succ(location, *target)?
            }
            mir::TerminatorKind::SwitchInt { discr, targets } => {
                let discr_ty_rs = discr.ty(self.local_decls, self.vcx.tcx());
                let discr_ty = self.ty_use_pure(discr_ty_rs).expect_primitive();

                let discr_ex =
                    discr_ty.snap_to_prim(self.encode_operand_snap(discr, &None)?.downcast_ty());
                let discr_tmp = self.new_tmp(discr_ex.ty());
                self.stmt(self.vcx.mk_pure_assign_stmt(discr_tmp, discr_ex));
                self.finish_terminator(location)?;

                let goto_targets = self.vcx.alloc_slice(
                    &targets
                        .iter()
                        .enumerate()
                        .map(|(idx, (value, target))| {
                            let mut extra_stmts = self.collect_pcs_succ_at(idx, target)?;
                            extra_stmts.push(self.set_from_to_flag(location.block, target));

                            Ok(self.vcx.mk_goto_if_target(
                                discr_ty.expr_from_bits(discr_ty_rs, value).as_dyn(),
                                self.current_block_succs.as_ref().unwrap()[&target],
                                self.vcx.alloc_slice(&extra_stmts),
                            ))
                        })
                        .collect::<EncodeResult<'vir, Vec<_>, E>>()?,
                );
                let goto_otherwise =
                    self.current_block_succs.as_ref().unwrap()[&targets.otherwise()];

                let otherwise_succ_idx = goto_targets.len();
                let mut otherwise_stmts =
                    self.collect_pcs_succ_at(otherwise_succ_idx, targets.otherwise())?;
                otherwise_stmts.push(self.set_from_to_flag(location.block, targets.otherwise()));

                self.vcx.mk_goto_if_stmt(
                    discr_tmp.as_dyn(),
                    goto_targets,
                    goto_otherwise,
                    self.vcx.alloc_slice(&otherwise_stmts),
                )
            }
            mir::TerminatorKind::Return => {
                let current_fpcs = self.current_fpcs.take().unwrap();
                let borrows = current_fpcs.statements.last().unwrap().states
                    [EvalStmtPhase::PostMain]
                    .borrow_pcg();
                let wand_packages = self.package_wands(borrows)?;
                self.current_fpcs = Some(current_fpcs);
                self.stmts(wand_packages);

                self.vcx
                    .mk_goto_stmt(self.vcx.alloc(vir::CfgBlockLabelData::End))
            }
            mir::TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                ..
            } => {
                // emit the current block, create a new label for the terminator
                // TODO: should we do this for any other terminators?
                let terminator_label = self.vcx.mk_terminator_label(
                    self.current_block.unwrap().as_usize(),
                    self.current_block_pres.as_ref().unwrap().iter().copied(),
                );
                self.encoded_blocks.push(
                    self.vcx.mk_cfg_block(
                        self.current_block_label.replace(terminator_label).unwrap(),
                        &[],
                        self.vcx
                            .alloc_slice(&self.current_stmts.replace(Vec::new()).unwrap()),
                        self.vcx.mk_goto_stmt(terminator_label),
                    ),
                );

                let (func_def_id, caller_substs) = self.get_call_data(func);
                // A call whose span comes from a macro expansion (e.g. the
                // `core::panicking::panic` call inside a failing `assert!`)
                // was not written by the user, so reporting "precondition
                // might not hold" at the expanded fragment is confusing: no
                // visible call exists there. Report at the macro invocation
                // that produced the call, naming the actually-called function.
                let verification_error = if span.from_expansion() {
                    let name = self.vcx.tcx().def_path_str(func_def_id);
                    let message = format!(
                        "precondition of `{name}` (called by this macro expansion) might not hold"
                    );
                    PrustiError::verification(message, span.source_callsite().into())
                } else {
                    PrustiError::verification("precondition might not hold.", span.into())
                };

                let dest = self
                    .encode_place(Place::from(*destination))?
                    .expr
                    .expect_predicate();
                self.vcx.with_span(span, |vcx| {
                    let pure = self.pure_call_result(func_def_id, caller_substs, args, span)?;
                    if let Some(pure) = pure {
                        self.wandless_calls.insert(self.current_block.unwrap());
                        let return_ty = destination.ty(self.local_decls, self.vcx.tcx()).ty;
                        let return_ty_use = self.ty_use_impure(return_ty);
                        let assign_stmt = return_ty_use.apply_method_assign(self.vcx, dest, pure);
                        self.stmt(assign_stmt);
                        // The call returned a value of the return type, therefore
                        // it is inhabited.
                        self.stmt(vcx.mk_inhale_stmt(return_ty_use.inhabited()));
                    } else {
                        let func_out = self.deps.require_dep_spanned::<encoders::MethodCallEnc>(
                            CallTaskDescription::new(self.def_id, caller_substs, func_def_id),
                            span,
                        )?;

                        let method_in = args
                            .iter()
                            .map(|arg| self.encode_operand(&arg.node))
                            .collect::<Result<Vec<_>, _>>()?;

                        // The callee's predicates for the lifetimes in its
                        // type parameters cover a block of the caller's
                        // positions each, which may be empty or have several
                        // positions; they are adapted before and after the call.
                        let adjust = if func_out.is_trait_stub() {
                            // A trait method's stub has no wands in its
                            // contract, so its call gives none to apply.
                            self.wandless_calls.insert(self.current_block.unwrap());
                            None
                        } else {
                            let wands = self.deps.require_dep::<WandEnc>(WandEncTask {
                                data: FunctionData::new(func_def_id),
                            })?;
                            let arg_snaps = args
                                .iter()
                                .map(|arg| self.encode_operand_snap_immediate(&arg.node))
                                .collect::<EncodeResult<'vir, Vec<_>, E>>()?;
                            let call_ctx = self.gargs(caller_substs);
                            let before = wands.before_call(vcx, self.deps, &arg_snaps, call_ctx)?;
                            self.stmts(before);
                            Some((wands, arg_snaps, call_ctx))
                        };

                        let call = func_out.call(method_in, dest);

                        let label_pre = self.new_label("pre");
                        vcx.handle_error(
                            "call.precondition:assertion.false",
                            move |reason_span_opt| {
                                let mut error = verification_error.clone();
                                if let Some(reason_span) = reason_span_opt {
                                    error.add_note_mut(
                                        "the failing precondition is here",
                                        Some(reason_span.into()),
                                    );
                                }
                                Some(vec![error])
                            },
                        );
                        self.stmts(call);
                        let label_post = self.new_label("post");
                        self.call_labels
                            .insert(location.block, (label_pre, label_post));

                        if let Some((wands, arg_snaps, call_ctx)) = adjust {
                            // The postcondition gives back the arguments'
                            // predicates keyed by their snapshots before the call.
                            let arg_snaps = arg_snaps
                                .into_iter()
                                .map(|arg| vcx.mk_local_labelled_old_expr(arg, label_pre))
                                .collect::<Vec<_>>();
                            let (_, result, _, _) =
                                self.encode_place_with_snap(Place::from(*destination))?;
                            let after =
                                wands.after_call(vcx, self.deps, &arg_snaps, result, call_ctx)?;
                            self.stmts(after);
                        }
                        // The callee gives the arguments' predicates back
                        // keyed by their values before the call, extended by
                        // what the arguments hold after it.
                        let arg_locals = args
                            .iter()
                            .filter_map(|arg| arg.node.place().map(|place| place.local))
                            .collect::<Vec<_>>();
                        self.rekey_nodes_of(&arg_locals)?;
                    }
                    Ok(())
                })?;

                match *target {
                    Some(target) => self.goto_single_succ(location, target)?,
                    None => {
                        // TODO: detect panic causes, adjust message accordingly
                        self.vcx.with_span(span, |vcx| {
                            vcx.handle_error("exhale.failed:assertion.false", move |_| {
                                Some(vec![PrustiError::verification(
                                    "unreachable statement might be reached",
                                    span.into(),
                                )])
                            });
                            self.stmt(self.vcx.mk_exhale_stmt(self.vcx.mk_bool::<false>()));
                            self.vcx.mk_assume_false_stmt()
                        })
                    }
                }
            }
            mir::TerminatorKind::Assert {
                cond,
                expected,
                msg,
                target,
                ..
            } => {
                let enc = self
                    .encode_operand_snap(cond, &None)?
                    .downcast_ty::<vir::Bool>();
                let expected = self
                    .vcx
                    .mk_const_expr(vir::ConstData::Bool(*expected))
                    .downcast_ty();
                let assert = self.vcx.mk_eq_expr(enc, expected);
                if let Some(error_msg) = assert_error_message(msg) {
                    self.vcx.with_span(span, |vcx| {
                        vcx.handle_error("exhale.failed:assertion.false", move |_| {
                            Some(vec![PrustiError::verification(error_msg, span.into())])
                        });
                        self.stmt(self.vcx.mk_exhale_stmt(assert));
                    });
                } else {
                    self.stmt(self.vcx.mk_inhale_stmt(assert));
                }
                // The check is the terminator's main effect, emitted before
                // the deferred `PostMain` re-pack, which may fold the place
                // the condition projects into.
                self.goto_single_succ(location, *target)?
            }
            mir::TerminatorKind::Unreachable => self.vcx.with_span(span, |vcx| {
                vcx.handle_error("exhale.failed:assertion.false", move |_| {
                    Some(vec![PrustiError::verification(
                        "unreachable statement might be reached",
                        span.into(),
                    )])
                });
                self.stmt(self.vcx.mk_exhale_stmt(self.vcx.mk_bool::<false>()));
                self.vcx.mk_assume_false_stmt()
            }),

            mir::TerminatorKind::UnwindResume | mir::TerminatorKind::UnwindTerminate(..) => {
                self.vcx.with_span(span, |vcx| {
                    vcx.handle_error("exhale.failed:assertion.false", move |_| {
                        Some(vec![PrustiError::unsupported(
                            "unwind paths are not supported",
                            span.into(),
                        )])
                    });
                    self.stmt(self.vcx.mk_exhale_stmt(self.vcx.mk_bool::<false>()));
                    self.vcx.mk_assume_false_stmt()
                })
            }

            mir::TerminatorKind::TailCall { .. } => {
                return Err(EncodeFullError::unsupported(
                    "tail calls are not supported".to_string(),
                    vec![span],
                ));
            }
            mir::TerminatorKind::Yield { .. } => {
                return Err(EncodeFullError::unsupported(
                    "yield statements are not supported".to_string(),
                    vec![span],
                ));
            }
            mir::TerminatorKind::CoroutineDrop => {
                return Err(EncodeFullError::unsupported(
                    "coroutines are not supported".to_string(),
                    vec![span],
                ));
            }
            mir::TerminatorKind::InlineAsm { .. } => {
                return Err(EncodeFullError::unsupported(
                    "inline assembly is not supported".to_string(),
                    vec![span],
                ));
            }
        };
        assert!(self.current_terminator.replace(terminator).is_none());
        Ok(())
    }

    fn pure_call_result(
        &mut self,
        func_def_id: DefId,
        caller_substs: ty::GenericArgsRef<'vir>,
        args: &[Spanned<mir::Operand<'vir>>],
        span: Span,
    ) -> EncodeResult<'vir, Option<vir::ExprSnap<'vir>>, E> {
        // The bodiless `ptr_metadata` intrinsic is only lowered to
        // `UnOp::PtrMetadata` in optimized MIR; do the lowering here.
        let intrinsic = self.vcx.tcx().intrinsic(func_def_id);
        let intrinsic = intrinsic.and_then(RustcIntrinsic::from_intrinsic);
        Ok(if let Some(intrinsic) = intrinsic {
            Some(self.encode_intrinsic(intrinsic, caller_substs, args, &None)?)
        } else if let Some(builtin) = PrustiBuiltin::new(func_def_id, self.gargs(caller_substs)) {
            // A `prusti_contracts` builtin used in executable code
            // (e.g. `Int::from(2) + Int::from(3)`): encode it with
            // the shared operand-based encoding and assign the
            // resulting ghost snapshot into the destination. The
            // spec-only builtins (those gated behind the `prusti`
            // feature) are rejected, except in ghost code (the inline
            // bodies of `ghost!` blocks), where all non-`Spec` builtins
            // are allowed.
            let in_ghost_code = self
                .spec_blocks
                .ghost
                .code
                .contains(&self.current_block.unwrap());
            if builtin.is_spec_only()
                && (!in_ghost_code || matches!(builtin, PrustiBuiltin::Spec(_)))
            {
                return Err(EncodeFullError::unsupported(
                    format!(
                        "`prusti_contracts` builtin {builtin:?} cannot be used in executable code"
                    ),
                    vec![span],
                ));
            }
            Some(self.encode_prusti_builtin(
                builtin,
                func_def_id,
                self.gargs(caller_substs),
                args,
                span,
                &None,
            )?)
        } else {
            None
        })
    }
}

impl<'vir, 'enc, E: TaskEncoder> PureRvalueEnc<'vir> for ImpureEncVisitor<'vir, 'enc, E> {
    type Encoder = E;
    type EncodePlaceCtxt = OperandSnaps<'vir>;
    const PURE: bool = false;
    type ExprCurr = ();
    type ExprNext = !;
    fn context(&self) -> GParams<'vir> {
        // Impure bodies are encoded at identity substs, so the function's own
        // context is the body's context.
        self.def_id.into()
    }

    fn deps(&mut self) -> &mut TaskEncoderDependencies<'vir, Self::Encoder> {
        self.deps
    }

    fn vcx(&self) -> &'vir vir::VirCtxt<'vir> {
        self.vcx
    }

    fn body(&self) -> &mir::Body<'vir> {
        self.body
    }
    fn encoding_depth(&self) -> usize {
        0
    }

    fn ty_use_pure(&mut self, ty: ty::Ty<'vir>) -> TyUsePure<'vir> {
        let ty_task = RustTyDecomposition::from_ty(ty, self.def_id);
        self.deps.require_dep::<TyUsePureEnc>(ty_task).unwrap()
    }

    fn encode_operand_snap(
        &mut self,
        operand: &mir::Operand<'vir>,
        operand_snaps: &Self::EncodePlaceCtxt,
    ) -> EncodeResult<'vir, vir::ExprSnap<'vir>, E> {
        // While a statement's effect is encoded, every place operand must
        // have been captured by [Self::capture_operand_snaps]. In particular,
        // `Move` operands were already consumed at capture time (snapshot
        // temporary plus predicate exhale) and must not be consumed again.
        if let Some(operand_snaps) = operand_snaps
            && let Some(place) = operand.place()
        {
            let snap = operand_snaps.get(&place).unwrap_or_else(|| {
                panic!("operand {operand:?} was not captured for the current statement")
            });
            return Ok(*snap);
        }
        match operand {
            mir::Operand::Move(_) => self.capture_operand_snap(operand),
            _ => self.encode_operand_snap_immediate(operand),
        }
    }

    fn encode_place_snap(
        &mut self,
        place: Place<'vir>,
        _ctxt: &Self::EncodePlaceCtxt,
    ) -> EncodeResult<'vir, vir::ExprSnap<'vir>, E> {
        Ok(self.encode_place_with_snap(place)?.1)
    }
}
