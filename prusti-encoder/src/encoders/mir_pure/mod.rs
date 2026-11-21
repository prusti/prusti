use crate::encoders::{
    FunctionCallEnc, Mode, PrustiBuiltin, SpecBuiltin,
    custom::{PairUse, PairUseEnc},
    mir_fn::{CallTaskDescription, RustSignature, SpecArms},
    mir_shared::{EncodeResult, PureRvalueEnc, RustcIntrinsic, unsupported_opaque},
    pure::spec::MirSpecEncMode,
    ty::{
        RustTyDecomposition,
        generics::{GArgs, GParams},
        use_pure::{TyUsePure, TyUsePureEnc},
    },
};
use itertools::Itertools;
use pcg::utils::Place;
use prusti_rustc_interface::{
    abi,
    data_structures::graph::{self, Successors},
    index::{IndexSlice, IndexVec},
    middle::{
        mir,
        ty::{self, TyKind},
    },
    span::{Span, Spanned, def_id::DefId},
};
use rustc_hash::{FxHashMap, FxHashSet};
use std::fmt;
use task_encoder::{
    DependencyErrorLink, EncodeFullError, EncodeFullResult, ErrorKind, TaskEncoder,
    TaskEncoderDependencies,
};
use vir::{CastType, CompType, add_debug_note};

mod place;
mod rvalue;
mod spec_closure;
mod ssa;
mod walk;

pub struct MirPureEnc;

#[derive(Clone, Debug)]
pub enum MirPureEncError {}

/// The values of an encoded body's inputs.
pub type Inputs<'vir> = &'vir FxHashMap<mir::Local, vir::ExprSnap<'vir>>;
/// The values of a spec closure's captures, in the modes its body reads them
/// in (see `MirPureEncOutput::captures`): the captured places, encoded by the
/// enclosing body (see `Enc::encode_capture`).
type Captures<'vir> =
    &'vir IndexSlice<abi::FieldIdx, FxHashMap<Modes, EncodedPlaceGen<'vir, (), !>>>;
/// What an encoded body is instantiated with (see
/// `MirPureEncOutput::reify_at`).
#[derive(Clone, Copy)]
struct ExprInput<'vir> {
    /// The body being instantiated; the encoded body checks that it is its
    /// own.
    def_id: DefId,
    /// The values of the body's inputs.
    inputs: Inputs<'vir>,
    /// The state `old` refers to.
    old: vir::OldLabel<'vir>,
    /// For a spec closure, the values of its captures.
    captures: Captures<'vir>,
}
type ExprRet<'vir> = vir::ExprGenSnap<'vir, ExprInput<'vir>, vir::ExprKind<'vir>>;
type ExprRetRef<'vir> = vir::ExprGenRef<'vir, ExprInput<'vir>, vir::ExprKind<'vir>>;
type ExprRetAny<'vir, T> = vir::ExprGen<'vir, ExprInput<'vir>, vir::ExprKind<'vir>, T>;
/// An encoded spec closure (see `Enc::encode_spec_closure`): the quantified
/// variables derived from the closure's arguments, and the closure's body.
type SpecClosure<'vir> = (
    &'vir [vir::LocalDeclSnap<'vir>],
    ExprRetAny<'vir, vir::Bool>,
);

#[derive(Clone, Debug)]
pub struct MirPureEncOutput<'vir> {
    pub inputs: Vec<mir::Local>,
    /// The body `expr` encodes.
    pub def_id: DefId,
    expr: ExprRet<'vir>,
    /// For a spec closure, the modes its body reads each capture in (see
    /// `Enc::encode_spec_closure`); empty for other bodies.
    captures: IndexVec<abi::FieldIdx, FxHashSet<Modes>>,
}

impl<'vir> MirPureEncOutput<'vir> {
    /// The encoded body for the given values of its inputs, with `old`
    /// referring to the pre-state of the method.
    pub fn reify(&self, inputs: Inputs<'vir>) -> vir::ExprSnap<'vir> {
        self.reify_at(inputs, vir::OldLabel::None)
    }

    /// Like `reify`, with `old(..)` in the body referring to the state `old`.
    pub fn reify_at(&self, inputs: Inputs<'vir>, old: vir::OldLabel<'vir>) -> vir::ExprSnap<'vir> {
        use vir::Reify;
        let input = ExprInput {
            def_id: self.def_id,
            inputs,
            old,
            captures: IndexSlice::empty(),
        };
        vir::with_vcx(|vcx| self.expr.reify(vcx, input))
    }
}

