use prusti_interface::PrustiError;
use prusti_rustc_interface::span::def_id::DefId;
use task_encoder::{EncodeFullResult, OutputRefAny, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, FunctionIdn, Reify};

use crate::encoders::{
    MirLocalDefEnc, MirLocalDefEncTask, MirPureEnc, MirPureEncTask, MirSpecEnc, Pure, PureKind,
    TyUsePureEnc,
    mir_fn::{CallTaskDescription, RustSignature},
    pure::spec::MirSpecEncMode,
    ty::{
        generics::{GArgCaster, GArgsTy, GArgsTyEnc, GParams, GenericParamsEnc},
        use_pure::{TyUsePure, TyUsePureImmRef},
    },
};

// Function wrapper

pub struct FunctionCallEnc;

#[derive(Debug, Clone)]
pub struct FunctionCallEncOutput<'vir> {
    function: FunctionEncOutputRef<'vir>,
    ty_args: GArgsTy<'vir>,
    /// The (instantiated) types of the arguments (see `shallow_arg`).
    arg_tys: Vec<TyUsePure<'vir>>,
    /// The (instantiated) types of the arguments the callee declares as
    /// shared references, whose addresses are dummied.
    shared_ref_args: Vec<Option<TyUsePureImmRef<'vir>>>,
    inputs: Vec<GArgCaster<'vir, Pure>>,
    output: GArgCaster<'vir, Pure>,
}

impl<'vir> FunctionCallEncOutput<'vir> {
    /// The index of an argument in `args` whose deep snapshot the callee
    /// takes, but which only has a shallow one (see `TyPureRef`). A generic
    /// parameter takes the shallow snapshot.
    pub fn shallow_arg<Curr, Next>(
        &self,
        args: &[vir::ExprGenSnap<'vir, Curr, Next>],
    ) -> Option<usize> {
        args.iter()
            .zip(&self.inputs)
            .zip(&self.arg_tys)
            .position(|((arg, caster), ty)| caster.is_no_cast() && ty.is_shallow(*arg))
    }

    /// Calls the definitional function `f_`. In impure code a pure function
    /// is called as a method (its `MethodEnc`), whose postcondition ties the
    /// result to this application.
    pub fn call_pure<Curr, Next>(
        &self,
        mut args: Vec<vir::ExprGenSnap<'vir, Curr, Next>>,
    ) -> vir::ExprGenSnap<'vir, Curr, Next> {
        let function = self.function.function_ref;
        assert_eq!(self.inputs.len(), args.len());
        for ((arg, caster), ty) in args
            .iter_mut()
            .zip(self.inputs.iter())
            .zip(self.shared_ref_args.iter())
        {
            if let Some(ty) = ty {
                *arg = ty.value_snap_of(arg.downcast_ty()).upcast_ty();
            }
            *arg = caster.cast_to_callee_ctx(*arg);
        }
        let call = function.call()(&args, self.ty_args.get_ty(), self.ty_args.get_const());
        self.output.cast_to_caller_ctx(call)
    }
}

impl TaskEncoder for FunctionCallEnc {
    task_encoder::encoder_cache!(FunctionCallEnc);
    const ENCODER_NAME: &'static str = "function call encoder";
    type TaskDescription<'tcx> = CallTaskDescription<'tcx>;
    type OutputFullDependency<'vir> = FunctionCallEncOutput<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        let (callee_def_id, assoc_enc) = task_key.trait_call(deps)?;
        let function_ref = if let Some(assoc_enc) = assoc_enc {
            FunctionEncOutputRef {
                function_ref: assoc_enc.call_stub_pure_function.unwrap(),
            }
        } else {
            deps.require_ref::<FunctionEnc>(task_key.callee)?
        };
        let signature = RustSignature::new(callee_def_id);
        let ty_args = deps.require_dep::<GArgsTyEnc>(task_key.gargs)?;
        let (inputs, output) = signature.casters(task_key.gargs, deps)?;
        let arg_tys = signature
            .inputs
            .iter()
            .map(|ty| deps.require_dep::<TyUsePureEnc>(ty.decompose_normalize(task_key.gargs)))
            .collect::<Result<Vec<_>, _>>()?;
        // Only arguments declared as shared references are dummied, not ones
        // of a type parameter instantiated with a reference: the dummying
        // must agree with the callee's (generic) contract, which ties its
        // result to `f_` on the same dummied arguments.
        let shared_ref_args = signature
            .inputs
            .iter()
            .zip(&arg_tys)
            .map(|(ty, arg_ty)| ty.is_immref().then(|| *arg_ty.expect_immref()))
            .collect();
        Ok((
            (),
            FunctionCallEncOutput {
                function: function_ref,
                ty_args,
                inputs,
                output,
                arg_tys,
                shared_ref_args,
            },
        ))
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        FunctionEnc::emit_outputs(program);
    }
}

