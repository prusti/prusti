use prusti_rustc_interface::{
    index::Idx,
    middle::{mir, ty},
};
use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::CastType;

use crate::encoders::{
    builtin::{
        MetadataCastAxiomEnc, ValueCastAxiomEnc,
        cast::{CastTyArgs, MirBuiltinCastEnc, MirBuiltinCastOutput, MirBuiltinCastTask},
    },
    ty::{
        RustTyDecomposition, RustTySpecial, TySpecifics,
        generics::{
            GArgs, GArgsTy, GArgsTyEnc,
            lifetime_position::{LifetimePosition, LifetimePositionEnc},
            merge_split::{MergeSplit, MergeSplitEnc, MergeTarget},
        },
        indirect_nodes::NodePredicates,
    },
};

/// See `MirBuiltinCastEnc`. This adds a wrapper which substitutes in the
/// concrete type arguments.
pub struct MirBuiltinUseCastEnc;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct MirBuiltinUseCastTask<'vir> {
    result_ty: RustTyDecomposition<'vir>,
    kind: mir::CastKind,
    operand_ty: RustTyDecomposition<'vir>,
}

impl<'vir> MirBuiltinUseCastTask<'vir> {
    pub fn new(
        result_ty: RustTyDecomposition<'vir>,
        kind: mir::CastKind,
        operand_ty: RustTyDecomposition<'vir>,
    ) -> Self {
        // Canonicalize away the `CoercionSource`: whether a coercion was
        // written as an explicit `as` cast or inserted implicitly does not
        // affect the encoding, but as part of the task key it would create two
        // tasks emitting the same Viper function (a duplicate-identifier
        // consistency error).
        let kind = match kind {
            mir::CastKind::PointerCoercion(coercion, _) => {
                mir::CastKind::PointerCoercion(coercion, mir::CoercionSource::Implicit)
            }
            other => other,
        };
        Self {
            result_ty,
            kind,
            operand_ty,
        }
    }
}

/// A builtin cast with its type arguments at one use (see `CastTyArgs`).
#[derive(Debug, Clone, Copy)]
pub struct MirBuiltinUseCastOutput<'vir> {
    builtin: MirBuiltinCastOutput<'vir>,
    ty_args: GArgsTy<'vir>,
    /// For a `&mut` unsizing coercion, the referent blocks of the operand and
    /// of the result, where the referent has lifetimes.
    referent_blocks: [Option<ReferentBlock<'vir>>; 2],
}

/// The positions of the lifetimes in the referent of a `&mut`, which follow the
/// reference's own position. The `unsize`/`undo` methods take the predicates of
/// a generic `&mut U`, which has one position for them, so a use merges them
/// into one predicate before and splits them after, as a call does. Without
/// lifetimes in the referent, that position holds nothing.
#[derive(Debug, Clone, Copy)]
struct ReferentBlock<'vir> {
    ty: RustTyDecomposition<'vir>,
    merge_split: MergeSplit<'vir>,
    /// The type arguments of the merge/split methods, which are shared by the
    /// instantiations with the same canonical form.
    args: GArgsTy<'vir>,
}

impl<'vir> ReferentBlock<'vir> {
    fn new<E: TaskEncoder>(
        ty: RustTyDecomposition<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<Option<Self>, EncodeFullError<'vir, E>> {
        let count = deps
            .require_dep::<LifetimePositionEnc>(ty)?
            .positions()
            .count();
        if count <= 1 {
            return Ok(None);
        }
        let block = (
            ty,
            LifetimePosition::new(1),
            LifetimePosition::new(count),
            MergeTarget::Own,
        );
        let merge_split = deps.require_ref::<MergeSplitEnc>(block)?;
        let (_, params) = ty
            .canonical_with_params()
            .expect("the merge/split encoder rejects const parameters");
        let params = vir::with_vcx(|vcx| {
            vcx.tcx()
                .mk_args_from_iter(params.into_iter().map(ty::GenericArg::from))
        });
        let args = deps.require_dep::<GArgsTyEnc>(GArgs::new(ty.args.context(), params))?;
        Ok(Some(Self {
            ty,
            merge_split,
            args,
        }))
    }

    /// The statement that merges (or splits) the block of a reference with
    /// snapshot `reference`.
    fn stmt<E: TaskEncoder>(
        &self,
        merge: bool,
        reference: vir::ExprCSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<vir::Stmt<'vir>, EncodeFullError<'vir, E>> {
        let key = NodePredicates::new(self.ty, deps)?
            .key(LifetimePosition::new(1), reference.upcast_ty());
        let method = if merge {
            self.merge_split.merge
        } else {
            self.merge_split.split
        };
        Ok(method(key, self.args.get_ty(), self.args.get_const()).alloc())
    }
}

impl<'vir> MirBuiltinUseCastOutput<'vir> {
    pub fn cast<Curr, Next>(
        &self,
        input: vir::ExprGenCSnap<'vir, Curr, Next>,
    ) -> vir::ExprGenCSnap<'vir, Curr, Next> {
        self.builtin.cast.call()(input, self.ty_args.get_ty(), self.ty_args.get_const())
    }