bitflags::bitflags! {
    /// The modes a specification reads places in. A mode only changes reads
    /// rooted at an argument or the result: `OLD` and `BEFORE_EXPIRY` the
    /// heap read of a `&mut` dereference, `REL0`/`REL1` the whole read.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct Modes: u8 {
        const OLD = 1 << 0;
        const REL0 = 1 << 1;
        const REL1 = 1 << 2;
        const BEFORE_EXPIRY = 1 << 3;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PureKind {
    /// The closure passed to a quantifier or to `spec_block`, encoded as part
    /// of the expression that contains it, in the generic context of the item
    /// `context`.
    Closure {
        context: DefId,
    },
    /// A specification, attached to the item `context` specifies. That is
    /// not the specified item itself when an `impl` refines the
    /// specification of a trait: there the trait's parameters (notably
    /// `Self`) become the implementing type, and the specification body is
    /// instantiated accordingly.
    Spec {
        context: DefId,
        /// How the arguments are supplied, which decides whether their
        /// snapshots are deep (see `Enc::impure_context`).
        mode: MirSpecEncMode,
    },
    Pure,
    Constant(mir::Promoted),
    SpecBlock(mir::BasicBlock),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct MirPureEncTask<'vir> {
    // TODO: depth of encoding should be in the lazy context rather than here;
    //   can we integrate the lazy context into the identifier system?
    pub encoding_depth: usize,
    pub kind: PureKind,
    pub parent_def_id: DefId, // ID of the function
    /// The generic arguments the body is encoded with, together with the
    /// context whose parameters give them meaning.
    pub gargs: GArgs<'vir>,
}

impl TaskEncoder for MirPureEnc {
    task_encoder::encoder_cache!(MirPureEnc);
    const ENCODER_NAME: &'static str = "MIR pure encoder";

    type TaskDescription<'vir> = MirPureEncTask<'vir>;

    type TaskKey<'vir> = (
        usize,       // encoding depth
        PureKind,    // encoding a pure function?
        DefId,       // ID of the function
        GArgs<'vir>, // the generic arguments and their context
    );

    type OutputFullDependency<'vir> = MirPureEncOutput<'vir>;

    type EncodingError = MirPureEncError;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        (
            task.encoding_depth,
            task.kind,
            task.parent_def_id,
            task.gargs,
        )
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;

        let (_, kind, def_id, gargs) = *task_key;

        tracing::debug!("encoding {def_id:?}");
        let (inputs, expr, captures) = vir::with_vcx(move |vcx| {
            // A specification is instantiated with the arguments of the
            // encoding, which differ from its own when a trait's specification
            // is encoded for an `impl` of that trait. So is the closure of a
            // quantifier or `spec_block`, which is encoded in the context of
            // the expression it is part of. The other bodies are encoded
            // generically, so they are taken as stored.
            let body = match kind {
                PureKind::Closure { context } => {
                    let substs = vcx.tcx().mk_args(gargs.args());
                    vcx.body_mut().get_closure_body(def_id, substs, context)
                }
                PureKind::Spec { context, .. } => {
                    let substs = vcx.tcx().mk_args(gargs.args());
                    vcx.body_mut().get_spec_body(def_id, substs, context)
                }
                PureKind::Pure => crate::encoders::pure_body(def_id),
                PureKind::Constant(promoted) => {
                    vcx.body_mut().get_promoted_constant_body(def_id, promoted)
                }
                PureKind::SpecBlock(_) => crate::encoders::impure_body(def_id)
                    .unwrap_or_else(|| panic!("no body to encode for {def_id:?}")),
            };

            // Encode under the body's span: expression nodes pick up the
            // ambient span at creation, so everything in a pure/spec body
            // defaults to the body's source (e.g. the spec attribute for a
            // spec closure), refined per statement and terminator. This is
            // what error positions inside the body backtranslate to. The MIR
            // span of a body deserialized from another crate is dummy; the
            // definition's span survives in the crate metadata.
            let body_span = if body.span.is_dummy() {
                vcx.tcx().def_span(def_id)
            } else {
                body.span
            };
            let mut enc = Enc::new(vcx, task_key.0, def_id, kind, gargs, &body, deps);
            let expr_inner = vcx.with_span(body_span, |_| {
                if let PureKind::SpecBlock(block) = kind {
                    enc.encode_spec_block(block)
                } else {
                    enc.encode_body()
                }
            })?;
            let captures = enc.captures.take().unwrap_or_default();
            let inputs = std::mem::take(&mut enc.uses)
                .read
                .into_iter()
                .filter(|(l, v)| *l != mir::RETURN_PLACE && *v == 0)
                .map(|(l, _v)| l)
                .sorted()
                .collect::<Vec<_>>();
            let inputs_expected = inputs.len();

            // We wrap the expression with an additional lazy that will perform
            // some sanity checks. These requirements cannot be expressed using
            // only the type system.
            let snapshot = if let PureKind::SpecBlock(..) = kind {
                vir::TYPE_BOOL.upcast_ty()
            } else {
                let ret = RustTyDecomposition::from_ty(body.return_ty(), enc.context);
                deps.require_ref::<TyUsePureEnc>(ret)?.snapshot
            };
            // Created under the body's span: this wrapper is the node whose
            // span survives reification as the top of the expression (`span`
            // is passed through by `reify`), so it is what error positions
            // pointing at the whole body (e.g. a failing precondition
            // conjunct) backtranslate to.
            let expr = vcx.with_span(body_span, |vcx| {
                vcx.mk_lazy_expr(
                    vir::vir_format!(vcx, "pure body {def_id:?}"),
                    snapshot,
                    Box::new(move |vcx, lctx: ExprInput<'_>| {
                        // check: are we actually providing inputs for the
                        //   correct `DefId`?
                        assert_eq!(lctx.def_id, def_id);

                        // check: are we providing the expected number of inputs?
                        // TODO: check that the expected inputs are present; this
                        //   check is not precise
                        assert!(lctx.inputs.len() >= inputs_expected);

                        use vir::Reify;
                        expr_inner.kind.reify(vcx, lctx)
                    }),
                )
            });
            add_debug_note!(expr.debug_info, "Inner expr: {}", expr_inner.debug_info);
            Ok((inputs, expr, captures))
        })?;
        tracing::debug!("finished {def_id:?}");

        let output = MirPureEncOutput {
            inputs,
            def_id,
            expr,
            captures,
        };
        Ok(((), output))
    }
}

