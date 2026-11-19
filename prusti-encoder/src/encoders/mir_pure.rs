use crate::encoders::{
    FunctionCallEnc, Mode, PrustiBuiltin, SpecBuiltin,
    custom::{PairUse, PairUseEnc},
    mir_fn::{CallTaskDescription, RustSignature, SpecArms},
    mir_shared::{EncodeResult, PureRvalueEnc, RustcIntrinsic, unsupported_opaque},
    pure::spec::MirSpecEncMode,
    trigger_checks,
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
use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, CompType, add_debug_note};

pub struct MirPureEnc;

#[derive(Clone, Debug)]
pub enum MirPureEncError {
    // UnsupportedStatement,
    // UnsupportedTerminator,
}

/// The values of an encoded body's inputs.
pub type Inputs<'vir> = &'vir FxHashMap<mir::Local, vir::ExprSnap<'vir>>;
/// The values of a spec closure's captures, in the modes its body reads them
/// in (see `MirPureEncOutput::captures`).
type Captures<'vir> = &'vir IndexSlice<abi::FieldIdx, FxHashMap<Modes, vir::ExprSnap<'vir>>>;
/// What an encoded body is instantiated with: the body's `DefId`, the values
/// of its inputs, the state `old` refers to, and, for a spec closure, the
/// values of its captures.
type ExprInput<'vir> = (DefId, Inputs<'vir>, vir::OldLabel<'vir>, Captures<'vir>);
type ExprRet<'vir> = vir::ExprGenSnap<'vir, ExprInput<'vir>, vir::ExprKind<'vir>>;
type ExprRetRef<'vir> = vir::ExprGenRef<'vir, ExprInput<'vir>, vir::ExprKind<'vir>>;
type ExprRetAny<'vir, T> = vir::ExprGen<'vir, ExprInput<'vir>, vir::ExprKind<'vir>, T>;
/// An encoded spec closure (see `Enc::encode_spec_closure`): the quantified
/// variables derived from the closure's arguments, and the closure's body.
type SpecClosure<'vir> = (
    &'vir [vir::LocalDeclSnap<'vir>],
    ExprRetAny<'vir, vir::Bool>,
);

/// The encoded body of a spec closure, with what it is instantiated with
/// (see `Enc::encode_closure_body`).
#[derive(Clone, Copy)]
struct ClosureBody<'vir> {
    def_id: DefId,
    expr: ExprRet<'vir>,
    /// The values of the closure's arguments after the closure itself, the
    /// quantified variables.
    inputs: Inputs<'vir>,
    /// The values of the closure's captures in the modes its body reads them
    /// in, encoded in the body that builds the closure.
    captures: &'vir IndexVec<abi::FieldIdx, FxHashMap<Modes, ExprRet<'vir>>>,
}

impl<'vir> ClosureBody<'vir> {
    /// The body as part of the expression the closure is built in, in that
    /// expression's state, named `name` for debugging.
    fn embed(self, vcx: &'vir vir::VirCtxt<'vir>, name: &'vir str) -> ExprRet<'vir> {
        use vir::Reify;
        let ClosureBody {
            def_id,
            expr,
            inputs,
            captures,
        } = self;
        vcx.mk_lazy_expr(
            name,
            expr.ty(),
            Box::new(move |vcx, lctx: ExprInput<'vir>| {
                let captures = captures
                    .iter()
                    .map(|values| {
                        values
                            .iter()
                            .map(|(&modes, value)| (modes, value.reify(vcx, lctx)))
                            .collect()
                    })
                    .collect::<IndexVec<_, _>>();
                expr.reify(vcx, (def_id, inputs, lctx.2, vcx.alloc(captures)))
                    .kind
            }),
        )
    }

