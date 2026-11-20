use prusti_rustc_interface::middle::ty;
use task_encoder::{EncodeFullError, TaskEncoder, TaskEncoderDependencies};
use vir::{BackendInterpretationPair, CallableIdn, CastType, FunctionIdn, VirCtxt};

use crate::encoders::ty::{
    RustTyDecomposition,
    interpretation::{
        bitvec::{BitVecEnc, BitVecSize},
        float_conv::float_format,
    },
    pure::{DomainBuilder, TyPureEnc},
    use_pure::TyUsePureEnc,
};

pub type FloatDomain<'vir> = &'vir FloatDomainData<'vir>;

#[derive(Debug, Clone, Copy)]
pub struct FloatDomainData<'vir> {
    /// Viper primitive value (the raw bits) as argument. Returns domain.
    pub prim_to_snap: FunctionIdn<'vir, vir::Prim, vir::CSnap>,
    #[allow(unused)]
    pub from_bv: FunctionIdn<'vir, vir::CSnap, vir::CSnap>,
    pub fp_eq: FunctionIdn<'vir, (vir::CSnap, vir::CSnap), vir::Bool>,
    pub fp_add: FunctionIdn<'vir, (vir::CSnap, vir::CSnap), vir::CSnap>,
    pub fp_sub: FunctionIdn<'vir, (vir::CSnap, vir::CSnap), vir::CSnap>,
    pub fp_mul: FunctionIdn<'vir, (vir::CSnap, vir::CSnap), vir::CSnap>,
    pub fp_div: FunctionIdn<'vir, (vir::CSnap, vir::CSnap), vir::CSnap>,
    pub fp_trunc: FunctionIdn<'vir, vir::CSnap, vir::CSnap>,
    pub fp_is_nan: FunctionIdn<'vir, vir::CSnap, vir::Bool>,
    pub fp_is_infinite: FunctionIdn<'vir, vir::CSnap, vir::Bool>,
    pub fp_lt: FunctionIdn<'vir, (vir::CSnap, vir::CSnap), vir::Bool>,
    pub fp_leq: FunctionIdn<'vir, (vir::CSnap, vir::CSnap), vir::Bool>,
    pub fp_gt: FunctionIdn<'vir, (vir::CSnap, vir::CSnap), vir::Bool>,
    pub fp_geq: FunctionIdn<'vir, (vir::CSnap, vir::CSnap), vir::Bool>,
    pub fp_neg: FunctionIdn<'vir, vir::CSnap, vir::CSnap>,
    pub fp_abs: FunctionIdn<'vir, vir::CSnap, vir::CSnap>,
    pub fp_to_real: FunctionIdn<'vir, vir::CSnap, vir::Perm>,
}

