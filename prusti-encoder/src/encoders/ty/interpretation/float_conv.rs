use prusti_rustc_interface::middle::ty;
use task_encoder::TaskEncoder;
use vir::{CastType, FunctionIdn, ViperIdent, vir_format_identifier};

use crate::encoders::ty::{
    RustTyDecomposition,
    interpretation::{
        bitvec::{BitVecDomain, BitVecEnc, BitVecSize},
        float::{ieee_params, int_const, rational, rne_overflow, rne_rounded},
    },
    use_pure::TyUsePureEnc,
};

#[derive(Debug, Clone, Copy)]
pub struct FloatBitVecConv<'vir> {
    pub bitvec: BitVecDomain<'vir>,
    /// `(_ fp.to_sbv N) RTZ`: unspecified for values outside the range of `iN`.
    pub to_sbv: FunctionIdn<'vir, vir::CSnap, vir::CSnap>,
    /// `(_ fp.to_ubv N) RTZ`: unspecified for values outside the range of `uN`.
    pub to_ubv: FunctionIdn<'vir, vir::CSnap, vir::CSnap>,
    /// The float nearest to an integer in the range of `uN` (an `as` cast).
    pub from_uint: FunctionIdn<'vir, vir::Prim, vir::CSnap>,
    /// The float nearest to an integer in the range of `iN` (an `as` cast).
    pub from_sint: FunctionIdn<'vir, vir::Prim, vir::CSnap>,
}

/// Conversions between a float and an integer type of the given width (used by
/// `as` casts), partly via bitvectors. These live in their own domain, separate
/// from the float domain, so that only the bitvector widths which are actually
/// used in casts are emitted.
pub struct FloatBitVecConvEnc;

impl TaskEncoder for FloatBitVecConvEnc {
    task_encoder::encoder_cache!(FloatBitVecConvEnc);
    const ENCODER_NAME: &'static str = "float bitvec conversion encoder";

    type TaskDescription<'vir> = (ty::FloatTy, BitVecSize);

    type OutputFullLocal<'vir> = vir::Domain<'vir>;

