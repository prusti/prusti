//! The encoding of MIR bodies as Viper expressions: of pure functions,
//! specifications, promoted constants, spec blocks, and the closures of
//! quantifiers and spec blocks.
//!
//! A body becomes one expression, a chain of `let`s in static single
//! assignment form: each assignment to a local binds a new version of it
//! (see `Version`), whose variable the later reads of the local refer to. The
//! walk over the control flow graph builds the chain region by region (see
//! `Fragment`). A conditional is a merge: its branches are encoded up to
//! their join point, the locals that some branch changes are bound as one
//! pair chosen by the branch conditions, and the locals continue as its
//! elements (see `Enc::merge`). An assertion is a merge whose branch for the
//! expected value is the rest of the region and whose other branch cannot
//! complete.
//!
//! The expression is built lazily: the values of its inputs (version 0 of
//! the arguments, or of every local of the enclosing body for a spec block),
//! the state `old` refers to, and a spec closure's captures are supplied when
//! it is instantiated (see `ExprInput`). The bindings of versions that nothing
//! reads, directly or through other bindings, are left out (see `Uses`).

use crate::encoders::{
    FunctionCallEnc, Mode, PrustiBuiltin, SpecBuiltin,
    custom::{PairUse, PairUseEnc},
    mir_fn::{CallTaskDescription, RustSignature, SpecArms},
    mir_shared::{EncodeResult, PureRvalueEnc, RustcIntrinsic, unsupported_opaque},
    ty::{
        RustTyDecomposition,
        generics::{GArgs, GParams},
        use_pure::{TyUsePure, TyUsePureEnc},
    },
};
use itertools::Itertools;
use pcg::utils::Place;
use prusti_interface::environment::body::MirBody;
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

use ssa::{Fragment, Uses, Versions};

pub struct MirPureEnc;

#[derive(Clone, Debug)]
pub enum MirPureEncError {}

/// The values of an encoded body's inputs.
pub type Inputs<'vir> = &'vir FxHashMap<mir::Local, vir::ExprSnap<'vir>>;
/// The values of a spec closure's captures, in the modes its body reads them
/// in (see `MirPureEncOutput::captures`): the captured places, encoded by the
/// enclosing body (see `Enc::encode_capture`).
type Captures<'vir> = &'vir IndexSlice<abi::FieldIdx, FxHashMap<Modes, EncodedPlace<'vir>>>;
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
    /// For a spec closure, the values of its captures, which are expressions
    /// of the enclosing body.
    captures: Captures<'vir>,
    /// For a spec closure, the instantiation of the body it is part of, which
    /// a read of a capture instantiates the capture's value with.
    enclosing: Option<&'vir ExprInput<'vir>>,
}

impl<'vir> ExprInput<'vir> {
    fn enclosing(&self) -> Self {
        *self
            .enclosing
            .expect("a spec closure is instantiated within its enclosing body")
    }
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
    /// The inputs the body reads (see `ExprInput::inputs`), which every
    /// instantiation supplies.
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
            enclosing: None,
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

impl Modes {
    fn of(mode: Mode) -> Self {
        match mode {
            Mode::Old => Self::OLD,
            Mode::Rel(0) => Self::REL0,
            Mode::Rel(_) => Self::REL1,
            Mode::BeforeExpiry => Self::BEFORE_EXPIRY,
        }
    }

