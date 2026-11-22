use prusti_rustc_interface::{
    middle::{mir, ty},
    span::symbol,
};
use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, ConstData, FunctionIdn, MethodIdn};

use crate::encoders::{
    TyUseImpureEnc,
    builtin::{MetadataCastEnc, ValueCastEnc},
    ty::{
        LazyRustTy, RustTy, RustTyDecomposition, RustTySpecial, TySpecifics,
        generics::{GParams, GenericParamsEnc},
        indirect_nodes::NodePredicates,
        interpretation::{
            bitvec::BitVecSize,
            float_conv::{FloatBitVecConvEnc, float_pow2_bits},
        },
        use_pure::TyUsePureEnc,
    },
};

/// Encodes the builtin MIR cast operations (e.g. `IntToInt`, `PointerCoercion`)
/// as Viper functions with the correct semantics. The unsizing coercions and the
/// casts between raw pointers are applied to generic refs (the inner type is a
/// parameter), so their meaning is given by axioms: the general ones of
/// `MetadataCastEnc` and, for unsizing, those for concrete types in
/// `MetadataCastAxiomEnc` (e.g. `&[T; N] -> &[T]`).
/// Also returns a side-effecting `unsize`/`undo` method for coercing the inner
/// referent (i.e. an `p_Param` to `p_Param` with different type).
///
/// Use `MirBuiltinUseCastEnc` to encode a cast in a specific context (with concrete
/// type arguments).
pub(super) struct MirBuiltinCastEnc;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(super) struct MirBuiltinCastTask<'vir> {
    pub(super) result_ty: RustTy<'vir>,
    pub(super) kind: mir::CastKind,
    pub(super) operand_ty: RustTy<'vir>,
}

/// The types a cast depends on beyond the operand and result types that key
/// its task. Its function and methods take them as type arguments,
/// which `MirBuiltinUseCastEnc` supplies at each use.
#[derive(Debug, Clone, Copy)]
pub(super) enum CastTyArgs {
    /// No type arguments: the cast depends only on the operand and result
    /// types (e.g. an integer cast).
    None,
    /// The operand's and the result's referent types `[U, V]`, which give the
    /// metadata after a pointer cast (via `metadata_cast`) and the value of an
    /// unsizing coercion (e.g. `&[T; N] -> &[T]`).
    Referents,
}

impl CastTyArgs {
    /// The type parameters that the cast's function and methods declare.
    fn params<'vir>(self, vcx: &'vir vir::VirCtxt<'vir>) -> GParams<'vir> {
        match self {
            CastTyArgs::None => GParams::empty(),
            CastTyArgs::Referents => {
                let [u, v] = Self::referents(vcx);
                GParams::empty_env(vcx.tcx().mk_args(&[u.into(), v.into()]))
            }
        }
    }

    /// The referent type parameters `[U, V]` of `CastTyArgs::Referents`.
    fn referents<'vir>(vcx: &'vir vir::VirCtxt<'vir>) -> [ty::Ty<'vir>; 2] {
        let param = |idx, name| ty::Ty::new_param(vcx.tcx(), idx, symbol::Symbol::intern(name));
        [param(0, "U"), param(1, "V")]
    }
}

/// The result of encoding a builtin cast.
#[derive(Debug, Clone, Copy)]
pub(super) struct MirBuiltinCastOutput<'vir> {
    /// The function from the operand snapshot to the result snapshot.
    pub(super) cast: FunctionIdn<'vir, (vir::CSnap, vir::ManyTyVal, vir::ManyCSnap), vir::CSnap>,
    /// For a `&mut` unsizing coercion, the side-effecting methods that
    /// transfer permission between the original and the coerced reference.
    pub(super) unsize: Option<vir::MethodIdn<'vir, (vir::CSnap, vir::ManyTyVal, vir::ManyCSnap)>>,
    pub(super) undo: Option<vir::MethodIdn<'vir, (vir::CSnap, vir::ManyTyVal, vir::ManyCSnap)>>,
    /// The type arguments that `cast`, `unsize` and `undo` take.
    pub(super) ty_args: CastTyArgs,
}

