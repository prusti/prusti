use task_encoder::{EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, Function, FunctionIdn, ViperIdent};

/// Encodes `clamp(x, lo, hi)`, which limits `x` to the interval `[lo, hi]`.
/// Indirect predicates use it to shift an interval of lifetime positions of a
/// type to the positions of one of its fields.
pub struct ClampEnc;

impl TaskEncoder for ClampEnc {
    task_encoder::encoder_cache!(ClampEnc);
    const ENCODER_NAME: &'static str = "clamp encoder";

    type TaskDescription<'vir> = ();
    type OutputFullLocal<'vir> = Function<'vir>;
    type OutputFullDependency<'vir> = FunctionIdn<'vir, (vir::Int, vir::Int, vir::Int), vir::Int>;

    fn task_to_key<'vir>(_task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {}

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        let clamp = FunctionIdn::new(
            ViperIdent::new("clamp"),
            (vir::TYPE_INT, vir::TYPE_INT, vir::TYPE_INT),
            vir::TYPE_INT,
        );
        let function = vir::with_vcx(|vcx| {
            let x_decl = vcx.mk_local_decl("x", vir::TYPE_INT);
            let lo_decl = vcx.mk_local_decl("lo", vir::TYPE_INT);
            let hi_decl = vcx.mk_local_decl("hi", vir::TYPE_INT);
            let x = vcx.mk_local_ex(x_decl);
            let lo = vcx.mk_local_ex(lo_decl);
            let hi = vcx.mk_local_ex(hi_decl);
            // x < lo ? lo : (hi < x ? hi : x)
            let body = vcx.mk_ternary_expr(
                vcx.mk_bin_op_expr(vir::BinOpKind::CmpLt, x, lo)
                    .downcast_ty(),
                lo,
                vcx.mk_ternary_expr(
                    vcx.mk_bin_op_expr(vir::BinOpKind::CmpLt, hi, x)
                        .downcast_ty(),
                    hi,
                    x,
                ),
            );
            vcx.mk_function(
                clamp,
                (x_decl, lo_decl, hi_decl),
                &[],
                &[],
                None,
                Some(body),
            )
        });
        Ok((function, clamp))
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for function in Self::all_outputs_local_no_errors(program) {
            program.add_function(function);
        }
    }
}