#[derive(Debug, Default)]
struct Update<'vir> {
    binds: Vec<UpdateBind<'vir>>,
    versions: FxHashMap<mir::Local, Version<'vir>>,
}

#[derive(Debug)]
enum UpdateBind<'vir> {
    Local(mir::Local, Version<'vir>, ExprRet<'vir>),
    Phi(vir::LocalDeclPair<'vir>, ExprRetAny<'vir, vir::Pair>),
}

impl<'vir> Update<'vir> {
    fn new() -> Self {
        Self::default()
    }

    fn mk_local(
        vcx: &'vir vir::VirCtxt<'vir>,
        encoding_depth: usize,
        local: mir::Local,
        version: Version<'vir>,
    ) -> &'vir str {
        vir::vir_format!(vcx, "_{}_{}s_{}", encoding_depth, local.as_usize(), version)
    }

    fn assign(
        &mut self,
        vcx: &'vir vir::VirCtxt<'vir>,
        encoding_depth: usize,
        local: mir::Local,
        mut version: Version<'vir>,
        expr: ExprRet<'vir>,
    ) {
        let decl = vcx.mk_local_decl(
            Self::mk_local(vcx, encoding_depth, local, version),
            expr.ty(),
        );
        version.initialised = Some(decl);
        self.binds.push(UpdateBind::Local(local, version, expr));
        self.versions.insert(local, version);
    }

    fn merge(self, newer: Option<Self>) -> Option<Self> {
        Some(self.merge_inner(newer?))
    }

    fn merge_inner(self, newer: Self) -> Self {
        Self {
            binds: self.binds.into_iter().chain(newer.binds).collect(),
            versions: self.versions.into_iter().chain(newer.versions).collect(),
        }
    }

    fn add_to_map(&self, curr_ver: &mut FxHashMap<mir::Local, Version<'vir>>) {
        for (local, ver) in &self.versions {
            curr_ver.insert(*local, *ver);
        }
    }
}