pub(crate) fn ty_pure_float<'vir>(
    vcx: &'vir VirCtxt<'vir>,
    deps: &mut TaskEncoderDependencies<'vir, TyPureEnc>,
    builder: &mut DomainBuilder<'vir>,
    float: ty::FloatTy,
    prim_to_snap: FunctionIdn<'vir, vir::Prim, vir::CSnap>,
) -> Result<FloatDomainData<'vir>, EncodeFullError<'vir, TyPureEnc>> {
    let backend_type = match float {
        ty::FloatTy::F16 => vcx.alloc_slice(&[
            vcx.alloc(BackendInterpretationPair {
                key: "SMTLIB",
                value: "(_ FloatingPoint 5 11)",
            }),
            vcx.alloc(BackendInterpretationPair {
                key: ("Boogie"),
                value: ("float11e5"),
            }),
        ]),
        ty::FloatTy::F32 => vcx.alloc_slice(&[
            vcx.alloc(BackendInterpretationPair {
                key: "SMTLIB",
                value: "(_ FloatingPoint 8 24)",
            }),
            vcx.alloc(BackendInterpretationPair {
                key: ("Boogie"),
                value: ("float24e8"),
            }),
        ]),
        ty::FloatTy::F64 => vcx.alloc_slice(&[
            vcx.alloc(BackendInterpretationPair {
                key: "SMTLIB",
                value: "(_ FloatingPoint 11 53)",
            }),
            vcx.alloc(BackendInterpretationPair {
                key: ("Boogie"),
                value: ("float53e11"),
            }),
        ]),
        ty::FloatTy::F128 => vcx.alloc_slice(&[
            vcx.alloc(BackendInterpretationPair {
                key: "SMTLIB",
                value: "(_ FloatingPoint 15 113)",
            }),
            vcx.alloc(BackendInterpretationPair {
                key: ("Boogie"),
                value: ("float113e15"),
            }),
        ]),
    };
    builder.set_interpretation(backend_type);

    let self_type = builder.self_type();
    let fp_eq = wrapped_binary(builder, "eq", vir::TYPE_BOOL, "fp.eq");
    let fp_add = wrapped_binary(builder, "add", self_type, "fp.add RNE");
    let fp_sub = wrapped_binary(builder, "sub", self_type, "fp.sub RNE");

    let fp_mul = builder.backend_func(
        "mul",
        (builder.self_type(), builder.self_type()),
        builder.self_type(),
        Some("fp.mul RNE"),
    );

    let fp_div = builder.backend_func(
        "div",
        (builder.self_type(), builder.self_type()),
        builder.self_type(),
        Some("fp.div RNE"),
    );

    let fp_trunc = builder.backend_func(
        "trunc",
        builder.self_type(),
        builder.self_type(),
        Some("fp.roundToIntegral RTZ"),
    );

    let fp_is_nan = builder.backend_func(
        "is_nan",
        builder.self_type(),
        vir::TYPE_BOOL,
        Some("fp.isNaN"),
    );

    let fp_is_infinite = builder.backend_func(
        "is_infinite",
        builder.self_type(),
        vir::TYPE_BOOL,
        Some("fp.isInfinite"),
    );

    let fp_lt = wrapped_binary(builder, "lt", vir::TYPE_BOOL, "fp.lt");
    let fp_leq = wrapped_binary(builder, "leq", vir::TYPE_BOOL, "fp.leq");
    let fp_geq = wrapped_binary(builder, "geq", vir::TYPE_BOOL, "fp.geq");
    let fp_gt = wrapped_binary(builder, "gt", vir::TYPE_BOOL, "fp.gt");
    let fp_neg = wrapped_unary(builder, "neg", "fp.neg");
    let fp_abs = wrapped_unary(builder, "abs", "fp.abs");

    let bit_vec = deps.require_dep::<BitVecEnc>(match float {
        ty::FloatTy::F16 => BitVecSize::BitVec16,
        ty::FloatTy::F32 => BitVecSize::BitVec32,
        ty::FloatTy::F64 => BitVecSize::BitVec64,
        ty::FloatTy::F128 => BitVecSize::BitVec128,
    })?;

    let from_bv_name = match float {
        ty::FloatTy::F16 => "(_ to_fp 5 11)",
        ty::FloatTy::F32 => "(_ to_fp 8 24)",
        ty::FloatTy::F64 => "(_ to_fp 11 53)",
        ty::FloatTy::F128 => "(_ to_fp 15 113)",
    };
    let from_bv = builder.backend_func(
        "from_bv",
        (bit_vec.domain)(),
        builder.self_type(),
        Some(from_bv_name),
    );

    // Not `fp.to_real`: Z3 cannot reason about it on symbolic floats, and
    // versions before 5.0 can answer `unsat` for satisfiable queries containing
    // it (Z3Prover/z3#9022). Its meaning comes from `real_value_axioms` and
    // the literal values of `FloatLitEnc`. Once the Z3 that Viper ships is 5.0
    // or later, using `fp.to_real` again can be reconsidered.
    let fp_to_real = builder.function("to_real", builder.self_type(), vir::TYPE_PERM);

    builder.axiom("prim_to_snap", vir::expr! {
        forall i: [prim_to_snap.arity()] :: {[prim_to_snap](i)} ([prim_to_snap](i)) == ([from_bv]([bit_vec.from_int](i)))
    });

    let data = FloatDomainData {
        prim_to_snap,
        from_bv,
        fp_eq,
        fp_add,
        fp_sub,
        fp_mul,
        fp_div,
        fp_trunc,
        fp_is_nan,
        fp_is_infinite,
        fp_lt,
        fp_leq,
        fp_gt,
        fp_geq,
        fp_neg,
        fp_abs,
        fp_to_real,
    };
    real_value_axioms(vcx, builder, &data, float);
    Ok(data)
}