    /// The modes that a read in `self` cannot be nested in: `self`, and for
    /// a `rel`, either execution.
    fn conflicting(self) -> Self {
        if self.intersects(Self::REL0 | Self::REL1) {
            Self::REL0 | Self::REL1
        } else {
            self
        }
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
        /// Whether the specification belongs to an impure function (see
        /// `Enc::impure_context`).
        impure: bool,
    },
    Pure,
    Constant(mir::Promoted),
    SpecBlock(mir::BasicBlock),
}

/// What the kind of a body decides about its encoding, apart from the body
/// itself (see `PureKind::body`) and the region of it that is encoded.
struct BodySetup {
    /// The item whose generic parameters the body's types are expressed in.
    context_def_id: DefId,
    impure_context: bool,
    captures: Option<IndexVec<abi::FieldIdx, FxHashSet<Modes>>>,
}

impl PureKind {
    /// The body of this kind that `def_id` holds, encoded with `gargs`.
    fn body<'vir>(
        self,
        vcx: &'vir vir::VirCtxt<'vir>,
        def_id: DefId,
        gargs: GArgs<'vir>,
    ) -> MirBody<'vir> {
        // A specification is instantiated with the arguments of the
        // encoding, which differ from its own when a trait's specification
        // is encoded for an `impl` of that trait. So is the closure of a
        // quantifier or `spec_block`, which is encoded in the context of
        // the expression it is part of. The other bodies are encoded
        // generically, so they are taken as stored.
        match self {
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
        }
    }

    fn setup(self, def_id: DefId, body: &mir::Body<'_>) -> BodySetup {
        BodySetup {
            context_def_id: match self {
                PureKind::Closure { context } | PureKind::Spec { context, .. } => context,
                PureKind::Pure | PureKind::Constant(_) | PureKind::SpecBlock(_) => def_id,
            },
            impure_context: matches!(
                self,
                PureKind::Spec { impure: true, .. } | PureKind::SpecBlock(..)
            ),
            captures: matches!(self, PureKind::Closure { .. }).then(|| {
                let closure_ty = body.local_decls[ty::CAPTURE_STRUCT_LOCAL].ty.peel_refs();
                let TyKind::Closure(_, args) = closure_ty.kind() else {
                    unreachable!("a closure body takes its closure first");
                };
                IndexVec::from_elem_n(FxHashSet::default(), args.as_closure().upvar_tys().len())
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MirPureEncTask<'vir> {
    /// How deeply the body is nested in the expression it becomes part of.
    /// It is part of every variable name of the body, so that the variables
    /// of a body inlined into another one do not clash with the other's.
    // TODO: depth of encoding should be in the lazy context rather than here;
    //   can we integrate the lazy context into the identifier system?
    pub encoding_depth: usize,
    pub kind: PureKind,
    /// The item whose MIR holds the body: the pure function, specification
    /// or closure itself, the item a constant is promoted from, or the
    /// function a spec block is in.
    pub def_id: DefId,
    /// The generic arguments the body is encoded with, together with the
    /// context whose parameters give them meaning.
    pub gargs: GArgs<'vir>,
}

impl TaskEncoder for MirPureEnc {
    task_encoder::encoder_cache!(MirPureEnc);
    const ENCODER_NAME: &'static str = "MIR pure encoder";

    type TaskDescription<'vir> = MirPureEncTask<'vir>;

    type TaskKey<'vir> = MirPureEncTask<'vir>;

    type OutputFullDependency<'vir> = MirPureEncOutput<'vir>;

    type EncodingError = MirPureEncError;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;

        let MirPureEncTask {
            encoding_depth,
            kind,
            def_id,
            gargs,
        } = *task_key;

        tracing::debug!("encoding {def_id:?}");
        let (inputs, expr, captures) = vir::with_vcx(move |vcx| {
            let body = kind.body(vcx, def_id, gargs);

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
            let setup = kind.setup(def_id, &body);
            let mut enc = Enc::new(vcx, encoding_depth, def_id, gargs, &body, setup, deps);
            let (expr_inner, snapshot) = vcx.with_span(body_span, |_| match kind {
                PureKind::SpecBlock(block) => enc
                    .encode_spec_block(block)
                    .map(|expr| (expr, vir::TYPE_BOOL.upcast_ty())),
                _ => enc
                    .encode_body()
                    .and_then(|expr| Ok((expr, enc.get_ty_for_local(mir::RETURN_PLACE)?))),
            })?;
            let captures = enc.captures.take().unwrap_or_default();
            let inputs = enc
                .uses
                .read
                .iter()
                .filter(|(l, v)| *l != mir::RETURN_PLACE && *v == 0)
                .map(|&(l, _v)| l)
                .sorted()
                .collect::<Vec<_>>();
            let inputs_expected = vcx.alloc_slice(&inputs);

            // The expression is wrapped in a lazy one that checks that it is
            // instantiated for this body, with every input it reads. It is
            // created under the body's span: this wrapper is the node whose
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

                        // check: are we providing every input the body reads?
                        assert!(inputs_expected.iter().all(|l| lctx.inputs.contains_key(l)));

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
    /// The index of the next merge, which makes its variable's name unique.
    phi_ctr: usize,
    /// The modes the walk is in: the mode markers it passes enter and leave
    /// them. A mode's markers enclose a whole expression, so every branch of
    /// a conditional ends in the modes it starts in.
    modes: Modes,
    /// Whether the body is a specification of an impure method or a spec
    /// block in one, whose heap holds the values behind `&mut` arguments: only
    /// there can a dereference of one read the value, through its predicate.
    impure_context: bool,
    /// For a spec closure, the modes its body reads each capture in (see
    /// [`MirPureEncOutput::captures`]); `None` for other bodies.
    captures: Option<IndexVec<abi::FieldIdx, FxHashSet<Modes>>>,
}

struct EncodedPlace<'vir> {
    snap: ExprRet<'vir>,
    /// The address of the place, if it is behind a dereference: the address
    /// that the reference or box holds, projected to the place. `None` for a
    /// place that exists only in this expression.
    place_ref: Option<ExprRetRef<'vir>>,
    /// The metadata for the pointed-to value. Set when going through a
    /// `ProjectionElem::Deref`.
    metadata: Option<ExprRet<'vir>>,
}

impl<'vir> EncodedPlace<'vir> {
    fn new(expr: ExprRet<'vir>, place_ref: Option<ExprRetRef<'vir>>) -> Self {
        Self {
            snap: expr,
            place_ref,
            metadata: None,
        }
    }

    fn with_metadata(mut self, metadata: ExprRet<'vir>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

impl<'vir: 'enc, 'enc> PureRvalueEnc<'vir> for Enc<'vir, 'enc> {
    type Encoder = MirPureEnc;
    type EncodePlaceCtxt = Versions<'vir>;
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
    fn encoding_depth(&self) -> usize {
        self.encoding_depth
    }
    fn ty_use_pure(&mut self, ty: ty::Ty<'vir>) -> TyUsePure<'vir> {
        self.ty_use(ty).unwrap()
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
        curr_ver: &Versions<'vir>,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        Ok(match operand {
            mir::Operand::Copy(place) | mir::Operand::Move(place) => {
                self.encode_place_snap((*place).into(), curr_ver)?
            }
            mir::Operand::Constant(box constant) => {
                self.encode_constant_snap(constant)?.upcast_ty().lazy()
            }
            mir::Operand::RuntimeChecks(_) => {
                return Err(EncodeFullError::unsupported(
                    format!("operand `{operand:?}` in pure code"),
                    vec![self.current_span()],
                ));
            }
        })
    }
}

impl<'vir: 'enc, 'enc> Enc<'vir, 'enc> {
    fn new(
        vcx: &'vir vir::VirCtxt<'vir>,
        encoding_depth: usize,
        def_id: DefId,
        gargs: GArgs<'vir>,
        body: &'enc mir::Body<'vir>,
        setup: BodySetup,
        deps: &'enc mut TaskEncoderDependencies<'vir, MirPureEnc>,
    ) -> Self {
        let BodySetup {
            context_def_id,
            impure_context,
            captures,
        } = setup;
        let rev_doms = rev_doms::ReverseDominators::new(&body.basic_blocks);
        Self {
            vcx,
            encoding_depth,
            context: gargs.context(),
            context_def_id,
            body,
            rev_doms,
            spec_arms: SpecArms::for_body(def_id, body),
            deps,
            version_ctr: IndexVec::from_elem_n(0, body.local_decls.len()),
            uses: Default::default(),
            phi_ctr: 0,
            modes: Modes::empty(),
            impure_context,
            captures,
        }
    }

    fn ty_use(&mut self, ty: ty::Ty<'vir>) -> EncodeResult<'vir, TyUsePure<'vir>, MirPureEnc> {
        let ty_task = RustTyDecomposition::from_ty(ty, self.context);
        self.deps.require_dep::<TyUsePureEnc>(ty_task)
    }

    fn get_ty_for_local(
        &mut self,
        local: mir::Local,
    ) -> EncodeResult<'vir, vir::TypeSnap<'vir>, MirPureEnc> {
        let ty = self.body.local_decls[local].ty;
        let ty_task = RustTyDecomposition::from_ty(ty, self.context);
        Ok(self.deps.require_ref::<TyUsePureEnc>(ty_task)?.snapshot)
    }

    /// The value that the body's inputs supply for `local` (see
    /// `ExprInput::inputs`).
    fn input(&mut self, local: mir::Local) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        Ok(self.vcx.mk_lazy_expr(
            vir::vir_format!(self.vcx, "input {local:?}"),
            self.get_ty_for_local(local)?,
            Box::new(move |_vcx, lctx: ExprInput<'vir>| lctx.inputs[&local].kind),
        ))
    }
}

mod rev_doms {
    /// The dominators of the reversed control flow graph, whose start is an
    /// extra block `end` succeeding every block that ends in `Return` or
    /// `UnwindResume`. A block that cannot complete is not connected to `end`,
    /// so that a switch with such an arm still joins its other arms.
    pub struct ReverseDominators {
        pub dom: dominators::Dominators<mir::BasicBlock>,
        pub end: mir::BasicBlock,
    }
    impl ReverseDominators {
        pub fn new(blocks: &mir::BasicBlocks<'_>) -> Self {
            let no_succ_blocks = blocks
                .iter_enumerated()
                .filter(|(_, data)| {
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