/// The versions of locals that the encoded expression reads, directly or
/// through the value of a version it reads. Only the bindings of these
/// versions are emitted, so that a binding nothing reads emits nothing its
/// value reads either.
#[derive(Default)]
struct Uses {
    /// The versions read. Closed under `pending`: once a version is read, so
    /// are the versions its value reads.
    read: FxHashSet<(mir::Local, usize)>,
    /// For each assigned version not read yet, the versions its value reads.
    pending: FxHashMap<(mir::Local, usize), Vec<(mir::Local, usize)>>,
    /// While an assignment is encoded, the versions its value reads so far.
    assignment: Option<Vec<(mir::Local, usize)>>,
}

impl Uses {
    /// Records a read of version `index` of `local`: by the value of the
    /// assignment being encoded, if any, or else by the expression itself.
    fn read(&mut self, local: mir::Local, index: usize) {
        if let Some(reads) = &mut self.assignment {
            reads.push((local, index));
        } else if self.read.insert((local, index))
            && let Some(reads) = self.pending.remove(&(local, index))
        {
            for (local, index) in reads {
                self.read(local, index);
            }
        }
    }

    /// Starts recording the reads of an assignment's value.
    fn start_assignment(&mut self) {
        debug_assert!(self.assignment.is_none(), "assignments do not nest");
        self.assignment = Some(Vec::new());
    }

    /// Stops recording the reads of an assignment's value, returning them.
    fn end_assignment(&mut self) -> Vec<(mir::Local, usize)> {
        self.assignment.take().unwrap()
    }

    /// Records that version `index` of `local` is assigned a value that
    /// reads `reads`; they count as read once that version is.
    fn assigned(&mut self, local: mir::Local, index: usize, reads: Vec<(mir::Local, usize)>) {
        self.pending.insert((local, index), reads);
    }

    fn is_read(&self, local: mir::Local, index: usize) -> bool {
        self.read.contains(&(local, index))
    }
}

struct Enc<'vir: 'enc, 'enc> {
    vcx: &'vir vir::VirCtxt<'vir>,
    encoding_depth: usize,
    context: GParams<'vir>,
    /// The item that `context` holds the generic parameters of.
    context_def_id: DefId,
    body: &'enc mir::Body<'vir>,
    rev_doms: rev_doms::ReverseDominators,
    /// The `ghost!` and `closure!` arms of the body (the same detection the
    /// impure encoder uses, via `SpecBlocks`).
    spec_arms: SpecArms,
    deps: &'enc mut TaskEncoderDependencies<'vir, MirPureEnc>,
    /// Always holds the next version to be used for a local.
    version_ctr: IndexVec<mir::Local, usize>,
    uses: Uses,
    phi_ctr: usize,
    modes: Modes,
    impure_context: bool,
    /// For a spec closure, the modes its body reads each capture in (see
    /// [`MirPureEncOutput::captures`]); `None` for other bodies.
    captures: Option<IndexVec<abi::FieldIdx, FxHashSet<Modes>>>,
}

/// An encoded place, as expressions in the context `Curr`/`Next`: lazy ones
/// while a body is encoded (`EncodedPlace`), and reified ones for the captures
/// passed into a spec closure (`Captures`).
struct EncodedPlaceGen<'vir, Curr, Next> {
    snap: vir::ExprGenSnap<'vir, Curr, Next>,
    place_ref: Option<vir::ExprGenRef<'vir, Curr, Next>>,
    /// The metadata for the pointed-to value. Set when going through a
    /// `ProjectionElem::Deref`.
    metadata: Option<vir::ExprGenSnap<'vir, Curr, Next>>,
}

type EncodedPlace<'vir> = EncodedPlaceGen<'vir, ExprInput<'vir>, vir::ExprKind<'vir>>;

