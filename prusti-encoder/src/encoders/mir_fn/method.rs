use pcg::borrow_pcg::FunctionData;
use prusti_interface::{PrustiError, specs::typed::PureMode};
use prusti_rustc_interface::{middle::mir, span::def_id::DefId};
use task_encoder::{
    EncodeFullError, EncodeFullResult, OutputRefAny, TaskEncoder, TaskEncoderDependencies,
};
use vir::MethodIdn;

use crate::encoders::{
    FunctionCallEnc, Impure, ImpureEncVisitor, MirLocalDefEnc, MirLocalDefEncTask, MirSpecEnc,
    WandEnc, WandEncTask,
    mir_fn::{CallTaskDescription, RustSignature, SpecBlocks, SpecBlocksEnc},
    pure::spec::MirSpecEncMode,
    ty::generics::{
        GArgCaster, GArgs, GArgsCastEnc, GArgsTy, GArgsTyEnc, GParams, GenericParamsEnc,
        trait_impls::TraitImplEnc,
    },
};

// Method wrapper

pub struct MethodCallEnc;

#[derive(Debug, Clone)]
pub struct MethodCallEncOutput<'vir> {
    method: MethodEncOutputRef<'vir>,
    ty_args: GArgsTy<'vir>,
    inputs: Vec<GArgCaster<'vir, Impure>>,
    output: GArgCaster<'vir, Impure>,
}

impl<'vir> MethodCallEncOutput<'vir> {
    pub fn call(
        &self,
        mut args: Vec<vir::ExprRef<'vir>>,
        ret: vir::ExprRef<'vir>,
    ) -> Vec<vir::Stmt<'vir>> {
        assert_eq!(self.inputs.len(), args.len());
        let generics = args.iter().zip(self.inputs.iter());
        let mut stmts: Vec<_> = generics
            .filter_map(|(arg, caster)| caster.cast_to_callee_ctx(arg))
            .collect();

        args.insert(0, ret);
        let call = (self.method.method_ref)(&args, self.ty_args.get_ty(), self.ty_args.get_const())
            .alloc();
        stmts.push(call);

        let result = self.output.cast_to_caller_ctx(ret);
        if let Some(result) = result {
            stmts.push(result);
        }
        stmts
    }
}

impl TaskEncoder for MethodCallEnc {
    task_encoder::encoder_cache!(MethodCallEnc);
    type TaskDescription<'tcx> = CallTaskDescription<'tcx>;
    type OutputFullDependency<'vir> = MethodCallEncOutput<'vir>;

    const ENCODER_NAME: &'static str = "method call encoder";

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        let (callee_def_id, assoc_enc) = task_key.trait_call(deps)?;
        let method_ref = if let Some(assoc_enc) = assoc_enc {
            MethodEncOutputRef {
                method_ref: assoc_enc.call_stub_impure,
            }
        } else {
            let method_ref = deps.require_ref::<MethodEnc>(task_key.callee)?;
            // The method of a callee whose wands cannot be generated is not
            // emitted (see `MethodEnc`), so a call to it cannot be encoded.
            let wands_task = WandEncTask {
                data: FunctionData::new(task_key.callee),
            };
            deps.require_dep::<WandEnc>(wands_task)?;
            method_ref
        };
        let signature = RustSignature::new(callee_def_id);
        let ty_args = deps.require_dep::<GArgsTyEnc>(task_key.gargs)?;
        let inputs = signature
            .inputs
            .iter()
            .map(|ty| {
                let normalized = ty.decompose_compare_normalize(signature.gparams, task_key.gargs);
                deps.require_dep::<GArgsCastEnc<Impure>>(normalized)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let normalized = signature
            .output
            .decompose_compare_normalize(signature.gparams, task_key.gargs);
        let output = deps.require_dep::<GArgsCastEnc<Impure>>(normalized)?;
        Ok((
            (),
            MethodCallEncOutput {
                method: method_ref,
                ty_args,
                inputs,
                output,
            },
        ))
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        MethodEnc::emit_outputs(program);
    }
}

// Method encoder

pub(super) struct MethodEnc;

#[derive(Debug, Clone)]
pub(super) struct MethodEncOutputRef<'vir> {
    method_ref: MethodIdn<'vir, (vir::ManyRef, vir::ManyTyVal, vir::ManyCSnap)>,
}