    type OutputFullDependency<'vir> = FloatBitVecConv<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut task_encoder::TaskEncoderDependencies<'vir, Self>,
    ) -> task_encoder::EncodeFullResult<'vir, Self> {
        let (float, size) = *task_key;
        deps.emit_output_ref(*task_key, ())?;
        vir::with_vcx(|vcx| {
            let float_ty = ty::Ty::new_float(vcx.tcx(), float);
            let float_snap = deps
                .require_ref::<TyUsePureEnc>(RustTyDecomposition::from_prim_ty(float_ty))?
                .snapshot
                .downcast_ty::<vir::CSnap>();
            let bitvec = deps.require_dep::<BitVecEnc>(size)?;
            let bv_snap = (bitvec.domain)();

            let bits = size.to_bits();
            let (ebits, sbits) = float_format(float);
            let domain_name =
                vir::vir_format!(vcx, "s_Float_{}_BitVec_{bits}_conv", float.name_str());

            let mk_fn = |name: &str, arg, ret, interpretation| {
                let ident = FunctionIdn::new(
                    vir_format_identifier!(vcx, "{domain_name}_{name}"),
                    arg,
                    ret,
                );
                (
                    ident,
                    vcx.mk_domain_function(ident, false, Some(interpretation)),
                )
            };
            let (to_sbv, to_sbv_data) = mk_fn(
                "to_sbv",
                float_snap,
                bv_snap,
                vir::vir_format!(vcx, "(_ fp.to_sbv {bits}) RTZ"),
            );
            let (to_ubv, to_ubv_data) = mk_fn(
                "to_ubv",
                float_snap,
                bv_snap,
                vir::vir_format!(vcx, "(_ fp.to_ubv {bits}) RTZ"),
            );
            let (from_ubv, from_ubv_data) = mk_fn(
                "from_ubv",
                bv_snap,
                float_snap,
                vir::vir_format!(vcx, "(_ to_fp_unsigned {ebits} {sbits}) RNE"),
            );

            // Integer-to-float casts are uninterpreted functions of the
            // integer, defined by their IEEE conversion. Z3 instantiates the
            // definition for `from_sint(t)` with the representative of `t`'s
            // equivalence class, a numeral once the value of `t` is known, and
            // the conversion of a numeral simplifies to a constant; `int2bv` of
            // any other term is expensive for Z3. For other integers, a second
            // axiom bounds the real value of the result by the rounding error.
            // It is limited to the range of the source type, outside of which
            // `int2bv` wraps around.
            let float_domain = *deps
                .require_dep::<TyUsePureEnc>(RustTyDecomposition::from_prim_ty(float_ty))?
                .expect_float();
            let mk_uninterp = |name: &str| {
                let ident = FunctionIdn::new(
                    vir_format_identifier!(vcx, "{domain_name}_{name}"),
                    vir::TYPE_INT.upcast_ty(),
                    float_snap,
                );
                (ident, vcx.mk_domain_function(ident, false, None))
            };
            let (from_uint, from_uint_data) = mk_uninterp("from_uint");
            let (from_sint, from_sint_data) = mk_uninterp("from_sint");

            let n_decl = vcx.mk_local_decl("n", vir::TYPE_INT);
            let n = vcx.mk_local_ex(n_decl);
            let from_nat = |m: vir::ExprInt<'vir>| from_ubv((bitvec.from_int)(m.upcast_ty()));
            let unsigned_def = from_nat(n);
            // Convert the magnitude and negate the result for
            // negative values: Z3 handles `int2bv` of negative
            // integers poorly, and since RNE rounding is symmetric
            // `-round(-x) == round(x)`. The magnitude of `iN::MIN`
            // still fits into an unsigned `N`-bit vector.
            let magnitude = from_nat(vir::expr! { -(n) });
            let signed_def =
                vir::expr! { ((n) < (0)) ? ([float_domain.fp_neg](magnitude)) : (unsigned_def) };
            let n_real = rational(vcx, n, vcx.mk_int::<1>());
            let (_, emax) = ieee_params(float);

            let mut axioms = vec![];
            // The integers in the range have magnitude at most
            // `2^magnitude_bits`.
            for (name, f, def, lo, hi, magnitude_bits) in [
                (
                    "from_uint",
                    from_uint,
                    unsigned_def,
                    vcx.mk_int::<0>(),
                    int_const(vcx, u128::MAX >> (128 - bits)),
                    bits,
                ),
                (
                    "from_sint",
                    from_sint,
                    signed_def,
                    vir::expr! { -([int_const(vcx, 1 << (bits - 1))]) },
                    int_const(vcx, (1 << (bits - 1)) - 1),
                    bits - 1,
                ),
            ] {
                let app = f(n.upcast_ty());
                let ax = |suffix: &str, body: vir::ExprBool<'vir>| {
                    vcx.mk_domain_axiom(
                        ViperIdent::new(vir::vir_format!(vcx, "{domain_name}_{name}_{suffix}")),
                        vir::expr! { forall [n_decl] :: {app} body },
                    )
                };
                axioms.push(ax("def", vir::expr! { (app) == (def) }));
                let mut guard = vir::expr! { ((lo) <= (n)) && ((n) <= (hi)) };
                // A magnitude of at most `2^emax` is below the overflow
                // threshold, so only wider integers need the bound.
                if magnitude_bits > emax {
                    let overflow = rne_overflow(vcx, float);
                    guard = vir::expr! { (guard) && (((-(overflow)) < (n)) && ((n) < (overflow))) };
                }
                axioms.push(ax(
                    "rounded",
                    vir::expr! { (guard) ==> ([rne_rounded(vcx, float_domain, float, app, n_real)]) },
                ));
            }

            let domain_data = vcx.mk_domain::<(), !>(
                ViperIdent::new(domain_name),
                &[],
                vcx.alloc_slice(&axioms),
                vcx.alloc_slice(&[
                    to_sbv_data,
                    to_ubv_data,
                    from_ubv_data,
                    from_uint_data,
                    from_sint_data,
                ]),
                None,
            );

            Ok((
                domain_data,
                FloatBitVecConv {
                    bitvec,
                    to_sbv,
                    to_ubv,
                    from_uint,
                    from_sint,
                },
            ))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for output in Self::all_outputs_local(program) {
            program.add_domain(output);
        }
    }
}

/// The SMT-LIB `(eb, sb)` parameters of a float format (`sb` includes the
/// hidden bit).
pub(crate) fn float_format(float: ty::FloatTy) -> (u32, u32) {
    match float {
        ty::FloatTy::F16 => (5, 11),
        ty::FloatTy::F32 => (8, 24),
        ty::FloatTy::F64 => (11, 53),
        ty::FloatTy::F128 => (15, 113),
    }
}

/// The raw bits of the float `+/-2^exp` (`+/-inf` if it is out of range).
pub fn float_pow2_bits(float: ty::FloatTy, exp: u32, negative: bool) -> u128 {
    let (ebits, sbits) = float_format(float);
    let bias = (1u128 << (ebits - 1)) - 1;
    let biased_exp = if u128::from(exp) > bias {
        (1u128 << ebits) - 1
    } else {
        u128::from(exp) + bias
    };
    let sign = if negative {
        1u128 << (ebits + sbits - 1)
    } else {
        0
    };
    sign | (biased_exp << (sbits - 1))
}