impl<'vir, Curr, Next> EncodedPlaceGen<'vir, Curr, Next> {
    fn new(
        expr: vir::ExprGenSnap<'vir, Curr, Next>,
        place_ref: Option<vir::ExprGenRef<'vir, Curr, Next>>,
    ) -> Self {
        Self {
            snap: expr,
            place_ref,
            metadata: None,
        }
    }

    fn with_metadata(mut self, metadata: vir::ExprGenSnap<'vir, Curr, Next>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

impl<'vir: 'enc, 'enc> PureRvalueEnc<'vir> for Enc<'vir, 'enc> {
    type Encoder = MirPureEnc;
    type EncodePlaceCtxt = FxHashMap<mir::Local, Version<'vir>>;
    const PURE: bool = true;
    type ExprCurr = ExprInput<'vir>;
    type ExprNext = vir::ExprKind<'vir>;
    fn context(&self) -> GParams<'vir> {
        self.context
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
    fn ty_use_pure(&mut self, ty: ty::Ty<'vir>) -> TyUsePure<'vir> {
        self.ty_use(ty)
    }

    fn encode_place_snap(
        &mut self,
        place: Place<'vir>,
        curr_ver: &Self::EncodePlaceCtxt,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        Ok(self.encode_place_with_ref(curr_ver, place)?.snap)
    }

    fn encode_operand_snap(
        &mut self,
        operand: &mir::Operand<'vir>,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        Ok(match operand {
            mir::Operand::Copy(place) | mir::Operand::Move(place) => {
                self.encode_place_snap((*place).into(), curr_ver)?
            }
            mir::Operand::Constant(box constant) => {
                self.encode_constant_snap(constant)?.upcast_ty().lazy()
            }
            mir::Operand::RuntimeChecks(_) => todo!(),
        })
    }
}

impl<'vir: 'enc, 'enc> Enc<'vir, 'enc> {
    fn new(
        vcx: &'vir vir::VirCtxt<'vir>,
        encoding_depth: usize,
        def_id: DefId,
        kind: PureKind,
        gargs: GArgs<'vir>,
        body: &'enc mir::Body<'vir>,
        deps: &'enc mut TaskEncoderDependencies<'vir, MirPureEnc>,
    ) -> Self {
        let rev_doms = rev_doms::ReverseDominators::new(&body.basic_blocks);
        Self {
            vcx,
            encoding_depth,
            context: gargs.context(),
            context_def_id: match kind {
                PureKind::Closure { context } | PureKind::Spec { context, .. } => context,
                PureKind::Pure | PureKind::Constant(_) | PureKind::SpecBlock(_) => def_id,
            },
            body,
            rev_doms,
            spec_arms: SpecArms::for_body(def_id, body),
            deps,
            version_ctr: IndexVec::from_elem_n(0, body.local_decls.len()),
            uses: Default::default(),
            phi_ctr: 0,
            modes: Modes::empty(),
            // An impure method's specs and contained spec blocks get shallow
            // argument snapshots (and can take a snapshot of the predicates
            // behind `&mut`s). Pure functions currently also only get shallow
            // snapshots but cannot get the value behind `&mut`s.
            impure_context: matches!(
                kind,
                PureKind::Spec {
                    mode: MirSpecEncMode::Impure,
                    ..
                } | PureKind::SpecBlock(..)
            ),
            captures: matches!(kind, PureKind::Closure { .. }).then(|| {
                let closure_ty = body.local_decls[ty::CAPTURE_STRUCT_LOCAL].ty.peel_refs();
                let TyKind::Closure(_, args) = closure_ty.kind() else {
                    unreachable!("a closure body takes its closure first");
                };
                IndexVec::from_elem_n(FxHashSet::default(), args.as_closure().upvar_tys().len())
            }),
        }
    }

    fn ty_use(&mut self, ty: ty::Ty<'vir>) -> TyUsePure<'vir> {
        let ty_task = RustTyDecomposition::from_ty(ty, self.context);
        self.deps.require_dep::<TyUsePureEnc>(ty_task).unwrap()
    }

    fn get_ty_for_local(&mut self, local: mir::Local) -> vir::TypeSnap<'vir> {
        let ty = self.body.local_decls[local].ty;
        let ty_task = RustTyDecomposition::from_ty(ty, self.context);
        self.deps
            .require_ref::<TyUsePureEnc>(ty_task)
            .unwrap()
            .snapshot
    }