/// The Viper definitions produced for a builtin cast: always the cast function,
/// plus the side-effecting `unsize`/`undo` methods for `&mut` unsize coercions.
#[derive(Debug, Clone, Copy)]
pub(super) struct MirBuiltinCastLocal<'vir> {
    cast: vir::Function<'vir>,
    unsize: Option<vir::Method<'vir>>,
    undo: Option<vir::Method<'vir>>,
    /// Whether this encodes something the user writes rather than something
    /// rustc or the encoding adds, which the simplifier may fold away (see
    /// `add_user_adt`).
    user_level: bool,
}

impl TaskEncoder for MirBuiltinCastEnc {
    task_encoder::encoder_cache!(MirBuiltinCastEnc);
    const ENCODER_NAME: &'static str = "MIR builtin cast encoder";

    type TaskDescription<'vir> = MirBuiltinCastTask<'vir>;

    type OutputFullDependency<'vir> = MirBuiltinCastOutput<'vir>;
    type OutputFullLocal<'vir> = MirBuiltinCastLocal<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        let MirBuiltinCastTask {
            result_ty,
            kind,
            operand_ty,
        } = *task_key;
        let op_ty = RustTyDecomposition::identity(operand_ty);
        let op_ty = deps.require_dep::<TyUsePureEnc>(op_ty)?;
        let op_ty_snap = op_ty.snapshot.downcast_ty::<vir::CSnap>();
        let res_ty = RustTyDecomposition::identity(result_ty);
        let res_ty = deps.require_dep::<TyUsePureEnc>(res_ty)?;
        let res_ty_snap = res_ty.snapshot.downcast_ty::<vir::CSnap>();

        let (name, ty_args, user_level) = match kind {
            mir::CastKind::PointerCoercion(ty::adjustment::PointerCoercion::Unsize, ..) => {
                ("unsize", CastTyArgs::Referents, false)
            }
            mir::CastKind::IntToInt => ("i2i", CastTyArgs::None, true),
            mir::CastKind::PtrToPtr => ("p2p", CastTyArgs::Referents, true),
            mir::CastKind::IntToFloat => ("i2f", CastTyArgs::None, true),
            mir::CastKind::FloatToInt => ("f2i", CastTyArgs::None, true),
            mir::CastKind::PointerCoercion(
                ty::adjustment::PointerCoercion::MutToConstPointer,
                ..,
            ) => ("mut2const", CastTyArgs::None, false),
            mir::CastKind::PointerCoercion(coercion, _) => {
                return Err(EncodeFullError::unsupported(
                    format!("the pointer coercion `{coercion:?}`"),
                    Vec::new(),
                ));
            }
            other => {
                return Err(EncodeFullError::unsupported(
                    format!("the cast `{other:?}`"),
                    Vec::new(),
                ));
            }
        };
        // The referent types are not part of the task key: a cast that needs
        // them takes them as type arguments (`CastTyArgs`), so the name only
        // uses the pointer types and one task serves every referent.
        let name = vir::with_vcx(|vcx| {
            vir::vir_format_identifier!(
                vcx,
                "mir_cast_{name}_{}_to_{}",
                operand_ty.name(),
                result_ty.name(),
            )
        });
        let gparams = vir::with_vcx(|vcx| ty_args.params(vcx));
        let params = deps.require_dep::<GenericParamsEnc>(gparams)?;
        vir::with_vcx(|vcx| {
            let cast_idn = FunctionIdn::new(
                name,
                (op_ty_snap, params.ty_args(), params.const_args()),
                res_ty_snap,
            );
            let arg_decl = vcx.mk_local_decl("arg", op_ty_snap);
            let arg_ex = vcx.mk_local_ex(arg_decl);
            // The body of the cast function, and the `unsize`/`undo` methods
            // with their identifiers if the cast has them.
            let (expr, methods) = match kind {
                mir::CastKind::IntToInt => {
                    let e_op_ty = op_ty.expect_primitive();
                    let e_res_ty = res_ty.expect_primitive();
                    let result_prim = *result_ty.expect_primitive();
                    let operand_prim = *operand_ty.expect_primitive();

                    // An integer `as` cast never panics: when every value of the
                    // source type is representable in the target the value is
                    // preserved unchanged; otherwise it is truncated to the target
                    // width and reinterpreted with the target's signedness. Either
                    // way any input is valid, so the cast function needs NO
                    // precondition.
                    let wrapped = if matches!(operand_prim.kind(), ty::TyKind::Bool) {
                        // `bool as <int>` is `b ? 1 : 0`; always lossless.
                        vcx.mk_ternary_expr(
                            arg_ex.downcast_ty(),
                            vcx.mk_int::<1>(),
                            vcx.mk_int::<0>(),
                        )
                        .upcast_ty()
                    } else {
                        let (to_bits, to_signed) = vcx.get_int_data(result_prim);
                        let (from_bits, from_signed) = vcx.get_int_data(operand_prim);

                        let arg_prim = e_op_ty.snap_to_prim(arg_ex);

                        let lossless = match (from_signed, to_signed) {
                            (false, false) | (true, true) => from_bits <= to_bits,
                            (false, true) => from_bits < to_bits, // one bit reserved for the sign
                            (true, false) => false,               // a negative source never fits
                        };
                        if lossless {
                            arg_prim
                        } else {
                            // Truncate to the target width and reinterpret with its
                            // signedness (two's complement); the shared helper wraps
                            // `((x [+ 2^(N-1)]) mod 2^N) [- 2^(N-1)]`.
                            vcx.get_wrapped_val(arg_prim.downcast_ty(), result_prim)
                                .upcast_ty()
                        }
                    };
                    let expr = e_res_ty.prim_to_snap(wrapped);

                    (expr, None)
                }
                mir::CastKind::PtrToPtr => {
                    let e_op_ty = op_ty.expect_raw();
                    let e_res_ty = res_ty.expect_raw();
                    let metadata_cast = deps.require_dep::<MetadataCastEnc>(())?;
                    let metadata = metadata_cast(
                        e_op_ty.metadata_access(arg_ex).downcast_ty(),
                        params.ty_exprs()[0],
                        params.ty_exprs()[1],
                    );
                    let expr =
                        e_res_ty.prim_to_snap(e_op_ty.address_access(arg_ex), metadata.upcast_ty());
                    (expr, None)
                }
                // `*mut T` to `*const T` keeps the address and metadata.
                mir::CastKind::PointerCoercion(
                    ty::adjustment::PointerCoercion::MutToConstPointer,
                    _,
                ) => {
                    let e_op_ty = op_ty.expect_raw();
                    let e_res_ty = res_ty.expect_raw();
                    let expr = e_res_ty.prim_to_snap(
                        e_op_ty.address_access(arg_ex),
                        e_op_ty.metadata_access(arg_ex),
                    );

                    (expr, None)
                }
                mir::CastKind::IntToFloat => {
                    let e_op_ty = op_ty.expect_primitive();
                    let (bits, signed) = vcx.get_int_data(*operand_ty.expect_primitive());
                    // Convert at the width of the source type, which keeps `int2bv` cheaper for Z3.
                    let ty::Float(float) = *result_ty.expect_primitive().kind() else {
                        unreachable!()
                    };
                    let conv = deps
                        .require_dep::<FloatBitVecConvEnc>((float, BitVecSize::from_bits(bits)))?;
                    let arg_prim = e_op_ty.snap_to_prim(arg_ex);
                    let expr = if signed {
                        (conv.from_sint)(arg_prim)
                    } else {
                        (conv.from_uint)(arg_prim)
                    };

                    (expr, None)
                }
                mir::CastKind::FloatToInt => {
                    let e_op_ty = op_ty.expect_float();
                    let e_res_ty = res_ty.expect_primitive();
                    let result_prim = *result_ty.expect_primitive();
                    let ty::Float(float) = *operand_ty.expect_primitive().kind() else {
                        unreachable!()
                    };
                    let (bits, signed) = vcx.get_int_data(result_prim);
                    // Convert at the width of the target type, so that every
                    // in-range value is representable.
                    let conv = deps
                        .require_dep::<FloatBitVecConvEnc>((float, BitVecSize::from_bits(bits)))?;
                    let float_const = |bits: u128| {
                        (e_op_ty.prim_to_snap)(vcx.mk_const_expr(ConstData::Int(bits)))
                    };

                    // `as` rounds towards zero, which the `RTZ` conversions
                    // already do. They are unspecified for values whose rounded
                    // value is out of range, so saturate first. The bounds
                    // `-2^(N-1)`/`0` and `2^(N-1)`/`2^N` are integers, so
                    // comparing the untruncated value against them gives the
                    // same result as comparing the truncated one. They are also
                    // powers of two and therefore exact (or infinite if out of
                    // the float's range, in which case only the infinities reach
                    // them). The bounds are compared as floats: going via
                    // `fp.to_real` makes symbolic casts unprovable with Z3's
                    // arithmetic solver.
                    let (lower, upper, in_range) = if signed {
                        (
                            float_pow2_bits(float, bits - 1, true),
                            float_pow2_bits(float, bits - 1, false),
                            (conv.bitvec.sbv_to_int)((conv.to_sbv)(arg_ex)),
                        )
                    } else {
                        (
                            0,
                            float_pow2_bits(float, bits, false),
                            (conv.bitvec.ubv_to_int)((conv.to_ubv)(arg_ex)),
                        )
                    };
                    let expr = vcx.mk_ternary_expr(
                        (e_op_ty.fp_is_nan)(arg_ex),
                        e_res_ty.prim_to_snap(vcx.mk_int::<0>().upcast_ty()),
                        vcx.mk_ternary_expr(
                            (e_op_ty.fp_leq)(arg_ex, float_const(lower)),
                            e_res_ty.prim_to_snap(vcx.get_min_int(result_prim).upcast_ty()),
                            vcx.mk_ternary_expr(
                                (e_op_ty.fp_geq)(arg_ex, float_const(upper)),
                                e_res_ty.prim_to_snap(vcx.get_max_int(result_prim).upcast_ty()),
                                e_res_ty.prim_to_snap(in_range.upcast_ty()),
                            ),
                        ),
                    );

                    (expr, None)
                }
                mir::CastKind::PointerCoercion(ty::adjustment::PointerCoercion::Unsize, _) => {
                    // Unsizing preserves the (wide) reference snapshot type and only
                    // rewrites the pointer metadata for the new referent. The cast
                    // (like the methods below) is parameterized by the operand and
                    // result referent types `U`, `V` (`CastTyArgs::Referents`),
                    // which `MirBuiltinUseCastEnc` supplies from each reference's
                    // referent arg.
                    assert_eq!(op_ty_snap, res_ty_snap);
                    let u = params.ty_exprs()[0];
                    let v = params.ty_exprs()[1];
                    let value_cast = deps.require_dep::<ValueCastEnc>(())?;
                    let metadata_cast = deps.require_dep::<MetadataCastEnc>(())?;
                    let value_cast = |value| value_cast(value, u, v);
                    let metadata_cast = |metadata| metadata_cast(metadata, u, v);
                    let (is_mut, expr) = match &op_ty.specifics {
                        TySpecifics::ImmRef(data) => {
                            let res_data = res_ty.expect_immref();
                            let expr = res_data.prim_to_snap(
                                data.addr_access(arg_ex),
                                metadata_cast(data.metadata_access(arg_ex).downcast_ty())
                                    .upcast_ty(),
                                value_cast(data.value_access(arg_ex).downcast_ty()).upcast_ty(),
                            );
                            (false, expr)
                        }
                        TySpecifics::MutRef(data) => {
                            let res_data = res_ty.expect_mutref();
                            let expr = res_data.prim_to_snap(
                                data.deref_access(arg_ex),
                                metadata_cast(data.metadata_access(arg_ex).downcast_ty())
                                    .upcast_ty(),
                                value_cast(data.value_access(arg_ex).downcast_ty()).upcast_ty(),
                            );
                            (true, expr)
                        }
                        // A `Box` (e.g. `Box<[T; N]> -> Box<[T]>`): rebuild the
                        // snapshot with the coerced pointer metadata and value
                        // (the address is unchanged). Unlike `&mut`, no
                        // side-effecting methods are needed: the box owns its
                        // referent, so the whole predicate is exhaled/inhaled
                        // by the surrounding assignment.
                        TySpecifics::StructLike(data)
                            if operand_ty.special == RustTySpecial::Box =>
                        {
                            assert_eq!(operand_ty, result_ty);
                            let expr = data.box_unsize_snap(
                                arg_ex,
                                metadata_cast(data.box_metadata_access(arg_ex).downcast_ty())
                                    .upcast_ty(),
                                value_cast(data.box_value_access(arg_ex).downcast_ty()).upcast_ty(),
                            );
                            (false, expr)
                        }
                        _ => {
                            return Err(EncodeFullError::unsupported(
                                format!(
                                    "unsizing of `{}` (only references and `Box` can be unsized)",
                                    operand_ty.name()
                                ),
                                Vec::new(),
                            ));
                        }
                    };
                    // For `&mut` coercions, generate the side-effecting
                    // `unsize`/`undo` methods that move the indirect predicates
                    // of the generic `&mut U` to those of `&mut V` on the cast
                    // reference, and back. The predicate of the reference's own
                    // position holds the referent's `p_Param` predicate, which the
                    // coercion keeps at the same address; the others hold those of
                    // the lifetimes in the referent, which a use merges into one
                    // predicate before and splits after (see
                    // `MirBuiltinUseCastOutput`). The methods are declared over the
                    // same `[U, V]` type params (`params`), which
                    // `MirBuiltinUseCastEnc` instantiates with the operand referent
                    // `U` and result referent `V`. The referent snapshot is
                    // transferred via `value_cast` (whose concrete-type axioms are
                    // added in `MirBuiltinUseCastEnc`).
                    let methods = if is_mut {
                        let u_impure = deps.require_dep::<TyUseImpureEnc>(
                            LazyRustTy::new_param_ty(0).decompose(gparams),
                        )?;
                        let v_impure = deps.require_dep::<TyUseImpureEnc>(
                            LazyRustTy::new_param_ty(1).decompose(gparams),
                        )?;
                        let tcx = vcx.tcx();
                        let mut_ref = |referent| {
                            let ty = ty::Ty::new_mut_ref(tcx, tcx.lifetimes.re_erased, referent);
                            RustTyDecomposition::from_ty(ty, gparams)
                        };
                        let [u, v] = CastTyArgs::referents(vcx);
                        let u_nodes = NodePredicates::new(mut_ref(u), deps)?;
                        let v_nodes = NodePredicates::new(mut_ref(v), deps)?;

                        let src_mutref = op_ty.expect_mutref();
                        let src_decl = vcx.mk_local_decl("src", op_ty_snap);
                        let src_ex = vcx.mk_local_ex(src_decl);
                        let addr = src_mutref.deref_access(src_ex);
                        let cast_ex =
                            cast_idn.call()(src_ex, params.ty_exprs(), params.const_exprs());

                        let u_preds = u_nodes.all(src_ex.upcast_ty());
                        let v_preds = v_nodes.all(cast_ex.upcast_ty());
                        // The referent's snapshot, read in the predicate of the
                        // reference's own position, the outermost one.
                        let referent_snap = |nodes: &NodePredicates<'vir>, reference, snap| {
                            let own = nodes.positions()[0];
                            vcx.mk_unfolding_expr(nodes.predicate(own, reference), snap)
                        };
                        let u_snap = referent_snap(
                            &u_nodes,
                            src_ex.upcast_ty(),
                            u_impure.ref_to_snap(addr).downcast_ty::<vir::PSnap>(),
                        );
                        let v_snap = referent_snap(
                            &v_nodes,
                            cast_ex.upcast_ty(),
                            v_impure.ref_to_snap(addr).downcast_ty::<vir::PSnap>(),
                        );

                        // `unsize`: operand referent `U` -> result referent `V`; the
                        // new `V` value is `value_cast(old(U), U, V)`. `undo` is the
                        // *reverse* equation: the old `V` value is `value_cast` of the
                        // recovered `U` value, `old(V) == value_cast(U, U, V)`. Both
                        // use the same `value_cast` direction (`U, V`), so a single
                        // axiom per coercion (added in `MirBuiltinUseCastEnc`) suffices.
                        let unsize_value =
                            vcx.mk_eq_expr(v_snap, value_cast(vcx.mk_old_expr(u_snap)));
                        let undo_value =
                            vcx.mk_eq_expr(vcx.mk_old_expr(v_snap), value_cast(u_snap));

                        let unsize_idn = MethodIdn::new(
                            vir::vir_format_identifier!(
                                vcx,
                                "mir_unsize_{}_to_{}",
                                operand_ty.name(),
                                result_ty.name()
                            ),
                            (op_ty_snap, params.ty_args(), params.const_args()),
                        );
                        let undo_idn = MethodIdn::new(
                            vir::vir_format_identifier!(
                                vcx,
                                "mir_undo_unsize_{}_to_{}",
                                operand_ty.name(),
                                result_ty.name()
                            ),
                            (op_ty_snap, params.ty_args(), params.const_args()),
                        );
                        let unsize_method = vcx.mk_method(
                            unsize_idn,
                            (src_decl, params.ty_decls(), params.const_decls()),
                            &[],
                            vcx.alloc_slice(&u_preds),
                            vcx.alloc_slice(&[v_preds.as_slice(), &[unsize_value]].concat()),
                            None,
                        );
                        let undo_method = vcx.mk_method(
                            undo_idn,
                            (src_decl, params.ty_decls(), params.const_decls()),
                            &[],
                            vcx.alloc_slice(&v_preds),
                            vcx.alloc_slice(&[u_preds.as_slice(), &[undo_value]].concat()),
                            None,
                        );
                        Some(((unsize_method, unsize_idn), (undo_method, undo_idn)))
                    } else {
                        None
                    };
                    (expr, methods)
                }
                _ => unreachable!("unsupported cast kinds are rejected above"),
            };

            let cast = vcx.mk_function(
                cast_idn,
                (arg_decl, params.ty_decls(), params.const_decls()),
                &[],
                &[],
                None,
                Some(expr),
            );
            let (unsize, undo) = methods.unzip();
            let (unsize, unsize_idn) = unsize.unzip();
            let (undo, undo_idn) = undo.unzip();
            Ok((
                MirBuiltinCastLocal {
                    cast,
                    unsize,
                    undo,
                    user_level,
                },
                MirBuiltinCastOutput {
                    cast: cast_idn,
                    unsize: unsize_idn,
                    undo: undo_idn,
                    ty_args,
                },
            ))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for local in Self::all_outputs_local(program) {
            if local.user_level {
                program.add_user_operator(local.cast);
            } else {
                program.add_function(local.cast);
            }
            if let Some(unsize) = local.unsize {
                program.add_method(unsize);
            }
            if let Some(undo) = local.undo {
                program.add_method(undo);
            }
        }
    }
}