impl<'vir> OutputRefAny for MethodEncOutputRef<'vir> {}

#[derive(Debug, Clone, Copy)]
pub(super) struct MethodEncOutput<'vir> {
    method: vir::Method<'vir>,
}

impl TaskEncoder for MethodEnc {
    task_encoder::encoder_cache!(MethodEnc);
    const ENCODER_NAME: &'static str = "method encoder";
    type TaskDescription<'tcx> = DefId;

    type OutputRef<'vir> = MethodEncOutputRef<'vir>;
    type OutputFullLocal<'vir> = MethodEncOutput<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        let def_id = *task_key;
        vir::with_vcx(|vcx| {
            let span = vcx.tcx().def_span(def_id);

            let arg_defs = deps.require_ref_spanned::<MirLocalDefEnc>(
                MirLocalDefEncTask::Local {
                    def_id,
                    all_locals: false,
                },
                span,
            )?;

            // Argument count for the Viper method:
            // - one (`Ref`) for the return place;
            // - one (`Ref`) for each MIR argument.
            //
            // Note that the return place is modelled as an argument of the
            // Viper method. This corresponds to an execution model where the
            // method can return data to the caller without a copy--it directly
            // modifies a place provided by the caller.
            //
            // TODO: type parameters: for generic methods we will want to pass
            //   values of type `Type` as well`
            let arg_count = arg_defs.arg_count + 1;

            // Create the identifier and use it as an output ref. This is what
            // is used when other methods call this one.
            let method_name =
                vir::vir_format_identifier!(vcx, "m_{}", vir::ViperIdent::from_def_id(vcx, def_id));
            let ref_args = vcx.alloc_slice(&vec![vir::TYPE_REF; arg_count]);
            let params = GParams::from(def_id);
            let generics = deps.require_dep_spanned::<GenericParamsEnc>(params, span)?;
            let method_ref = MethodIdn::new(
                method_name,
                (ref_args, generics.ty_args(), generics.const_args()),
            );
            deps.emit_output_ref(def_id, MethodEncOutputRef { method_ref })?;

            let arg_defs = deps.require_dep_spanned::<MirLocalDefEnc>(
                MirLocalDefEncTask::Local {
                    def_id,
                    all_locals: false,
                },
                span,
            )?;

            // Method contract. We will need to emit pre- and postconditions for
            // the permissions, the functional spec, and (in the postcondition)
            // wands in case of a reborrowing function.
            let mut pres = Vec::new();
            let mut posts = Vec::new();
            let spec = deps.require_dep_spanned::<MirSpecEnc>(
                (def_id, def_id, MirSpecEncMode::Impure),
                span,
            )?;
            let function_data = FunctionData::new(def_id);
            let wands_task = WandEncTask {
                data: function_data,
            };
            let wands = deps
                .require_dep_spanned::<WandEnc>(wands_task, span)
                .map_err(|err| match err {
                    EncodeFullError::AlreadyEncoded => err,
                    // Without its wands, the method has no sound contract, so
                    // it is not emitted. This is reported here, once; each call
                    // to it reports its own failure (see `MethodCallEnc`).
                    err => EncodeFullError::Reported(WandEnc::report_failure(vcx, def_id, &err)),
                })?;

            // Add direct resources for inputs and outputs to the pre- and
            // postconditions, respectively. "Direct" here refers to owned
            // Viper resources that must be passed in/out given the signature,
            // without going through any dereferences.
            let mut args = Vec::with_capacity(arg_count + params.count());
            for arg_idx in (0..arg_count).map(mir::Local::from) {
                let name_p = arg_defs[arg_idx].local.name;
                args.push(vir::vir_local_decl! { vcx; [name_p] : Ref });
                if arg_idx != mir::RETURN_PLACE {
                    pres.push(arg_defs[arg_idx].impure_pred);
                }
            }
            posts.push(arg_defs[mir::RETURN_PLACE].impure_pred);

            // ..
            pres.extend(wands.indirect_pres(vcx, &arg_defs, deps));
            posts.extend(wands.indirect_posts(vcx, &arg_defs, deps));
            posts.extend(wands.wand_posts(vcx, &arg_defs, deps));

            // The method of a pure function `f` gets the postcondition
            //     ensures snap(result) == f_(old(snap(args)))
            // with the addresses of `&T` arguments nulled unless `f` is in
            // memory mode, exactly as a call `f(args)` in a specification is
            // encoded. So an impure call learns the same value; the body
            // proves it like any other postcondition.
            let mode = crate::encoders::pure_mode(def_id, GArgs::new(params, params.rust_params()));
            let tie = if let Some(mode) = mode {
                let pure_func = deps.require_dep_spanned::<FunctionCallEnc>(
                    CallTaskDescription::new(def_id, params.rust_params(), def_id)
                        .resolve_trait_calls(false),
                    span,
                )?;
                let arg_snaps = (1..arg_count)
                    .map(mir::Local::from)
                    .map(|arg_idx| vcx.mk_old_expr(arg_defs[arg_idx].impure_snap))
                    .collect::<Vec<_>>();
                let name = vcx.tcx().def_path_str(def_id);
                let error = match mode {
                    PureMode::Value => {
                        let error = PrustiError::verification(
                            format!(
                                "the result of pure function `{name}` might depend on memory \
                                 addresses, which the result of a `#[pure]` function must not"
                            ),
                            span.into(),
                        );
                        match memory_mode_help(def_id, pure_func.dummies_addresses()) {
                            Some(help) => error.set_help(help),
                            None => error,
                        }
                    }
                    // The arguments keep their addresses, but a borrow of a
                    // local in pure code has no address.
                    PureMode::Memory => PrustiError::verification(
                        format!(
                            "the result of pure function `{name}` might depend on the addresses \
                             of its local variables, which Prusti does not yet support in pure \
                             functions"
                        ),
                        span.into(),
                    ),
                };
                Some(vcx.with_span(span, |vcx| {
                    vcx.add_error_taint(pure_func.body_taint());
                    vcx.handle_error("postcondition.violated:assertion.false", move |_| {
                        Some(vec![error.clone()])
                    });
                    vcx.mk_eq_expr(
                        arg_defs[mir::RETURN_PLACE].impure_snap,
                        pure_func.call_pure(arg_snaps),
                    )
                }))
            } else {
                None
            };

            // Trusted functions, call stubs, external functions and trait
            // functions without a default implementation have no body to
            // encode; only their contract is emitted.
            let blocks = if let Some(body_with_facts) =
                crate::encoders::impure_body_with_facts(def_id)
            {
                let body = &body_with_facts.body;
                let local_defs = deps.require_dep_spanned::<MirLocalDefEnc>(
                    MirLocalDefEncTask::Local {
                        def_id,
                        all_locals: true,
                    },
                    span,
                )?;

                let pcg_creator = pcg::PcgCtxtCreator::new(vcx.tcx());
                let pcg_ctxt = pcg_creator.new_nll_ctxt(&body_with_facts);
                let fpcs_analysis = pcg::run_pcg(pcg_ctxt);
                pcg_ctxt.update_debug_visualization_metadata();

                let block_count = body.basic_blocks.len();

                let mut encoded_blocks = Vec::with_capacity(
                    // extra blocks: Start, End
                    2 + block_count,
                );
                let mut start_stmts = TraitImplEnc::assume_context_bounds(vcx, deps, params)?;
                for local in (arg_count..body.local_decls.len()).map(mir::Local::from) {
                    // Spec-only locals have no definition.
                    let Some(local_def) = local_defs.get(local) else {
                        continue;
                    };
                    let name_p = local_def.local.name;
                    start_stmts.push(
                        vcx.mk_local_decl_stmt(vir::vir_local_decl! { vcx; [name_p] : Ref }, None),
                    )
                }
                // This will be overwritten later.
                encoded_blocks.push(vcx.mk_cfg_block(
                    &vir::CfgBlockLabelData::Start,
                    &[],
                    &[],
                    vcx.mk_goto_stmt(&vir::CfgBlockLabelData::BasicBlock(0)),
                ));

                let spec_blocks = SpecBlocks::new(
                    deps.require_dep::<SpecBlocksEnc>(def_id)?,
                    body,
                    fpcs_analysis.analysis().loop_analysis(),
                );

                deps.check_cycle()?;
                let mut visitor = ImpureEncVisitor {
                    vcx,
                    deps,
                    def_id,
                    local_decls: &body.local_decls,
                    fpcs_analysis,
                    local_defs,
                    spec_blocks,
                    body,

                    wands,

                    tmp_ctr: 0,
                    label_ctr: 0,
                    call_labels: Default::default(),
                    wandless_calls: Default::default(),
                    from_to_vars: Default::default(),

                    current_block: None,
                    current_block_pres: None,
                    current_block_succs: None,
                    current_block_label: None,
                    current_fpcs: None,

                    current_stmts: None,
                    current_terminator: None,
                    encoded_blocks,
                };
                // if we encountered an error/cycle during encoding, we don't
                // emit a method body; encoding errors additionally surface as
                // early errors rather than silently degrading to a stub
                match visitor.visit_body(body) {
                    Ok(()) => {
                        start_stmts.extend(
                            visitor
                                .from_to_vars
                                .decls()
                                .map(|v| vcx.mk_local_decl_stmt(v, Some(vcx.mk_bool::<false>()))),
                        );
                        visitor.encoded_blocks[0] = vcx.mk_cfg_block(
                            &vir::CfgBlockLabelData::Start,
                            &[],
                            vcx.alloc_slice(&start_stmts),
                            vcx.mk_goto_stmt(&vir::CfgBlockLabelData::BasicBlock(0)),
                        );

                        visitor.encoded_blocks.push(vcx.mk_cfg_block(
                            vcx.alloc(vir::CfgBlockLabelData::End),
                            &[],
                            &[],
                            vcx.alloc(vir::TerminatorStmtData::Exit),
                        ));

                        visitor.deps.check_cycle()?;

                        Some(visitor.encoded_blocks)
                    }
                    Err(EncodeFullError::AlreadyEncoded) => None,
                    Err(err) => {
                        super::report_dep_error(vcx, &err, |err| {
                            let span = err.span.unwrap_or_else(|| vcx.tcx().def_span(def_id));
                            // Worded like `FunctionEnc`'s error, so that when both
                            // encodings of a pure function's body fail in the same
                            // way, rustc reports the identical error only once.
                            let name = vcx.tcx().def_path_str(def_id);
                            err.report(format!("function body `{name}`"), span)
                        });
                        None
                    }
                }
            } else {
                None
            };

            // Add functional specification as the last pre- and postconditions.
            pres.extend(spec.pre_exprs());
            posts.extend(spec.post_exprs());
            // After the functional postconditions: applying `f_` assumes its
            // (unchecked) postconditions, so they must be checked first.
            // In value mode, proving the tie still assumes them on the dummied
            // arguments.
            // A postcondition that holds on the real arguments but not on the
            // dummied ones (e.g. `x as *const i32 != null`) is then assumed
            // false, which makes the tie vacuously provable and lets callers
            // derive false.
            posts.extend(tie);

            Ok((
                MethodEncOutput {
                    method: vcx.mk_method(
                        method_ref,
                        (&args, generics.ty_decls(), generics.const_decls()),
                        &[],
                        vcx.alloc_slice(&pres),
                        vcx.alloc_slice(&posts),
                        blocks.map(|blocks| vcx.alloc_slice(&blocks)),
                    ),
                },
                (),
            ))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for output in Self::all_outputs_local(program) {
            program.add_method(output.method);
        }
    }
}

/// The help for a `#[pure]` function `def_id` whose result might depend on
/// memory addresses: declaring it `#[pure(memory)]` only helps if its calls
/// dummy the address of some argument, and an implementation of a trait method
/// may only be in memory mode if the trait method is.
fn memory_mode_help(def_id: DefId, dummies_addresses: bool) -> Option<String> {
    if !dummies_addresses {
        return None;
    }
    let value_mode_trait_method = vir::with_vcx(|vcx| vcx.tcx().opt_associated_item(def_id))
        .and_then(|item| item.trait_item_def_id())
        .filter(|&trait_item| {
            let trait_params = GParams::from(trait_item);
            crate::encoders::pure_mode(
                trait_item,
                GArgs::new(trait_params, trait_params.rust_params()),
            ) == Some(PureMode::Value)
        });
    let to_declare = match value_mode_trait_method {
        None => "the function `#[pure(memory)]`",
        Some(trait_item) if trait_item.is_local() => {
            "the trait method it implements `#[pure(memory)]` (and this function too, if it is \
             annotated `#[pure]`)"
        }
        // The specification of a foreign trait method is fixed.
        Some(_) => return None,
    };
    Some(format!(
        "if the result is meant to depend on the addresses of shared-reference arguments (e.g. \
         it is a reference into an argument), declare {to_declare}"
    ))
}