    /// The value that the body's inputs supply for `local` (see
    /// `ExprInput::inputs`).
    fn input(&mut self, local: mir::Local) -> ExprRet<'vir> {
        self.vcx.mk_lazy_expr(
            vir::vir_format!(self.vcx, "input {local:?}"),
            self.get_ty_for_local(local),
            Box::new(move |_vcx, lctx: ExprInput<'vir>| lctx.inputs[&local].kind),
        )
    }
}

mod rev_doms {
    /// Identical to `body.basic_blocks.dominators()` except in reverse. Since
    /// there may be multiple `Return`/`Unreachable`/etc. terminators, we add a
    /// special end block index which is invalid in `basic_blocks` but pretends
    /// to be the successor of all these no-successor blocks.
    pub struct ReverseDominators {
        pub dom: dominators::Dominators<mir::BasicBlock>,
        pub end: mir::BasicBlock,
    }
    impl ReverseDominators {
        #[allow(clippy::needless_lifetimes)]
        pub fn new<'a, 'vir>(blocks: &'a mir::BasicBlocks<'vir>) -> Self {
            let no_succ_blocks = blocks
                .iter_enumerated()
                .filter(|(_, data)| {
                    /* The commented line below prevents e.g. a SwitchInt with a branch
                    going to an Unreachable terminator from having a join point, so
                    do not treat such terminators as return blocks. */
                    // data.terminator().successors().next().is_none()
                    matches!(
                        data.terminator().kind,
                        mir::TerminatorKind::Return | mir::TerminatorKind::UnwindResume
                    )
                })
                .map(|(bb, _)| bb)
                .collect();
            let rbb = RevBasicBlocks(blocks, no_succ_blocks);
            Self {
                dom: dominators::dominators(&rbb),
                end: rbb.start_node(),
            }
        }
        pub fn immediate_dominator(&self, bb: mir::BasicBlock) -> mir::BasicBlock {
            // This unwrap should never fail since all blocks can reach `end`
            self.dom.immediate_dominator(bb).unwrap()
        }
    }

    use super::*;
    use prusti_rustc_interface::data_structures::graph::*;

    /// A wrapper around `mir::BasicBlocks` which reverses the direction of the
    /// edges. Implements `ControlFlowGraph` such that we can call `dominators`.
    struct RevBasicBlocks<'a, 'vir>(&'a mir::BasicBlocks<'vir>, Vec<mir::BasicBlock>);
    impl DirectedGraph for RevBasicBlocks<'_, '_> {
        type Node = mir::BasicBlock;

        fn num_nodes(&self) -> usize {
            self.0.len() + 1
        }
    }
    impl StartNode for RevBasicBlocks<'_, '_> {
        fn start_node(&self) -> Self::Node {
            self.0.next_index()
        }
    }

    impl Predecessors for RevBasicBlocks<'_, '_> {
        fn predecessors(&self, node: Self::Node) -> impl Iterator<Item = Self::Node> {
            if node == self.start_node() {
                Box::new([].into_iter())
            } else if self.1.contains(&node) {
                Box::new([self.start_node()].into_iter())
            } else {
                Box::new(self.0.successors(node)) as Box<dyn Iterator<Item = _>>
            }
        }
    }

    impl Successors for RevBasicBlocks<'_, '_> {
        fn successors(&self, node: Self::Node) -> impl Iterator<Item = Self::Node> {
            if node == self.start_node() {
                Box::new(self.1.iter().copied()) as Box<dyn Iterator<Item = _>>
            } else {
                Box::new((&self.0).predecessors(node)) as Box<dyn Iterator<Item = _>>
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Version<'vir> {
    index: usize,
    location: mir::Location,
    initialised: Option<vir::LocalDeclSnap<'vir>>,
}

impl<'vir> Default for Version<'vir> {
    fn default() -> Self {
        Self {
            index: 0,
            location: mir::Location::START,
            initialised: None,
        }
    }
}

impl<'vir> fmt::Display for Version<'vir> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if cfg!(debug_assertions) {
            write!(
                f,
                "{:?}_s{}_i{}",
                self.location.block, self.location.statement_index, self.index
            )
        } else {
            write!(f, "{}", self.index)
        }
    }
}