    /// The body as a term of its own, whose shape decides whether it is a
    /// valid trigger: the state of `old` is left open, and each capture is a
    /// local. That is what a capture is to the shape, a value from outside
    /// the quantifier, which mentions none of its variables.
    fn shape_term(self, vcx: &'vir vir::VirCtxt<'vir>) -> vir::ExprDyn<'vir> {
        use vir::Reify;
        let captures = self
            .captures
            .iter_enumerated()
            .map(|(field, values)| {
                values
                    .iter()
                    .map(|(&modes, value)| {
                        let name = vir::vir_format!(vcx, "capture_{}", field.as_usize());
                        (modes, vcx.mk_local_ex(vcx.mk_local_decl(name, value.ty())))
                    })
                    .collect()
            })
            .collect::<IndexVec<_, _>>();
        let captures: Captures<'vir> = vcx.alloc(captures);
        let input = (self.def_id, self.inputs, vir::OldLabel::None, captures);
        self.expr.reify(vcx, input).as_dyn()
    }
}
type TriggerRet<'vir> = vir::TriggerGen<'vir, ExprInput<'vir>, vir::ExprKind<'vir>>;

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
        let input = (self.def_id, inputs, old, IndexSlice::empty());
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
    /// A closure holding a trigger of a quantifier, encoded like `Closure`. A
    /// trigger is a pattern, whose value never becomes a fact, so the runtime
    /// checks of MIR (bounds, overflow, division by zero) are not encoded:
    /// Silicon evaluates a trigger that is not part of the body without the
    /// body's conditions, where the failing branch of a check would assume
    /// `false`.
    Trigger {
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
            // TODO
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
            // is encoded for an `impl` of that trait. So are the closures of a
            // quantifier's body and triggers and of `spec_block`, which are
            // encoded in the context of the expression they are part of. The
            // other bodies are encoded generically, so they are taken as
            // stored.
            let body = match kind {
                PureKind::Closure { context } | PureKind::Trigger { context } => {
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
                .unique()
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
                        assert_eq!(lctx.0, def_id);

                        // check: are we providing the expected number of inputs?
                        // TODO: check that the expected inputs are present; this
                        //   check is not precise
                        assert!(lctx.1.len() >= inputs_expected);

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
    Local(#[allow(dead_code)] mir::Local, Version<'vir>, ExprRet<'vir>),
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
    /// Whether the body is a trigger (see `PureKind::Trigger`).
    trigger: bool,
}

struct EncodedPlace<'vir> {
    snap: ExprRet<'vir>,
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

    fn encode_place_snap<'slf>(
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
                PureKind::Closure { context }
                | PureKind::Trigger { context }
                | PureKind::Spec { context, .. } => context,
                PureKind::Pure | PureKind::Constant(_) | PureKind::SpecBlock(_) => def_id,
            },
            body,
            rev_doms,
            spec_arms: SpecArms::for_body(def_id, body),
            deps,
            // visited: IndexVec::from_elem_n(false, body.basic_blocks.len()),
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
            captures: matches!(kind, PureKind::Closure { .. } | PureKind::Trigger { .. }).then(
                || {
                    let closure_ty = body.local_decls[ty::CAPTURE_STRUCT_LOCAL].ty.peel_refs();
                    let TyKind::Closure(_, args) = closure_ty.kind() else {
                        unreachable!("a closure body takes its closure first");
                    };
                    IndexVec::from_elem_n(FxHashSet::default(), args.as_closure().upvar_tys().len())
                },
            ),
            trigger: matches!(kind, PureKind::Trigger { .. }),
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

    fn mk_local_ex(&mut self, _local: mir::Local, version: Version<'vir>) -> ExprRet<'vir> {
        self.vcx.mk_local_ex(version.initialised.unwrap())
    }

    fn mk_phi(&self, version: Version<'vir>) -> &'vir str {
        vir::vir_format!(self.vcx, "_{}_phi_{}", self.encoding_depth, version)
    }

    fn bump_version(
        &mut self,
        update: &mut Update<'vir>,
        local: mir::Local,
        expr: ExprRet<'vir>,
        location: mir::Location,
    ) {
        if self.version_ctr[local] == 0 {
            // Version 0 signifies "inputs into the expression", as checked in
            // `do_encode_full` to figure out which locals should be encoded
            // and provided by the caller/user of the encoded expression. In
            // most cases, version 0 is not assigned to for non-input locals,
            // because a preceding `StorageLive` bumps the version before the
            // assignment takes place, and so any later reads of that local use
            // at least version 1. However, there are some operations, such as
            // checked binary operations, that do not cause the compiler to
            // emit a `StorageLive` for their target place. This workaround
            // checks for such a case and bumps the version twice.
            //
            // see: https://github.com/rust-lang/rust/issues/67400
            self.bump_version_no_assign(local, location);
        }

        let new_version = self.bump_version_no_assign(local, location);
        // check that `local` and `expr` type correspond
        update.assign(self.vcx, self.encoding_depth, local, new_version, expr);
    }

    fn bump_version_init(
        &mut self,
        update: &mut Update<'vir>,
        local: mir::Local,
        expr: ExprRet<'vir>,
        location: mir::Location,
    ) {
        let new_version = self.bump_version_no_assign(local, location);
        // check that `local` and `expr` type correspond
        update.assign(self.vcx, self.encoding_depth, local, new_version, expr);
    }

    fn bump_version_no_assign(
        &mut self,
        local: mir::Local,
        location: mir::Location,
    ) -> Version<'vir> {
        let index = self.version_ctr[local];
        self.version_ctr[local] += 1;
        Version {
            index,
            location,
            initialised: None,
        }
    }

    fn reify_binds<T: CompType>(
        &self,
        update: Update<'vir>,
        expr: ExprRetAny<'vir, T>,
    ) -> ExprRetAny<'vir, T> {
        update.binds.iter().rfold(expr, |expr, bind| match bind {
            UpdateBind::Local(local, version, val) => {
                // skip bindings which were not used
                // TODO: this might optimise away some function calls which
                //   can act as triggers for quantifiers
                if !self.uses.is_read(*local, version.index) {
                    expr
                } else {
                    let decl = version.initialised.unwrap();
                    self.vcx.mk_let_expr(decl, val, expr)
                }
            }
            UpdateBind::Phi(phi, val) => self.vcx.mk_let_expr(*phi, val, expr),
        })
    }

    fn reify_branch(
        &mut self,
        tuple_ref: &PairUse<'vir>,
        mod_locals: &[mir::Local],
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
        update: Option<Update<'vir>>,
    ) -> ExprRetAny<'vir, vir::Pair> {
        update
            .map(|update| {
                let tuple_args = mod_locals
                    .iter()
                    .map(|local| {
                        let version = update.versions.get(local).copied().unwrap_or_else(|| {
                            // TODO: remove (debug)
                            if !curr_ver.contains_key(local) {
                                tracing::error!("unknown version of local! {}", local.as_usize());
                                return Version {
                                    index: 0xff,
                                    ..Default::default()
                                };
                            }
                            curr_ver[local]
                        });

                        self.uses.read(*local, version.index);
                        self.mk_local_ex(*local, version)
                    })
                    .collect::<Vec<_>>();
                self.reify_binds(update, tuple_ref.mk_cons(&tuple_args))
            })
            .unwrap_or_else(|| {
                // A branch whose update is `None` cannot complete (it ends in
                // `unreachable` or a diverging call, or is the failing side
                // of an `Assert`). It merges the unreachable snapshots of the
                // locals' types, which assume `false` where they are
                // evaluated; with no locals to merge, nothing is assumed.
                let unreachable = mod_locals
                    .iter()
                    .map(|local| {
                        self.ty_use(self.body.local_decls[*local].ty)
                            .unreachable_to_snap()
                    })
                    .collect::<Vec<_>>();
                tuple_ref.mk_cons(&unreachable)
            })
    }

    fn encode_common(
        &mut self,
        start: mir::BasicBlock,
        end: mir::BasicBlock,
        local_count: usize,
        result_local: mir::Local,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let mut init = Update::new();
        let v0 = Version::default();
        // TODO: what about locals which never have StorageLive (i.e. always_live)?
        init.versions.insert(mir::RETURN_PLACE, v0);
        for local in 1..local_count {
            let local_ex = self.vcx.mk_lazy_expr(
                vir::vir_format!(self.vcx, "pure in _{local}"),
                self.get_ty_for_local(local.into()),
                Box::new(move |_vcx, lctx: ExprInput<'vir>| lctx.1[&local.into()].kind),
            );
            // check that `local` and `expr` type correspond
            self.bump_version_init(&mut init, local.into(), local_ex, v0.location);
        }

        let update = self.encode_cfg(&init.versions, start, end)?;

        // The body may unconditionally terminate with `unreachable` (e.g. a
        // `match` on an uninhabited type): its result is the unreachable snapshot.
        let Some(res) = init.merge(update) else {
            let result_ty = self.body.local_decls[result_local].ty;
            return Ok(self.ty_use(result_ty).unreachable_to_snap());
        };
        let ret_version = res.versions.get(&result_local).copied().unwrap_or(v0);
        self.uses.read(result_local, ret_version.index);

        let ex = self.mk_local_ex(result_local, ret_version);
        Ok(self.reify_binds(res, ex))
    }

    fn encode_body(&mut self) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        assert!(
            !graph::is_cyclic(&self.body.basic_blocks),
            "MIR pure encoding does not support loops"
        );
        self.encode_common(
            mir::START_BLOCK,
            self.rev_doms.end,
            self.body.arg_count + 1,
            mir::RETURN_PLACE,
        )
    }

    fn encode_spec_block(
        &mut self,
        block: mir::BasicBlock,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let Some(mir::TerminatorKind::Call { destination, .. }) = self.body.basic_blocks[block]
            .terminator
            .as_ref()
            .map(|t| &t.kind)
        else {
            unreachable!("malformed spec-only block: should end in a call terminator");
        };
        assert!(destination.projection.is_empty());
        self.encode_common(
            block,
            self.body.basic_blocks.successors(block).next().unwrap(),
            self.body.local_decls.len(),
            destination.local,
        )
    }

    fn encode_cfg(
        &mut self,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
        curr: mir::BasicBlock,
        join_point: mir::BasicBlock,
    ) -> EncodeResult<'vir, Option<Update<'vir>>, MirPureEnc> {
        if curr == join_point {
            // We are done with the current fragment of the CFG, the rest is
            // handled in a parent call.
            return Ok(Some(Update::new()));
        }

        // walk block statements first
        let mut new_curr_ver = curr_ver.clone();
        let mut stmt_update = Update::new();
        for (statement_index, stmt) in self.body[curr].statements.iter().enumerate() {
            let location = mir::Location {
                block: curr,
                statement_index,
            };
            let newer = self.encode_stmt(&new_curr_ver, stmt, location)?;
            newer.add_to_map(&mut new_curr_ver);
            stmt_update = stmt_update.merge_inner(newer);
        }

        // then walk terminator. Its operands and call are encoded under its
        // span, like statements in `encode_stmt`; the walks of its successors
        // are not, as they encode other blocks.
        let term = self.body[curr].terminator.as_ref().unwrap();
        let location = mir::Location {
            block: curr,
            statement_index: self.body[curr].statements.len(),
        };
        match &term.kind {
            &mir::TerminatorKind::Goto { target }
            | &mir::TerminatorKind::FalseEdge {
                real_target: target,
                ..
            }
            | &mir::TerminatorKind::Drop { target, .. } => {
                let rest_update = self.encode_cfg(&new_curr_ver, target, join_point)?;
                Ok(stmt_update.merge(rest_update))
            }

            mir::TerminatorKind::SwitchInt { discr, targets } => {
                // The `if false` switch of a `ghost!` block or a `closure!`
                // spec marker: continue straight into the live target (the
                // inline ghost body, or the continuation); the other arm is
                // skipped.
                if let Some(live) = self.spec_arms.switches.get(&curr) {
                    let rest_update = self.encode_cfg(&new_curr_ver, *live, join_point)?;
                    return Ok(stmt_update.merge(rest_update));
                }
                if self.trigger {
                    return Err(EncodeFullError::incorrect(
                        "a trigger cannot contain a conditional (`if`, `match`, `&&` or `||`)"
                            .to_string(),
                        term.source_info.span,
                    ));
                }

                // encode the discriminant operand
                let discr_expr = self
                    .vcx
                    .with_span(term.source_info.span, |_| {
                        self.encode_operand_snap(discr, &new_curr_ver)
                    })?
                    .downcast_ty();
                let discr_ty = discr.ty(self.body, self.vcx.tcx());
                let discr_ty_out = self.ty_use(discr_ty).expect_primitive();

                // walk `curr` -> `targets[i]` -> `join` for each target. The
                // join point the bb which is an immediate reverse dominator of
                // the branch point.
                // TODO: indexvec?
                let new_join_point = self.rev_doms.immediate_dominator(curr);
                let mut updates = targets
                    .all_targets()
                    .iter()
                    .map(|target| self.encode_cfg(&new_curr_ver, *target, new_join_point))
                    .collect::<Result<Vec<_>, _>>()?;

                // find locals updated in any of the results, which were also
                // defined before the branch
                let mut mod_locals = updates
                    .iter()
                    .flat_map(|update| update.iter().flat_map(|v| v.versions.keys()))
                    .filter(|local| new_curr_ver.contains_key(local))
                    .copied()
                    .collect::<Vec<_>>();
                mod_locals.sort();
                mod_locals.dedup();
                let mod_tys = mod_locals
                    .iter()
                    .map(|l| self.get_ty_for_local(*l).as_dyn())
                    .collect();

                // for each branch, create a Viper tuple of the updated locals
                let tuple_ref = self.deps.require_dep::<PairUseEnc>(mod_tys).unwrap();
                let otherwise_update = updates.pop().unwrap();
                let phi_expr = targets.iter().zip(updates).fold(
                    self.reify_branch(&tuple_ref, &mod_locals, &new_curr_ver, otherwise_update),
                    |expr, ((cond_val, _target), branch_update)| {
                        self.vcx.mk_ternary_expr(
                            self.vcx.mk_eq_expr(
                                discr_ty_out.snap_to_prim(discr_expr),
                                discr_ty_out.expr_from_bits(discr_ty, cond_val).lazy(),
                            ),
                            self.reify_branch(
                                &tuple_ref,
                                &mod_locals,
                                &new_curr_ver,
                                branch_update,
                            ),
                            expr,
                        )
                    },
                );

                // assign tuple into a `phi` variable
                let phi_idx = Version {
                    index: self.phi_ctr,
                    location,
                    initialised: None,
                };
                let phi = self.vcx.mk_local_decl(self.mk_phi(phi_idx), tuple_ref.ty);
                self.phi_ctr += 1;
                let mut phi_update = Update::new();
                phi_update.binds.push(UpdateBind::Phi(phi, phi_expr));

                // update locals by destructuring `phi` variable
                // TODO: maybe this is unnecessary, we could instead use tuple
                //   access directly instead of the locals going forward?
                for (elem_idx, local) in mod_locals.iter().enumerate() {
                    let expr = tuple_ref
                        .mk_elem(self.vcx.mk_local_ex(phi), elem_idx)
                        .downcast_ty();
                    self.bump_version(&mut phi_update, *local, expr, location);
                    new_curr_ver.insert(*local, phi_update.versions[local]);
                }

                // walk `join` -> `end`
                let end_update = self.encode_cfg(&new_curr_ver, new_join_point, join_point)?;
                Ok(stmt_update.merge(phi_update.merge(end_update)))
            }

            // TODO: there is some code duplication between here and SwitchInt
            mir::TerminatorKind::Assert {
                cond,
                expected,
                target,
                ..
            } => {
                // A trigger does not contain the checks (see
                // `PureKind::Trigger`).
                if self.trigger {
                    let rest_update = self.encode_cfg(&new_curr_ver, *target, join_point)?;
                    return Ok(stmt_update.merge(rest_update));
                }

                // encode the condition operand
                let cond_ty = cond.ty(self.body, self.vcx.tcx());
                assert_eq!(*cond_ty.kind(), TyKind::Bool);
                let cond_expr = self
                    .vcx
                    .with_span(term.source_info.span, |_| {
                        self.encode_operand_snap(cond, &new_curr_ver)
                    })?
                    .downcast_ty::<vir::Bool>();

                // if cond == expected: walk the rest of the CFG
                let ok_update = self.encode_cfg(&new_curr_ver, *target, join_point)?;

                // find locals updated in the "ok" branch, which were also
                // defined before the branch
                // TODO: is the unwrap here ok? can ok_update also be None?
                let mut mod_locals = ok_update
                    .as_ref()
                    .unwrap()
                    .versions
                    .keys()
                    .filter(|local| new_curr_ver.contains_key(local))
                    .copied()
                    .collect::<Vec<_>>();
                mod_locals.sort();
                mod_locals.dedup();
                let mod_tys = mod_locals
                    .iter()
                    .map(|l| self.get_ty_for_local(*l).as_dyn())
                    .collect();

                // create a Viper tuple of the updated locals
                let tuple_ref = self.deps.require_dep::<PairUseEnc>(mod_tys).unwrap();
                let phi_expr = self.vcx.mk_ternary_expr(
                    self.vcx.mk_eq_expr(
                        cond_expr,
                        self.vcx
                            .mk_const_expr(vir::ConstData::Bool(*expected))
                            .downcast_ty(),
                    ),
                    self.reify_branch(&tuple_ref, &mod_locals, &new_curr_ver, ok_update),
                    self.reify_branch(&tuple_ref, &mod_locals, &new_curr_ver, None),
                );

                // assign tuple into a `phi` variable
                let phi_idx = Version {
                    index: self.phi_ctr,
                    location,
                    initialised: None,
                };
                let phi = self.vcx.mk_local_decl(self.mk_phi(phi_idx), tuple_ref.ty);
                self.phi_ctr += 1;
                let mut phi_update = Update::new();
                phi_update.binds.push(UpdateBind::Phi(phi, phi_expr));

                // update locals by destructuring `phi` variable
                // TODO: maybe this is unnecessary, we could instead use tuple
                //   access directly instead of the locals going forward?
                for (elem_idx, local) in mod_locals.iter().enumerate() {
                    let expr = tuple_ref
                        .mk_elem(self.vcx.mk_local_ex(phi), elem_idx)
                        .downcast_ty();
                    self.bump_version(&mut phi_update, *local, expr, location);
                    new_curr_ver.insert(*local, phi_update.versions[local]);
                }

                Ok(stmt_update.merge(Some(phi_update)))
            }

            mir::TerminatorKind::Return => Ok(Some(stmt_update)),

            mir::TerminatorKind::Unreachable => {
                // will result in generating an unreachable phi merge
                Ok(None)
            }

            mir::TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                ..
            } => {
                let Some(target) = *target else {
                    // target is none if the call diverges (i.e. returns !), in
                    // this case treat it just like an unreachable terminator.
                    return Ok(None);
                };

                let func_ty = func.ty(self.body, self.vcx.tcx());
                let (def_id, arg_tys) = RustSignature::get_def_id_and_caller_substs(func_ty);
                let expr = self.vcx.with_span(term.source_info.span, |_| {
                    // Diverging calls were handled above; every other call
                    // must be to an intrinsic, a Prusti builtin or a pure
                    // function.
                    let is_pure = crate::encoders::is_function_pure(def_id, self.gargs(arg_tys));

                    // The bodiless `ptr_metadata` intrinsic is only lowered to
                    // `UnOp::PtrMetadata` in optimized MIR; do the lowering here.
                    let intrinsic = self.vcx.tcx().intrinsic(def_id);
                    let intrinsic = intrinsic.and_then(RustcIntrinsic::from_intrinsic);
                    if let Some(intrinsic) = intrinsic {
                        self.encode_intrinsic(intrinsic, arg_tys, args, &new_curr_ver)
                    } else if let Some(builtin) = PrustiBuiltin::new(def_id, self.gargs(arg_tys)) {
                        match self.encode_prusti_builtin(
                            builtin,
                            def_id,
                            self.gargs(arg_tys),
                            args,
                            term.source_info.span,
                            &new_curr_ver,
                        )? {
                            Some(expr) => Ok(expr),
                            // The pure-only builtins (quantifiers, spec blocks,
                            // mode markers).
                            None => {
                                let PrustiBuiltin::Spec(builtin) = builtin else {
                                    // Operand-based builtins are handled by
                                    // `PureRvalueEnc::encode_prusti_builtin` before this is reached.
                                    unreachable!(
                                        "operand-based builtin in the pure-only handler: {builtin:?}"
                                    )
                                };
                                self.encode_pure_only_builtin(
                                    builtin,
                                    arg_tys,
                                    args,
                                    curr,
                                    &new_curr_ver,
                                )
                            }
                        }
                    } else if is_pure {
                        let pure_func = self.deps.require_dep_spanned::<FunctionCallEnc>(
                            CallTaskDescription::new(self.context, arg_tys, def_id),
                            term.source_info.span,
                        )?;
                        let snap_args = args
                            .iter()
                            .map(|arg| self.encode_operand_snap(&arg.node, &new_curr_ver))
                            .collect::<Result<Vec<_>, _>>()?;
                        Ok(pure_func.call_pure(snap_args))
                    } else {
                        Err(EncodeFullError::incorrect(
                            format!(
                                "use of impure function `{}` in pure code is not allowed",
                                self.vcx.tcx().def_path_str(def_id)
                            ),
                            term.source_info.span,
                        ))
                    }
                });

                let expr = self.encode_place_update(
                    &new_curr_ver,
                    *destination,
                    expr?,
                    term.source_info.span,
                )?;
                let mut term_update = Update::new();
                self.bump_version(&mut term_update, destination.local, expr, location);
                term_update.add_to_map(&mut new_curr_ver);

                // walk rest of CFG
                let end_update = self.encode_cfg(&new_curr_ver, target, join_point)?;

                Ok(stmt_update.merge_inner(term_update).merge(end_update))
            }

            k => todo!("terminator kind {k:?}"),
        }
    }

    fn encode_stmt(
        &mut self,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
        stmt: &mir::Statement<'vir>,
        location: mir::Location,
    ) -> EncodeResult<'vir, Update<'vir>, MirPureEnc> {
        // Encoded under the statement's span: expressions pick up the ambient
        // span at creation, which makes error positions inside pure/spec
        // bodies point at the originating source (e.g. the failing conjunct
        // of a spec).
        self.vcx.with_span(stmt.source_info.span, |_| {
            let mut update = Update::new();
            match &stmt.kind {
                &mir::StatementKind::StorageLive(local) => {
                    let new_version = self.bump_version_no_assign(local, location);
                    update.versions.insert(local, new_version);
                }
                mir::StatementKind::StorageDead(..)
                | mir::StatementKind::FakeRead(..)
                | mir::StatementKind::AscribeUserType(..)
                | mir::StatementKind::PlaceMention(..)
                | mir::StatementKind::Coverage(..)
                | mir::StatementKind::ConstEvalCounter
                | mir::StatementKind::Nop
                | mir::StatementKind::BackwardIncompatibleDropHint { .. } => {} // nop
                mir::StatementKind::Assign(box (dest, rvalue)) => {
                    let span = stmt.source_info.span;
                    self.uses.start_assignment();
                    let expr = self
                        .encode_rvalue(curr_ver, rvalue, span)
                        .and_then(|expr| self.encode_place_update(curr_ver, *dest, expr, span));
                    let reads = self.uses.end_assignment();
                    self.bump_version(&mut update, dest.local, expr?, location);
                    let version = update.versions[&dest.local].index;
                    self.uses.assigned(dest.local, version, reads);
                }
                k => todo!("statement kind {k:?}"),
            }
            Ok(update)
        })
    }

    fn encode_rvalue(
        &mut self,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
        rvalue: &mir::Rvalue<'vir>,
        span: Span,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let rvalue_ty = rvalue.ty(self.body, self.vcx.tcx());
        match rvalue {
            mir::Rvalue::Use(op, _) => self.encode_operand_snap(op, curr_ver),
            mir::Rvalue::Ref(_, kind, place) => {
                let rvalue_snapshot_encoding = self.ty_use(rvalue_ty);
                // Reading a mutable reference's referent needs an address
                // holding a predicate, which pure-created mutrefs don't have.
                if kind.mutability().is_mut() {
                    return Err(EncodeFullError::unsupported(
                        format!(
                            "mutable borrow of `{}` in a specification: take a shared \
                             reference (`&`) instead",
                            place.ty(self.body, self.vcx.tcx()).ty
                        ),
                        vec![self.current_span()],
                    ));
                }
                let encoded_place = self.encode_place_with_ref(curr_ver, (*place).into())?;
                // We want to distinguish if `place` is a value that lives
                // in pure code or not. If it lives in impure (the only way
                // that this can happen is that we have a `&mut` argument)
                // then we want to return the actual address in the
                // snapshot. Otherwise we want to use `null` as this value
                // should never escape pure code anyway. Thus `place_ref`
                // will return `None` if this isn't a re-borrow, and if it's
                // a re-borrow of created-in-pure reference then it will be
                // field projections of `null` which is also `null`.
                let place_ref = encoded_place
                    .place_ref
                    .unwrap_or_else(|| self.vcx.mk_null().lazy());
                let metadata = encoded_place
                    .metadata
                    .unwrap_or_else(|| self.expect_thin_ptr_metadata(rvalue_ty));
                let snap = if kind.mutability().is_mut() {
                    let e_rvalue_ty = rvalue_snapshot_encoding.expect_mutref();
                    e_rvalue_ty.prim_to_snap(place_ref, metadata, encoded_place.snap)
                } else {
                    let e_rvalue_ty = rvalue_snapshot_encoding.expect_immref();
                    e_rvalue_ty.prim_to_snap(place_ref, metadata, encoded_place.snap)
                };
                Ok(snap.upcast_ty())
            }
            mir::Rvalue::BinaryOp(op, box (l, r)) => {
                self.encode_binop_snap(rvalue_ty, *op, l, r, curr_ver, span)
            }
            mir::Rvalue::UnaryOp(unop, operand) => {
                self.encode_unary_op_snap(rvalue_ty, *unop, operand, curr_ver)
            }
            mir::Rvalue::Aggregate(
                box kind @ (mir::AggregateKind::Adt(..)
                | mir::AggregateKind::Tuple
                | mir::AggregateKind::Closure(..)),
                fields,
            ) => self.encode_aggregate_snap(rvalue_ty, kind, fields, curr_ver),
            mir::Rvalue::Aggregate(box mir::AggregateKind::Array(..), elements) => {
                self.encode_array_snap(rvalue_ty, elements, curr_ver)
            }
            mir::Rvalue::Repeat(operand, len) => {
                self.encode_repeat_snap(rvalue_ty, operand, *len, curr_ver)
            }
            mir::Rvalue::Discriminant(place) => {
                let place_ty = place.ty(self.body, self.vcx.tcx());
                let ty = self.ty_use(place_ty.ty);
                if ty.specifics.is_opaque() {
                    return Err(unsupported_opaque(place_ty.ty, span));
                }
                let discr = match ty
                    .get_enumlike()
                    .filter(|_| place_ty.variant_index.is_none())
                {
                    Some(ty) => ty.snap_to_discr_snap(
                        self.encode_place_snap((*place).into(), curr_ver)?
                            .downcast_ty(),
                    ),
                    None => {
                        let e_rvalue_ty = self.ty_use(rvalue_ty).expect_primitive();
                        // mir::Rvalue::Discriminant documents "Returns zero for types without discriminant"
                        let zero = self.vcx.mk_uint::<0>();
                        e_rvalue_ty.prim_to_snap(zero.upcast_ty()).lazy()
                    }
                };
                Ok(discr.upcast_ty())
            }
            mir::Rvalue::Cast(kind, operand, ty) => {
                assert_eq!(*ty, rvalue_ty);
                Ok(self
                    .encode_cast_snap(rvalue_ty, *kind, operand, curr_ver)?
                    .1)
            }
            mir::Rvalue::RawPtr(_, place) => {
                let encoded_place = self.encode_place_with_ref(curr_ver, (*place).into())?;
                // As for `Rvalue::Ref`: a raw pointer built in pure code never
                // escapes, so its address is `null` unless it re-borrows the
                // place of an impure `&mut` argument.
                let place_ref = encoded_place
                    .place_ref
                    .unwrap_or_else(|| self.vcx.mk_null().lazy());
                let metadata = encoded_place
                    .metadata
                    .unwrap_or_else(|| self.expect_thin_ptr_metadata(rvalue_ty));
                let raw_ty = self.ty_use(rvalue_ty);
                Ok(raw_ty
                    .expect_raw()
                    .prim_to_snap(place_ref, metadata)
                    .upcast_ty())
            }
            _ => Err(EncodeFullError::unsupported(
                format!("unsupported rvalue {rvalue:?} in pure code"),
                vec![span],
            )),
        }
    }

    fn encode_place_element(
        &mut self,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
        place_ty: mir::PlaceTy<'vir>,
        elem: mir::PlaceElem<'vir>,
        encoded_place: EncodedPlace<'vir>,
    ) -> EncodeResult<'vir, EncodedPlace<'vir>, MirPureEnc> {
        let e_ty = self.ty_use(place_ty.ty);
        Ok(match elem {
            mir::ProjectionElem::Deref => {
                assert!(place_ty.variant_index.is_none());
                match place_ty.ty.kind() {
                    TyKind::Adt(adt, _) if adt.is_box() => {
                        // The boxed value, its address and its pointer metadata
                        // are all read out of the snapshot.
                        let data = e_ty.expect_structlike();
                        let snap = encoded_place.snap.downcast_ty();
                        let val_expr = data.box_value_access(snap);
                        let place_ref = Some(data.box_address_access(snap));
                        let metadata = data.box_metadata_access(snap);
                        EncodedPlace::new(val_expr, place_ref).with_metadata(metadata)
                    }
                    TyKind::Ref(.., ty::Mutability::Not) => {
                        let e_ty = e_ty.expect_immref();
                        let snap = encoded_place.snap.downcast_ty();
                        let metadata = e_ty.metadata_access(snap);
                        let val_expr = e_ty.value_access(snap);
                        let ref_expr = e_ty.addr_access(snap);
                        EncodedPlace::new(val_expr, Some(ref_expr)).with_metadata(metadata)
                    }
                    TyKind::Ref(.., ty::Mutability::Mut) => {
                        let e_ty = e_ty.expect_mutref();
                        let snap = encoded_place.snap.downcast_ty();
                        let metadata = e_ty.metadata_access(snap);
                        let ref_expr = e_ty.deref_access(snap);
                        if !self.impure_context {
                            // The value field of a mutable reference's snapshot
                            // is never constrained to the referent (see
                            // `p_Ref_mutable_arbitrary_value`).
                            return Err(EncodeFullError::unsupported(
                                format!(
                                    "dereference of the mutable reference `{}` in pure code",
                                    place_ty.ty
                                ),
                                vec![self.current_span()],
                            ));
                        }
                        let val_expr = {
                            // The snapshot is shallow and doesn't contain the
                            // value behind the mutable reference, so we need to
                            // take an extra snapshot here.
                            // TODO: avoid all of this by using shallow and deep snapshots
                            let ty_task = RustTyDecomposition::from_ty(place_ty.ty, self.context);
                            let inner = ty_task.ty.expect_mutref();
                            let normalized = inner
                                .referent
                                .decompose_compare_normalize(ty_task.ty.params, ty_task.args);
                            let caster = self
                                .deps
                                .require_dep::<crate::GArgsCastEnc<crate::Pure>>(normalized)
                                .unwrap();
                            let inner_ty_task = inner
                                .referent
                                .decompose_context(ty_task.ty.params, ty_task.args);
                            let inner_ty = self
                                .deps
                                .require_dep::<crate::encoders::TyUseImpureEnc>(inner_ty_task)
                                .unwrap();
                            caster.cast_to_caller_ctx(inner_ty.ref_to_snap(ref_expr))
                        };
                        EncodedPlace::new(val_expr, Some(ref_expr)).with_metadata(metadata)
                    }
                    TyKind::RawPtr(..) => {
                        return Err(EncodeFullError::unsupported(
                            format!("dereference of the raw pointer `{}`", place_ty.ty),
                            vec![self.current_span()],
                        ));
                    }
                    _ => unreachable!(),
                }
            }
            mir::ProjectionElem::Field(field_idx, _) => {
                if e_ty.specifics.is_opaque() {
                    return Err(unsupported_opaque(place_ty.ty, self.current_span()));
                }
                let variant = e_ty.expect_variant_opt(place_ty.variant_index);
                let proj = variant[field_idx];
                let proj_app = proj.read(encoded_place.snap.downcast_ty());
                let place_ref = encoded_place.place_ref.map(|pr| proj.field_ref(pr));
                let place = EncodedPlace::new(proj_app, place_ref);
                // Only the last field can be an unsized DST tail that shares the
                // containing value's pointer metadata; propagate it there, and
                // nowhere else (every other field is sized and thin).
                let is_last_field = field_idx.index() + 1 == variant.fields.len();
                match encoded_place.metadata {
                    Some(metadata) if is_last_field => place.with_metadata(metadata),
                    _ => place,
                }
            }
            mir::ProjectionElem::Index(idx) => {
                let proj = e_ty.expect_array();
                let idx = self
                    .encode_place_with_ref(curr_ver, mir::Place::from(idx).into())?
                    .snap
                    .downcast_ty();
                let proj_app = proj.index(encoded_place.snap.downcast_ty(), idx);
                let place_ref = encoded_place
                    .place_ref
                    .map(|pr| proj.ref_to_index_ref(pr, idx));
                EncodedPlace::new(proj_app, place_ref)
            }
            // `from_end` indexes from the runtime length of a slice; it is
            // always false for an array, whose statically known length lets
            // the compiler resolve the offset. Only the array form is
            // supported (a slice is matched behind a reference and its
            // length is not in the snapshot's type).
            mir::ProjectionElem::ConstantIndex {
                offset,
                from_end: false,
                ..
            } => {
                let proj = e_ty.expect_array();
                let idx = self.usize_const_snap(offset);
                let proj_app = proj.index(encoded_place.snap.downcast_ty(), idx);
                let place_ref = encoded_place
                    .place_ref
                    .map(|pr| proj.ref_to_index_ref(pr, idx));
                EncodedPlace::new(proj_app, place_ref)
            }
            mir::ProjectionElem::Downcast(..) => encoded_place,
            _ => todo!("Unsupported ProjectionElem {:?}", elem),
        })
    }

    fn in_mode(&self) -> bool {
        !self.modes.is_empty()
    }

    fn encode_place_with_ref(
        &mut self,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
        place: Place<'vir>,
    ) -> EncodeResult<'vir, EncodedPlace<'vir>, MirPureEnc> {
        // TODO: remove (debug)
        assert!(curr_ver.contains_key(&place.local));

        let mut place_ty = mir::PlaceTy::from_ty(self.body.local_decls[place.local].ty);

        // A spec closure's capture `(*_1).k` is the value the enclosing body
        // supplies for capture `k` in the modes of this read (see
        // `Enc::encode_spec_closure`); the closure value itself is never read.
        let (capture, projection): (_, &[mir::PlaceElem<'vir>]) = if let Some(captures) =
            &mut self.captures
            && place.local == ty::CAPTURE_STRUCT_LOCAL
        {
            let [
                mir::ProjectionElem::Deref,
                mir::ProjectionElem::Field(field, _),
                ..,
            ] = place.projection[..]
            else {
                unreachable!("an `Fn` closure body reads a capture `k` only as `(*_1).k`");
            };
            captures[field].insert(self.modes);
            (Some(field), &place.projection[2..])
        } else {
            self.uses.read(place.local, curr_ver[&place.local].index);
            (None, place.projection)
        };

        let should_wrap = capture.is_none() && self.in_mode() && {
            let local_kind = self.body.local_kind(place.local);
            local_kind == mir::LocalKind::Arg || local_kind == mir::LocalKind::ReturnPointer
        };

        let expr = if let Some(field) = capture {
            for elem in &place.projection[..2] {
                place_ty = place_ty.projection_ty(self.vcx.tcx(), *elem);
            }
            let modes = self.modes;
            self.vcx.mk_lazy_expr(
                vir::vir_format!(self.vcx, "capture {field:?}"),
                self.ty_use(place_ty.ty).snapshot,
                Box::new(move |_vcx, lctx: ExprInput<'vir>| lctx.3[field][&modes].kind),
            )
        } else if should_wrap {
            self.vcx.mk_lazy_expr(
                vir::vir_format!(self.vcx, "wrapped in {:?}", place.local),
                self.get_ty_for_local(place.local),
                Box::new(move |_vcx, lctx: ExprInput<'vir>| lctx.1[&place.local].kind),
            )
        } else {
            self.mk_local_ex(place.local, curr_ver[&place.local])
        };
        let mut encoded_place = EncodedPlace::new(expr, None);
        // TODO: factor this out (duplication with impure encoder)?
        for elem in projection {
            // The snapshots of the arguments and the result are pinned to a
            // state, so the only read of the heap is the dereference of a
            // mutable reference in an impure context; only that is wrapped.
            // An `old` around a pinned snapshot is a no-op, but breaks the
            // match with the callee's wand at a call site, where the `old` is
            // labelled. Silicon evaluates the heap-independent parts of a wand
            // when inhaling it, where `old[lhs]` is undefined.
            let heap_read = should_wrap
                && self.impure_context
                && matches!(elem, mir::ProjectionElem::Deref)
                && matches!(place_ty.ty.kind(), TyKind::Ref(.., ty::Mutability::Mut));
            encoded_place = self.encode_place_element(curr_ver, place_ty, *elem, encoded_place)?;
            place_ty = place_ty.projection_ty(self.vcx.tcx(), *elem);
            if heap_read && self.modes.contains(Modes::OLD) {
                let inner = encoded_place.snap;
                encoded_place.snap = self.vcx.mk_lazy_expr(
                    "old_mode_wrap",
                    inner.ty(),
                    Box::new(move |vcx, lctx: ExprInput<'vir>| {
                        use vir::Reify;
                        let reified = inner.reify(vcx, lctx);
                        vcx.mk_old(reified, lctx.2).kind
                    }),
                );
            }
            if heap_read && self.modes.contains(Modes::BEFORE_EXPIRY) {
                encoded_place.snap = self.vcx.mk_old_lhs_expr(encoded_place.snap);
            }
        }
        // Can we ever have the use of a projected place?
        assert!(place_ty.variant_index.is_none());

        if should_wrap {
            if self.modes.contains(Modes::REL0) {
                encoded_place.snap = self.vcx.mk_rel_expr(encoded_place.snap, 0);
            }
            if self.modes.contains(Modes::REL1) {
                encoded_place.snap = self.vcx.mk_rel_expr(encoded_place.snap, 1);
            }
        }

        Ok(encoded_place)
    }

    /// The snapshot of `place.local` after `value` is written to `place`: the
    /// old snapshot with only the part at `place` replaced.
    fn encode_place_update(
        &mut self,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
        place: mir::Place<'vir>,
        mut value: ExprRet<'vir>,
        span: Span,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        let Some((_, prefix)) = place.projection.split_last() else {
            return Ok(value);
        };
        // The type and snapshot of each place that `place` projects from.
        let mut base_ty = mir::PlaceTy::from_ty(self.body.local_decls[place.local].ty);
        let mut base =
            self.encode_place_with_ref(curr_ver, mir::Place::from(place.local).into())?;
        let mut bases = vec![(base_ty, base.snap)];
        for &elem in prefix {
            base = self.encode_place_element(curr_ver, base_ty, elem, base)?;
            base_ty = base_ty.projection_ty(self.vcx.tcx(), elem);
            bases.push((base_ty, base.snap));
        }
        for ((base_ty, base_snap), &elem) in bases.into_iter().zip(&place.projection[..]).rev() {
            let e_ty = self.ty_use(base_ty.ty);
            value = match elem {
                mir::ProjectionElem::Downcast(..) => value,
                mir::ProjectionElem::Field(..) if e_ty.specifics.is_opaque() => {
                    return Err(unsupported_opaque(base_ty.ty, span));
                }
                mir::ProjectionElem::Field(field_idx, _) => e_ty
                    .expect_variant_opt(base_ty.variant_index)
                    .with_field(base_snap.downcast_ty(), field_idx, value)
                    .upcast_ty(),
                mir::ProjectionElem::Deref if base_ty.ty.is_box() => e_ty
                    .expect_structlike()
                    .with_box_value(base_snap.downcast_ty(), value)
                    .upcast_ty(),
                // TODO: array elements need a function on the array snapshot
                // that replaces one element; writes through a `&mut` need its
                // referent's value, which pure code does not have.
                _ => {
                    return Err(EncodeFullError::unsupported(
                        format!(
                            "assignment to a part of a `{}` in pure code: only fields and \
                             boxed values can be assigned to",
                            base_ty.ty
                        ),
                        vec![span],
                    ));
                }
            };
        }
        Ok(value)
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

        let kind = PureKind::Closure {
            context: self.context_def_id,
        };
        let body =
            self.encode_closure_body(name, kind, cl_def_id, cl_args, qvars, block, curr_ver)?;
        let body = body.embed(
            self.vcx,
            vir::vir_format!(self.vcx, "spec closure body ({name})"),
        );
        Ok((qvars, body.downcast_ty::<vir::Bool>()))
    }

    /// Encodes the body of the spec closure `cl_def_id` with the generic
    /// arguments `cl_args`, as a body of `kind` whose arguments after the
    /// closure itself are the quantified variables `qvars`, together with its
    /// captures (see [`Self::encode_spec_closure`]). `name` is the builtin's
    /// `prusti_contracts` name, for error reporting.
    #[allow(clippy::too_many_arguments)]
    fn encode_closure_body(
        &mut self,
        name: &str,
        kind: PureKind,
        cl_def_id: DefId,
        cl_args: ty::GenericArgsRef<'vir>,
        qvars: &'vir [vir::LocalDeclSnap<'vir>],
        block: mir::BasicBlock,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
    ) -> EncodeResult<'vir, ClosureBody<'vir>, MirPureEnc> {
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

        let out = self.deps.require_dep::<MirPureEnc>(MirPureEncTask {
            encoding_depth: self.encoding_depth + 1,
            kind,
            parent_def_id: cl_def_id,
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
        Ok(ClosureBody {
            def_id: cl_def_id,
            expr: out.expr,
            inputs: self.vcx.alloc(reify_args),
            captures: self.vcx.alloc(captures),
        })
    }

    /// Encodes the trigger sets of a quantifier builtin over `qvars`, whose
    /// type `sets_ty` is a tuple of trigger sets, each itself a tuple of
    /// references to spec closures whose arguments are the quantified
    /// variables and whose body is one trigger expression. Like the body
    /// closure, the trigger closures are built in `block` and get their
    /// captures from it (see [`Self::encode_spec_closure`]). Reports a term or
    /// a set that is not a valid trigger (see `trigger_checks`).
    fn encode_trigger_sets(
        &mut self,
        name: &str,
        sets_ty: ty::Ty<'vir>,
        qvars: &'vir [vir::LocalDeclSnap<'vir>],
        block: mir::BasicBlock,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
    ) -> EncodeResult<'vir, &'vir [TriggerRet<'vir>], MirPureEnc> {
        let TyKind::Tuple(set_tys) = sets_ty.kind() else {
            panic!("illegal prusti::{name}: expected a trigger-set tuple, got {sets_ty:?}")
        };
        let mut sets = Vec::with_capacity(set_tys.len());
        for set_ty in set_tys.iter() {
            let TyKind::Tuple(cl_tys) = set_ty.kind() else {
                panic!("illegal prusti::{name}: expected a trigger tuple, got {set_ty:?}")
            };
            let mut exprs = Vec::with_capacity(cl_tys.len());
            let mut shapes = Vec::with_capacity(cl_tys.len());
            let mut closures = Vec::with_capacity(cl_tys.len());
            for cl_ty in cl_tys.iter() {
                let &TyKind::Ref(_, cl_ty, _) = cl_ty.kind() else {
                    panic!("illegal prusti::{name}: expected a trigger closure, got {cl_ty:?}")
                };
                let &TyKind::Closure(cl_def_id, cl_args) = cl_ty.kind() else {
                    panic!("illegal prusti::{name}: expected a trigger closure, got {cl_ty:?}")
                };
                let cl_span = self.vcx.tcx().def_span(cl_def_id);
                let kind = PureKind::Trigger {
                    context: self.context_def_id,
                };
                let body = self
                    .encode_closure_body(name, kind, cl_def_id, cl_args, qvars, block, curr_ver)?;
                shapes.push(vir::trigger_term_shape(vir::inline_lets(
                    self.vcx,
                    body.shape_term(self.vcx),
                )));
                // Created under the trigger's span, so that the reified term
                // carries it and the verifier's warnings about the trigger
                // point at it.
                let trigger = self.vcx.with_span(cl_span, |vcx| {
                    body.embed(vcx, vir::vir_format!(vcx, "trigger ({name})"))
                });
                exprs.push(trigger);
                closures.push(cl_def_id);
            }
            trigger_checks::check_set(self.vcx, qvars, &closures, &shapes, self.current_span())?;
            sets.push(self.vcx.mk_trigger(&exprs));
        }
        Ok(self.vcx.alloc_slice(&sets))
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
    /// encoded from that value, so that the modes reach the read of `arr`
    /// inside it. A capture by value is the captured place itself.
    fn encode_capture(
        &mut self,
        capture: &mir::Operand<'vir>,
        modes: Modes,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
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
            Some(value) => self.encode_rvalue(curr_ver, value, self.current_span()),
            None => self.encode_operand_snap(capture, curr_ver),
        };
        self.modes = outer_modes;
        res
    }

    /// Encodes the pure-only `prusti_contracts` builtins (quantifiers, spec
    /// blocks, and the old/rel/before-expiry mode markers), which need this
    /// encoder's own state. The operand-based builtins are handled by the
    /// shared [`PureRvalueEnc::encode_prusti_builtin`].
    fn encode_pure_only_builtin(
        &mut self,
        builtin: SpecBuiltin,
        arg_tys: ty::GenericArgsRef<'vir>,
        args: &[Spanned<mir::Operand<'vir>>],
        block: mir::BasicBlock,
        curr_ver: &FxHashMap<mir::Local, Version<'vir>>,
    ) -> EncodeResult<'vir, ExprRet<'vir>, MirPureEnc> {
        if self.trigger {
            let span = self.current_span();
            match builtin {
                SpecBuiltin::Forall | SpecBuiltin::Exists => {
                    return Err(EncodeFullError::incorrect(
                        "a trigger cannot contain a quantifier".to_string(),
                        span,
                    ));
                }
                SpecBuiltin::SpecBlock => {
                    return Err(EncodeFullError::incorrect(
                        "a trigger cannot contain a specification block".to_string(),
                        span,
                    ));
                }
                // Viper allows `old` in a trigger; the other states have no
                // encoding in a trigger yet.
                SpecBuiltin::ModeStart(Mode::Old) | SpecBuiltin::ModeEnd(Mode::Old) => {}
                SpecBuiltin::ModeStart(_) | SpecBuiltin::ModeEnd(_) => {
                    return Err(EncodeFullError::unsupported(
                        "`rel` and `before_expiry` in a trigger".to_string(),
                        vec![span],
                    ));
                }
            }
        }
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
                let triggers =
                    self.encode_trigger_sets(name, arg_tys[0].expect_ty(), qvars, block, curr_ver)?;
                let res = if builtin == SpecBuiltin::Forall {
                    self.vcx.mk_forall_expr(qvars, triggers, body)
                } else {
                    self.vcx.mk_exists_expr(qvars, triggers, body)
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