/// An uninterpreted wrapper around a binary backend operation, so that axioms
/// can trigger on it (Viper rejects interpreted functions in triggers).
fn wrapped_binary<'vir, T: vir::CompType>(
    builder: &mut DomainBuilder<'vir>,
    name: &str,
    ret: vir::Type<'vir, T>,
    interpretation: &'static str,
) -> FunctionIdn<'vir, (vir::CSnap, vir::CSnap), T> {
    let ty = builder.self_type();
    let backend: FunctionIdn<'vir, (vir::CSnap, vir::CSnap), T> =
        builder.backend_func(&format!("{name}_ieee"), (ty, ty), ret, Some(interpretation));
    let wrapper = builder.function(name, (ty, ty), ret);
    builder.axiom(
        &format!("{name}_def"),
        vir::expr! {
            forall x: [ty], y: [ty] :: {[wrapper](x, y)} ([wrapper](x, y)) == ([backend](x, y))
        },
    );
    wrapper
}

/// Like [`wrapped_binary`], for a unary operation on the float itself.
fn wrapped_unary<'vir>(
    builder: &mut DomainBuilder<'vir>,
    name: &str,
    interpretation: &'static str,
) -> FunctionIdn<'vir, vir::CSnap, vir::CSnap> {
    let ty = builder.self_type();
    let backend: FunctionIdn<'vir, vir::CSnap, vir::CSnap> =
        builder.backend_func(&format!("{name}_ieee"), ty, ty, Some(interpretation));
    let wrapper = builder.function(name, ty, ty);
    builder.axiom(
        &format!("{name}_def"),
        vir::expr! {
            forall x: [ty] :: {[wrapper](x)} ([wrapper](x)) == ([backend](x))
        },
    );
    wrapper
}

/// The precision `p` (significand bits, including the implicit one) and the
/// maximum exponent `emax` of an IEEE 754 binary format.
pub(crate) fn ieee_params(float: ty::FloatTy) -> (u32, u32) {
    let (ebits, sbits) = float_format(float);
    (sbits, (1 << (ebits - 1)) - 1)
}

/// An integer constant.
pub(crate) fn int_const<'vir>(vcx: &'vir VirCtxt<'vir>, value: u128) -> vir::ExprInt<'vir> {
    vcx.mk_const_expr(vir::ConstData::Int(value)).downcast_ty()
}

/// `2^k`, as a single constant if it fits into one, and otherwise as a product
/// of `2^127`, the largest power of two that does.
fn pow2<'vir>(vcx: &'vir VirCtxt<'vir>, k: u32) -> vir::ExprInt<'vir> {
    if k < 128 {
        return int_const(vcx, 1 << k);
    }
    let mut res = int_const(vcx, 1 << (k % 127));
    for _ in 0..k / 127 {
        res = vir::expr! { (res) * ([int_const(vcx, 1 << 127)]) };
    }
    res
}

/// The rational number `num / den`.
pub(crate) fn rational<'vir>(
    vcx: &'vir VirCtxt<'vir>,
    num: vir::ExprInt<'vir>,
    den: vir::ExprInt<'vir>,
) -> vir::ExprPerm<'vir> {
    vcx.mk_bin_op_expr(vir::BinOpKind::IntIntPermDiv, num, den)
        .downcast_ty()
}

/// The absolute value of a real.
fn perm_abs<'vir>(vcx: &'vir VirCtxt<'vir>, e: vir::ExprPerm<'vir>) -> vir::ExprPerm<'vir> {
    let zero = rational(vcx, vcx.mk_int::<0>(), vcx.mk_int::<1>());
    vir::expr! { ((e) >= (zero)) ? (e) : (-(e)) }
}

/// The float is neither NaN nor an infinity.
fn finite<'vir>(data: &FloatDomainData<'vir>, e: vir::ExprCSnap<'vir>) -> vir::ExprBool<'vir> {
    vir::expr! { (!([data.fp_is_nan](e))) && (!([data.fp_is_infinite](e))) }
}

/// The overflow threshold of round-to-nearest, `2^(emax + 1) - 2^(emax - p)`:
/// halfway between the largest finite value and the next power of two. A value
/// rounds to an infinity exactly when its magnitude is at least this.
pub(crate) fn rne_overflow<'vir>(
    vcx: &'vir VirCtxt<'vir>,
    float: ty::FloatTy,
) -> vir::ExprInt<'vir> {
    let (p, emax) = ieee_params(float);
    if emax <= 127 {
        int_const(vcx, (u128::MAX >> (127 - emax)) - (1 << (emax - p)) + 1)
    } else {
        vir::expr! { ([pow2(vcx, emax + 1)]) - ([pow2(vcx, emax - p)]) }
    }
}