// Function encoder

struct FunctionEnc;

#[derive(Debug, Clone)]
struct FunctionEncOutputRef<'vir> {
    function_ref: FunctionIdn<'vir, (vir::ManySnap, vir::ManyTyVal, vir::ManyCSnap), vir::Snap>,
}

impl<'vir> OutputRefAny for FunctionEncOutputRef<'vir> {}

#[derive(Debug, Clone, Copy)]
struct FunctionEncOutput<'vir> {
    function: vir::Function<'vir>,
}

#[derive(Clone, Debug)]
pub enum FunctionEncError {}

impl TaskEncoder for FunctionEnc {
    task_encoder::encoder_cache!(FunctionEnc);
    const ENCODER_NAME: &'static str = "function encoder";
    type TaskDescription<'tcx> = DefId;

    type OutputRef<'vir> = FunctionEncOutputRef<'vir>;
    type OutputFullLocal<'vir> = FunctionEncOutput<'vir>;

    type EncodingError = FunctionEncError;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        vir::with_vcx(|vcx| {
            let def_id = *task_key;
            let local_defs = deps.require_dep::<MirLocalDefEnc>(MirLocalDefEncTask::Local {
                def_id,
                all_locals: true,
            })?;

            tracing::debug!("encoding {def_id:?}");

            let name = vir::ViperIdent::from_def_id(vcx, def_id);
            let function_ident = vir::vir_format_identifier!(vcx, "f_{name}");
            let arg_types = vcx.alloc_slice(&local_defs.snap_ty_args().collect::<Vec<_>>());
            let return_type = local_defs.snap_ty_return();
            let params = GParams::from(def_id);
            let generics = deps.require_dep::<GenericParamsEnc>(params)?;
            let function_ref = FunctionIdn::new(
                function_ident,
                (arg_types, generics.ty_args(), generics.const_args()),
                return_type,
            );
            deps.emit_output_ref(def_id, FunctionEncOutputRef { function_ref })?;

            let spec =
                deps.require_dep::<MirSpecEnc>((def_id, def_id, MirSpecEncMode::PureWithResult))?;

            let expr = if !crate::encoders::encodes_body(def_id) {
                None
            } else {
                // Encode the body of the function. If it cannot be encoded (e.g. it
                // uses an unsupported feature), report it and emit the function
                // abstractly (keeping its contract) rather than failing entirely
                // (which would leave a dangling reference for callers).
                match deps.require_dep::<MirPureEnc>(MirPureEncTask {
                    encoding_depth: 0,
                    kind: PureKind::Pure,
                    parent_def_id: def_id,
                    gargs: params.identity_args(),
                }) {
                    Ok(out) => {
                        let expr = out
                            .expr
                            .reify(vcx, (def_id, spec.pre_args, vir::OldLabel::None));
                        assert!(
                            expr.ty() == return_type,
                            "expected {:?}, got {:?}",
                            return_type,
                            expr.ty()
                        );
                        Some(expr)
                    }
                    Err(err) => {
                        let (message, span) = super::dep_error(&err);
                        vcx.emit_early_error(PrustiError::unsupported(
                            format!(
                                "cannot encode function body `{}`: {message}",
                                vcx.tcx().def_path_str(def_id),
                            ),
                            span.unwrap_or_else(|| vcx.tcx().def_span(def_id)).into(),
                        ));
                        None
                    }
                }
            };

            tracing::debug!("finished {def_id:?}");

            let posts = spec
                .posts
                .iter()
                .map(|(post, _)| {
                    // use inhale-exhale expression to prevent viper checking that
                    // the function body expression satisfies the postcondition:
                    // that's checked in the method encoding of this function.
                    vcx.mk_inhale_exhale_expr(*post, vcx.mk_bool::<true>())
                })
                .collect::<Vec<_>>();
            let posts = vcx.alloc_slice(&posts);

            let func_args = local_defs.local_decl_args().collect::<Vec<_>>();
            let function = vcx.mk_function(
                function_ref,
                (&func_args, generics.ty_decls(), generics.const_decls()),
                &[],
                posts,
                expr.is_none().then_some(&vir::DecreasesGenData::Star),
                expr,
            );
            Ok((FunctionEncOutput { function }, ()))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for output in Self::all_outputs_local_no_errors(program) {
            program.add_function(output.function);
        }
    }
}