    /// The statements of a `&mut` unsizing coercion of the reference with
    /// snapshot `operand`, which move its predicates to the result's.
    pub fn unsize<E: TaskEncoder>(
        &self,
        operand: vir::ExprCSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<Vec<vir::Stmt<'vir>>, EncodeFullError<'vir, E>> {
        let Some(unsize) = self.builtin.unsize else {
            return Ok(Vec::new());
        };
        let [op, res] = self.referent_blocks;
        let mut stmts = Vec::new();
        if let Some(op) = op {
            stmts.push(op.stmt(true, operand, deps)?);
        }
        let call = unsize.call()(operand, self.ty_args.get_ty(), self.ty_args.get_const());
        stmts.push(vir::StmtKindGenData::alloc(call));
        if let Some(res) = res {
            stmts.push(res.stmt(false, self.cast(operand), deps)?);
        }
        Ok(stmts)
    }

    /// The statements that undo a `&mut` unsizing coercion of the reference
    /// with snapshot `operand`, which move the result's predicates back.
    pub fn undo<E: TaskEncoder>(
        &self,
        operand: vir::ExprCSnap<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<Vec<vir::Stmt<'vir>>, EncodeFullError<'vir, E>> {
        let Some(undo) = self.builtin.undo else {
            return Ok(Vec::new());
        };
        let [op, res] = self.referent_blocks;
        let mut stmts = Vec::new();
        if let Some(res) = res {
            stmts.push(res.stmt(true, self.cast(operand), deps)?);
        }
        let call = undo.call()(operand, self.ty_args.get_ty(), self.ty_args.get_const());
        stmts.push(vir::StmtKindGenData::alloc(call));
        if let Some(op) = op {
            stmts.push(op.stmt(false, operand, deps)?);
        }
        Ok(stmts)
    }
}

impl TaskEncoder for MirBuiltinUseCastEnc {
    task_encoder::encoder_cache!(MirBuiltinUseCastEnc);
    const ENCODER_NAME: &'static str = "MIR builtin use cast encoder";

    type TaskDescription<'vir> = MirBuiltinUseCastTask<'vir>;

    type OutputFullDependency<'vir> = MirBuiltinUseCastOutput<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        let MirBuiltinUseCastTask {
            result_ty,
            kind,
            operand_ty,
        } = *task_key;
        let task = MirBuiltinCastTask {
            result_ty: result_ty.ty,
            kind,
            operand_ty: operand_ty.ty,
        };
        let cast = deps.require_dep::<MirBuiltinCastEnc>(task)?;
        let ty_args = match cast.ty_args {
            CastTyArgs::None => &[],
            CastTyArgs::Referents => {
                // The cast function and the unsize/undo methods are all generic over
                // the operand and result referent type values `[U, V]`. The referent
                // is index 0 of each reference's type args (the metadata type is
                // derived from the referent, and the lifetime is not a type arg).
                let op_generics = deps.require_dep::<GArgsTyEnc>(operand_ty.args)?;
                let result_generics = deps.require_dep::<GArgsTyEnc>(result_ty.args)?;
                // Recover the operand/result referent (`[T; N]` / `[T]`) structure by
                // normalizing the reference's (generic `p_Param`) referent type
                // against the reference's args. The axioms requested with it only
                // apply to unsizing coercions.
                let (op_ref, res_ref) = match (&operand_ty.ty.specifics, &result_ty.ty.specifics) {
                    (TySpecifics::ImmRef(od), TySpecifics::ImmRef(rd)) => {
                        (od.referent, rd.referent)
                    }
                    (TySpecifics::MutRef(od), TySpecifics::MutRef(rd)) => {
                        (od.referent, rd.referent)
                    }
                    (TySpecifics::Raw(od), TySpecifics::Raw(rd)) => (od.referent, rd.referent),
                    // A `Box` unsize coercion: the referent is the boxed value.
                    (TySpecifics::StructLike(_), TySpecifics::StructLike(_))
                        if operand_ty.ty.special == RustTySpecial::Box =>
                    {
                        (operand_ty.ty.box_value_ty(), result_ty.ty.box_value_ty())
                    }
                    // `CastTyArgs::Referents` should only happen for
                    // reference/pointer types, do not expect other types here.
                    _ => unreachable!(
                        "unexpected operand/result reference types for pointer cast: {operand_ty:?} / {result_ty:?}"
                    ),
                };
                let op_inner = op_ref.decompose_normalize(operand_ty.args);
                let res_inner = res_ref.decompose_normalize(result_ty.args);
                deps.require_dep::<MetadataCastAxiomEnc>((op_inner.ty, res_inner.ty))?;
                deps.require_dep::<ValueCastAxiomEnc>((op_inner.ty, res_inner.ty))?;
                vir::with_vcx(|vcx| {
                    vcx.alloc_slice(&[op_generics.get_ty()[0], result_generics.get_ty()[0]])
                })
            }
        };
        let referent_blocks = match cast.unsize {
            Some(_) => [
                ReferentBlock::new(operand_ty, deps)?,
                ReferentBlock::new(result_ty, deps)?,
            ],
            None => [None, None],
        };
        Ok((
            (),
            MirBuiltinUseCastOutput {
                builtin: cast,
                ty_args: GArgsTy::new(ty_args, &[]),
                referent_blocks,
            },
        ))
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        MirBuiltinCastEnc::emit_outputs(program);
    }
}
