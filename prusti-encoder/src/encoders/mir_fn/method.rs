use pcg::borrow_pcg::FunctionData;
use prusti_interface::PrustiError;
use prusti_rustc_interface::{dataflow, middle::mir, span::def_id::DefId};
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
            deps.require_ref::<MethodEnc>(task_key.callee)?
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

#[derive(Clone, Debug)]
pub enum MethodEncError {
    /// The method cannot be encoded; this was reported as an early error.
    Reported,
}

impl TaskEncoder for MethodEnc {
    task_encoder::encoder_cache!(MethodEnc);
    const ENCODER_NAME: &'static str = "method encoder";
    type TaskDescription<'tcx> = DefId;

    type OutputRef<'vir> = MethodEncOutputRef<'vir>;
    type OutputFullLocal<'vir> = MethodEncOutput<'vir>;

    type EncodingError = MethodEncError;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn error_reported(error: &Self::EncodingError) -> bool {
        matches!(error, MethodEncError::Reported)
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
            // Interior-mutable values inside another's contents are not
            // supported yet (see `im::nested_im`): the body is not encoded,
            // the contract is (for the callers).
            let tys = match crate::encoders::impure_body_with_facts(def_id) {
                Some(body) => body.body.local_decls.iter().map(|decl| decl.ty).collect(),
                None => vcx
                    .tcx()
                    .fn_sig(def_id)
                    .instantiate_identity()
                    .skip_normalization()
                    .skip_binder()
                    .inputs_and_output
                    .to_vec(),
            };
            let nested = tys
                .into_iter()
                .find_map(|ty| crate::encoders::ty::im::nested_im(ty, params));
            if let Some(nested) = nested {
                vcx.emit_early_error(PrustiError::unsupported(
                    format!("interior-mutable values inside the contents of `{nested}`"),
                    span.into(),
                ));
            }
            let function_data = FunctionData::new(def_id);
            let wands = deps
                .require_dep_spanned::<WandEnc>(
                    WandEncTask {
                        data: function_data,
                    },
                    span,
                )
                .map_err(|err| {
                    // Without its wands, the method has no contract.
                    let (message, _) = super::dep_error(&err);
                    vcx.emit_early_error(PrustiError::unsupported(
                        format!(
                            "cannot encode method `{}`: {message}",
                            vcx.tcx().def_path_str(def_id),
                        ),
                        vcx.tcx().def_span(def_id).into(),
                    ));
                    EncodeFullError::EncodingError(MethodEncError::Reported, None)
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
            // The caller allocates the return place.
            pres.push(arg_defs[mir::RETURN_PLACE].impure_uninit);
            posts.push(arg_defs[mir::RETURN_PLACE].impure_pred);

            // ..
            pres.extend(wands.indirect_pres(vcx, &arg_defs, deps));
            posts.extend(wands.indirect_posts(vcx, &arg_defs, deps));
            posts.extend(wands.wand_posts(vcx, &arg_defs, deps));
            // The arguments borrowed by the result.
            let blocked = wands
                .blocked_inputs()
                .iter()
                .map(|input| input.mir_local())
                .collect::<Vec<_>>();

            // The method of a pure function ties its result to the
            // definitional function `f_` (see `FunctionEnc`), so a caller in
            // impure code learns the result's value from the method call. The
            // callee proves nothing for it: the function's body is this
            // method's body by construction. Closures have no `f_`.
            let is_pure =
                crate::encoders::is_function_pure(def_id, GArgs::new(params, params.rust_params()));
            if is_pure && !vcx.tcx().is_closure_like(def_id) {
                let pure_func = deps.require_dep_spanned::<FunctionCallEnc>(
                    CallTaskDescription::new(def_id, params.rust_params(), def_id)
                        .resolve_trait_calls(false),
                    span,
                )?;
                // A `#[pure_unstable]` function sees deep values.
                let deep = crate::encoders::is_pure_unstable(def_id);
                let arg_snaps = (1..arg_count)
                    .map(mir::Local::from)
                    .map(|arg_idx| {
                        let arg = &arg_defs[arg_idx];
                        let snap = if deep {
                            crate::encoders::ty::im::deep_snap(
                                deps,
                                arg.ty,
                                arg.local_ex,
                                arg.impure_snap,
                            )?
                        } else {
                            arg.impure_snap
                        };
                        Ok(vcx.mk_old_expr(snap))
                    })
                    .collect::<Result<Vec<_>, EncodeFullError<'vir, MethodEnc>>>()?;
                posts.push(vcx.mk_inhale_exhale_expr(
                    vcx.mk_eq_expr(
                        arg_defs[mir::RETURN_PLACE].impure_snap,
                        pure_func.call_pure(arg_snaps),
                    ),
                    vcx.mk_bool::<true>(),
                ));
            }

            // Trusted functions, call stubs, external functions and trait
            // functions without a default implementation have no body to
            // encode; only their contract is emitted.
            let body_with_facts = crate::encoders::impure_body_with_facts(def_id);
            let blocks = if nested.is_none()
                && let Some(body_with_facts) = body_with_facts
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
                // Other locals receive their `Uninit` token at `StorageLive`.
                let always_live = dataflow::impls::always_storage_live_locals(body);
                let mut start_stmts = TraitImplEnc::assume_context_bounds(vcx, deps, params)?;
                for local in (arg_count..body.local_decls.len()).map(mir::Local::from) {
                    // Spec-only locals have no definition.
                    let Some(local_def) = local_defs.get(local) else {
                        continue;
                    };
                    let name_p = local_def.local.name;
                    start_stmts.push(
                        vcx.mk_local_decl_stmt(vir::vir_local_decl! { vcx; [name_p] : Ref }, None),
                    );
                    if always_live.contains(local) {
                        start_stmts.push(vcx.mk_inhale_stmt(local_defs[local].impure_uninit));
                    }
                }
                // An `#[im_open]` function works on the opened atom (closed
                // again at its return).
                if let Some(param) = crate::encoders::ty::im::im_open_param(def_id) {
                    let arg = &local_defs[param];
                    match crate::encoders::ty::im::im_open_referent(deps, arg.ty, arg.impure_snap)? {
                        Some((referent, addr)) => start_stmts.extend(
                            crate::encoders::ty::im::open_or_close(deps, referent, addr, true)?,
                        ),
                        None => vcx.emit_early_error(PrustiError::unsupported(
                            "`#[im_open]` names a parameter that is no shared reference to an \
                             `#[im_opaque]` value",
                            span.into(),
                        )),
                    }
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
                    deferred_join_stmts: Default::default(),
                    entry_vars: Default::default(),
                    blocks_encoded: Default::default(),
                    array_indices: Default::default(),

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
                        start_stmts.extend(
                            visitor
                                .entry_vars
                                .decls()
                                .map(|v| vcx.mk_local_decl_stmt(v, None)),
                        );
                        start_stmts.extend(
                            visitor
                                .array_indices
                                .values()
                                .map(|v| vcx.mk_local_decl_stmt(v, None)),
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
                        let (message, span) = super::dep_error(&err);
                        vcx.emit_early_error(PrustiError::unsupported(
                            format!(
                                "cannot encode method body `{}`: {message}",
                                vcx.tcx().def_path_str(def_id),
                            ),
                            span.unwrap_or_else(|| vcx.tcx().def_span(def_id)).into(),
                        ));
                        None
                    }
                }
            } else {
                None
            };

            // The IM footprints (address-keyed atoms): the call receives the
            // atoms of everything it is given, and returns those of the
            // result and of everything behind references in the arguments.
            let mut fp_pre = Vec::new();
            let mut fp_post = Vec::new();
            let im_open = crate::encoders::ty::im::im_open_param(def_id);
            for arg_idx in (1..arg_count).map(mir::Local::from) {
                let arg = &arg_defs[arg_idx];
                if !crate::encoders::ty::im::reaches_im(arg.ty) {
                    continue;
                }
                let entries = crate::encoders::ty::im::footprint(
                    deps,
                    arg.ty,
                    arg.local_ex,
                    arg.impure_snap,
                )?;
                fp_pre.extend(entries.iter().copied());
                // What the result borrows is lent to it: its wand returns the
                // atoms the argument holds alone. Shared atoms are not
                // blocked; as the argument cannot be read until the borrow
                // expires, they are returned at their keys in the pre-state.
                if blocked.contains(&arg_idx) {
                    fp_post.extend(
                        entries
                            .iter()
                            .filter(|e| e.reached && e.shared)
                            .map(|e| e.in_state(vir::OldLabel::None)),
                    );
                    continue;
                }
                let old_entries = crate::encoders::ty::im::footprint(
                    deps,
                    arg.ty,
                    arg.local_ex,
                    vcx.mk_old_expr(arg.impure_snap),
                )?;
                fp_post.extend(old_entries.into_iter().filter(|e| e.reached));
            }
            // An `#[im_open]` function's result points into the opened
            // contents, which it returns already.
            if im_open.is_none()
                && crate::encoders::ty::im::reaches_im(arg_defs[mir::RETURN_PLACE].ty)
            {
                let result = &arg_defs[mir::RETURN_PLACE];
                fp_post.extend(crate::encoders::ty::im::footprint(
                    deps,
                    result.ty,
                    result.local_ex,
                    result.impure_snap,
                )?);
            }
            if !fp_pre.is_empty() {
                pres.push(crate::encoders::ty::im::footprint_assertion(&fp_pre));
            }
            if !fp_post.is_empty() {
                posts.push(crate::encoders::ty::im::footprint_assertion(&fp_post));
            }
            let defs = (0..arg_count)
                .map(|idx| arg_defs[mir::Local::from(idx)])
                .collect::<Vec<_>>();
            let (cell_pres, cell_posts) =
                crate::encoders::ty::im::unsafe_cell_contract(deps, def_id, &defs)?;
            pres.extend(cell_pres);
            posts.extend(cell_posts);
            // A pure function changes no interior-mutable state: the deep
            // values of its arguments are framed.
            if is_pure {
                for arg_idx in (1..arg_count).map(mir::Local::from) {
                    let arg = &arg_defs[arg_idx];
                    if !crate::encoders::ty::im::reaches_im(arg.ty) {
                        continue;
                    }
                    // The argument itself is consumed: its snapshot is read
                    // in the pre-state, its contents in both.
                    let snap = vcx.mk_old_expr(arg.impure_snap);
                    let deep =
                        crate::encoders::ty::im::deep_snap(deps, arg.ty, arg.local_ex, snap)?;
                    posts.push(vcx.mk_inhale_exhale_expr(
                        vcx.mk_eq_expr(deep, vcx.mk_old_expr(deep)),
                        vcx.mk_bool::<true>(),
                    ));
                }
            }

            // Add the functional specification as the last pre- and
            // postconditions, after the footprints that grant the permissions
            // its interior-mutable reads need.
            pres.extend(spec.pre_exprs());
            posts.extend(spec.post_exprs());

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
        for output in Self::all_outputs_local_no_errors(program) {
            program.add_method(output.method);
        }
    }
}