/// The assertion that the float `app` is finite and within a relative error of
/// `2^-p` of `exact` (which should be a variable or a short term, since it is
/// repeated). Round-to-nearest guarantees this when `exact` is below the
/// overflow threshold and is either representable or outside the subnormal
/// range. Finiteness is part of it because deriving it from the bit-level
/// definition stalls under Silicon's `smt.case_split 3` and
/// `smt.delay_units true`.
pub(crate) fn rne_rounded<'vir>(
    vcx: &'vir VirCtxt<'vir>,
    data: &FloatDomainData<'vir>,
    float: ty::FloatTy,
    app: vir::ExprCSnap<'vir>,
    exact: vir::ExprPerm<'vir>,
) -> vir::ExprBool<'vir> {
    let (p, _) = ieee_params(float);
    let eps = rational(vcx, vcx.mk_int::<1>(), pow2(vcx, p));
    let error_decl = vcx.mk_local_decl("error", vir::TYPE_PERM);
    let bound_decl = vcx.mk_local_decl("bound", vir::TYPE_PERM);
    let (error, bound) = (vcx.mk_local_ex(error_decl), vcx.mk_local_ex(bound_decl));
    let body =
        vir::expr! { ([finite(data, app)]) && (((-(bound)) <= (error)) && ((error) <= (bound))) };
    let body = vcx.mk_let_expr(
        bound_decl,
        vir::expr! { (eps) * ([perm_abs(vcx, exact)]) },
        body,
    );
    vcx.mk_let_expr(
        error_decl,
        vir::expr! { ([data.fp_to_real](app)) - (exact) },
        body,
    )
}

/// Axioms giving `to_real` the IEEE 754 semantics of round-to-nearest-even:
/// comparisons of finite floats are comparisons of their values, and a sum or
/// difference of finite floats is finite and within a relative error of
/// `2^-p` of the exact result, unless that exceeds the overflow threshold.
/// (Subnormal results of additions are exact, so no absolute error term is
/// needed.) Negation and absolute value of a finite float are exact. This
/// keeps the reasoning in linear real arithmetic. The values of products,
/// quotients and truncations are left unconstrained.
fn real_value_axioms<'vir>(
    vcx: &'vir VirCtxt<'vir>,
    builder: &mut DomainBuilder<'vir>,
    data: &FloatDomainData<'vir>,
    float: ty::FloatTy,
) {
    let ty = builder.self_type();
    let overflow = rational(vcx, rne_overflow(vcx, float), vcx.mk_int::<1>());
    let x = vcx.mk_local_decl("x", ty);
    let y = vcx.mk_local_decl("y", ty);
    let (xe, ye) = (vcx.mk_local_ex(x), vcx.mk_local_ex(y));
    let (rx, ry) = ((data.fp_to_real)(xe), (data.fp_to_real)(ye));

    for (name, op, value) in [
        ("eq", data.fp_eq, vir::expr! { (rx) == (ry) }),
        ("lt", data.fp_lt, vir::expr! { (rx) < (ry) }),
        ("leq", data.fp_leq, vir::expr! { (rx) <= (ry) }),
        ("geq", data.fp_geq, vir::expr! { (rx) >= (ry) }),
        ("gt", data.fp_gt, vir::expr! { (rx) > (ry) }),
    ] {
        let app = op(xe, ye);
        let body =
            vir::expr! { (([finite(data, xe)]) && ([finite(data, ye)])) ==> ((app) == (value)) };
        builder.axiom(
            &format!("{name}_real"),
            vir::expr! { forall [x], [y] :: {app} body },
        );
    }

    for (name, op, exact) in [
        ("add", data.fp_add, vir::expr! { (rx) + (ry) }),
        ("sub", data.fp_sub, vir::expr! { (rx) - (ry) }),
    ] {
        let app = op(xe, ye);
        let exact_decl = vcx.mk_local_decl("exact", vir::TYPE_PERM);
        let e = vcx.mk_local_ex(exact_decl);
        let in_range = vir::expr! { ([perm_abs(vcx, e)]) < (overflow) };
        let body = vir::expr! {
            (([finite(data, xe)]) && (([finite(data, ye)]) && (in_range)))
                ==> ([rne_rounded(vcx, data, float, app, e)])
        };
        let body = vcx.mk_let_expr(exact_decl, exact, body);
        builder.axiom(
            &format!("{name}_real"),
            vir::expr! { forall [x], [y] :: {app} body },
        );
    }

    for (name, op, value) in [
        ("neg", data.fp_neg, vir::expr! { -(rx) }),
        ("abs", data.fp_abs, perm_abs(vcx, rx)),
    ] {
        let app = op(xe);
        let body = vir::expr! {
            ([finite(data, xe)])
                ==> (([finite(data, app)]) && (([data.fp_to_real](app)) == (value)))
        };
        builder.axiom(
            &format!("{name}_real"),
            vir::expr! { forall [x] :: {app} body },
        );
    }
}

/// The exact value of the float with the given bits, or `None` for NaN and the
/// infinities.
fn float_value<'vir>(
    vcx: &'vir VirCtxt<'vir>,
    float: ty::FloatTy,
    bits: u128,
) -> Option<vir::ExprPerm<'vir>> {
    let (exp_bits, p) = float_format(float);
    let (_, emax) = ieee_params(float);
    let width = exp_bits + p;
    let mant_bits = p - 1;
    let mant = bits & ((1 << mant_bits) - 1);
    let exp = (bits >> mant_bits) & ((1 << exp_bits) - 1);
    if exp == (1 << exp_bits) - 1 {
        return None;
    }
    // The value is `mant * 2^shift`; subnormals have the minimum exponent.
    let (mant, shift) = if exp == 0 {
        (mant, 1 - emax as i64 - mant_bits as i64)
    } else {
        (
            mant | (1 << mant_bits),
            exp as i64 - emax as i64 - mant_bits as i64,
        )
    };
    // Reduce the fraction, which keeps the emitted constants readable.
    let tz = if mant == 0 { 0 } else { mant.trailing_zeros() };
    let (mant, shift) = (mant >> tz, shift + i64::from(tz));
    let shift_abs = shift.unsigned_abs() as u32;
    let value = if shift >= 0 && shift_abs < mant.leading_zeros() {
        rational(vcx, int_const(vcx, mant << shift_abs), vcx.mk_int::<1>())
    } else {
        let (mant, scale) = (int_const(vcx, mant), pow2(vcx, shift_abs));
        if shift >= 0 {
            rational(vcx, vir::expr! { (mant) * (scale) }, vcx.mk_int::<1>())
        } else {
            rational(vcx, mant, scale)
        }
    };
    Some(if (bits >> (width - 1)) & 1 == 1 {
        vir::expr! { -(value) }
    } else {
        value
    })
}

/// Emits an axiom stating the exact value `to_real(prim_to_snap(bits))` of a
/// finite float constant (nothing for NaN and the infinities). The axioms of
/// all constants of a float type are emitted in one domain.
pub struct FloatLitEnc;

impl TaskEncoder for FloatLitEnc {
    task_encoder::encoder_cache!(FloatLitEnc);
    const ENCODER_NAME: &'static str = "float literal encoder";

    type TaskDescription<'vir> = (ty::FloatTy, u128);

    type OutputFullLocal<'vir> = Option<(ty::FloatTy, vir::DomainAxiom<'vir>)>;

    type OutputFullDependency<'vir> = ();

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> task_encoder::EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        vir::with_vcx(|vcx| {
            let (float, bits) = *task_key;
            let Some(value) = float_value(vcx, float, bits) else {
                return Ok((None, ()));
            };
            let ty = ty::Ty::new_float(vcx.tcx(), float);
            let domain = *deps
                .require_dep::<TyUsePureEnc>(RustTyDecomposition::from_prim_ty(ty))?
                .expect_float();
            let snap = (domain.prim_to_snap)(int_const(vcx, bits).upcast_ty());
            let axiom = vcx.mk_domain_axiom(
                vir::ViperIdent::new(vir::vir_format!(
                    vcx,
                    "s_Float_{}_lit_{bits:x}",
                    float.name_str()
                )),
                vir::expr! { ([domain.fp_to_real](snap)) == (value) },
            );
            Ok((Some((float, axiom)), ()))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        let mut by_type = std::collections::BTreeMap::<_, Vec<_>>::new();
        for (float, axiom) in Self::all_outputs_local(program).into_iter().flatten() {
            by_type.entry(float.name_str()).or_default().push(axiom);
        }
        vir::with_vcx(|vcx| {
            for (float, mut axioms) in by_type {
                axioms.sort_by_key(|axiom| axiom.name);
                program.add_domain(vcx.mk_domain(
                    vir::ViperIdent::new(vir::vir_format!(vcx, "s_Float_{float}_lits")),
                    &[],
                    vcx.alloc_slice(&axioms),
                    &[],
                    None,
                ));
            }
        });
    }
}
