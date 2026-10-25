use crate::encoders::{
    EncodeResult, ImpureEncVisitor, MirLocalDefEncOutput, MirSpecEnc,
    pure::spec::{EncodedPledge, MirSpecEncMode, PledgeArgs, PledgeExpr},
    ty::{
        RustTyDecomposition,
        generics::{GArgs, GArgsTyEnc, GParams, GenericParamsEnc},
        im::{deep_snap, lent_footprint, on_expiry},
        indirect::{IndirectPredicatesEnc, projection_for_generalized_idx},
    },
};
use pcg::borrow_pcg::{
    FunctionData, FunctionShape, FunctionShapeInput, FunctionShapeNode, FunctionShapeOutput,
    GeneralizedLifetime, MakeFunctionShapeError,
    region_projection::{ExtractRegionsCtxt, Generalized, PcgRegion},
    state::BorrowsState,
    unblock_graph::UnblockGraph,
};
use prusti_interface::PrustiError;
use prusti_rustc_interface::{
    data_structures::fx::{FxHashMap, FxHashSet},
    index::{Idx, IndexVec},
    middle::{mir, ty},
    span::def_id::DefId,
};
use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, HasType};

/// Encodes the magic wands given a function signature.
pub struct WandEnc;

#[derive(Clone, Debug)]
pub enum WandEncError {
    Unsupported(String),
}

impl<'vir, E: TaskEncoder> ImpureEncVisitor<'vir, '_, E> {
    pub fn package_wands(
        &mut self,
        final_borrow_state: &BorrowsState<'_, 'vir>,
    ) -> EncodeResult<'vir, Vec<vir::Stmt<'vir>>, E> {
        let mut wand_packages = Vec::new();
        let label = self.new_label("package_post");
        let result = self.local_defs[mir::RETURN_PLACE].impure_snap;
        let result = self.vcx.mk_local_labelled_old_expr(result, label);
        let args = self
            .local_defs
            .args()
            .map(|a| self.vcx.mk_old_expr(a.impure_snap));
        let args = PledgeExpr::pledge_args(result, args);

        for wand_data in self.wands.viper_wands() {
            let Some(wand) = self
                .wands
                .mk_wand(&wand_data, args, None, None, self.vcx, self.deps)
            else {
                continue;
            };
            let mut package_script = Vec::new();
            for rhs in wand_data.rhs.iter() {
                let ug = UnblockGraph::for_node(
                    mir::Place::from(rhs.mir_local()),
                    final_borrow_state,
                    self.pcg_ctxt(),
                );
                let actions = ug.actions(self.pcg_ctxt()).unwrap();
                let unblock = self.block(|visitor| {
                    visitor.pcs_unblock_actions(final_borrow_state, &actions, Some(label))
                })?;
                package_script.extend(unblock);
            }

            let lent = self
                .wands
                .lent_footprints(&wand_data, args, None, self.vcx, self.deps);
            if !wand_data.pledges.is_empty() || !lent.is_empty() {
                // Statements in the package script only see resources already
                // in the package state. A resource that is not obtained from
                // the LHS (e.g. an argument the result does not borrow from)
                // is only moved there when consumed, which for the RHS happens
                // after the script. Asserting the RHS resources moves them in
                // early, so that the pledge exhales below can read them.
                // Likewise for the atoms lent to the result that it does not
                // reach, which the deep values in the RHS read.
                let resources = wand_data
                    .rhs
                    .iter()
                    .filter_map(|g| {
                        self.wands.encode_predicates_for_function_shape_node(
                            self.vcx,
                            self.deps,
                            *g,
                            None,
                            |i| args[i],
                        )
                    })
                    .chain(lent)
                    .collect::<Vec<_>>();
                package_script.push(self.vcx.mk_assert_stmt(self.vcx.mk_conj(&resources)));
            }

            for EncodedPledge {
                expiry_postcondition,
                ..
            } in &wand_data.pledges
            {
                let span = expiry_postcondition.span();
                self.vcx.with_span(span, |vcx| {
                    vcx.handle_error("exhale.failed:assertion.false", move |_| {
                        Some(vec![PrustiError::verification(
                            "pledge postcondition might not hold",
                            span.into(),
                        )])
                    });
                    package_script.push(vcx.mk_exhale_stmt(expiry_postcondition.expr(args)));
                });
            }
            wand_packages.push(
                self.vcx
                    .mk_package_stmt(wand, self.vcx.alloc_slice(&package_script)),
            );
        }
        Ok(wand_packages)
    }
}

type EncodedPledges<'vir> = Vec<EncodedPledge<'vir>>;

/// Not tied to a caller or callee context. `indirect_pres`, `indirect_posts`,
/// `wand_posts`, and `package_wands` are identity-substituted and intended for
/// use in the callee's own contract; `apply_wands` is for caller use and
/// re-substitutes via a [`WandCallContext`].
#[derive(Clone)]
pub struct WandEncOutput<'vir> {
    /// Information about the corresponding function.
    function_data: FunctionData<'vir>,

    /// The lifetime projections of all arguments to the function.
    inputs: Vec<FunctionShapeInput<Generalized>>,

    /// The lifetime projections of all function outputs (according to the
    /// corresponding [`FunctionShape`]). This *includes* lifetime projections
    /// of nested lifetimes in the function arguments.
    outputs: Vec<FunctionShapeOutput<Generalized>>,

    /// Encoded VIR expressions for the magic wands.
    wands: Vec<WandData<'vir>>,
}

/// Substitution context for instantiating a wand at a call site. When `None`,
/// the wand is encoded using the callee's identity substitution (appropriate
/// when emitting wands inside the function being defined). When `Some`, the
/// wand is re-encoded with the call-site substitutions and the caller's
/// generic parameters, so that placeholders like `Self` or other callee
/// generics are replaced by concrete types from the caller's perspective.
pub type WandCallContext<'vir> = Option<GArgs<'vir>>;

impl<'vir> WandEncOutput<'vir> {
    pub(crate) fn fn_sig(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        call_ctx: WandCallContext<'vir>,
    ) -> ty::FnSig<'vir> {
        match call_ctx {
            // TODO: change pcg's `fn_sig` to take `&[GenericArg]` instead of `GenericArgsRef`
            Some(ctx) => self
                .function_data
                .fn_sig(vcx.tcx(), vcx.tcx().mk_args(ctx.args())),
            None => self.function_data.identity_fn_sig(vcx.tcx()),
        }
    }

    pub(crate) fn g_params(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        call_ctx: WandCallContext<'vir>,
    ) -> GParams<'vir> {
        match call_ctx {
            Some(ctx) => ctx.context(),
            None => GParams::new(
                self.function_data.identity_substs(vcx.tcx()),
                self.function_data.param_env(vcx.tcx()),
                false,
            )
            .with_im_contents(crate::encoders::ty::im::im_contents_params(
                self.function_data.def_id(),
            )),
        }
    }

    /// The (unreified) predicates associated with the given node, or `None` if
    /// there are no resources associated with it.
    #[allow(clippy::type_complexity)]
    fn predicates_for_function_shape_node(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, impl TaskEncoder>,
        g: FunctionShapeNode<Generalized>,
        call_ctx: WandCallContext<'vir>,
    ) -> Option<Vec<vir::ExprGenBool<'vir, vir::ExprSnap<'vir>, vir::ExprKind<'vir>>>> {
        let arg_ty = g.ty(self.fn_sig(vcx, call_ctx));
        let decomp = RustTyDecomposition::from_ty(arg_ty, self.g_params(vcx, call_ctx));
        let region_proj =
            projection_for_generalized_idx(arg_ty, g.region_idx(), decomp, vcx.tcx())?;
        let predicates = deps
            .require_dep::<IndirectPredicatesEnc>(region_proj)
            .unwrap()
            .predicate_applications;
        (!predicates.is_empty()).then_some(predicates)
    }

    fn encode_predicates_for_function_shape_node(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, impl TaskEncoder>,
        g: impl Into<FunctionShapeNode<Generalized>>,
        call_ctx: WandCallContext<'vir>,
        mut snap: impl FnMut(mir::Local) -> vir::ExprSnap<'vir>,
    ) -> Option<vir::ExprBool<'vir>> {
        use vir::Reify;
        let g = g.into();
        let predicates = self.predicates_for_function_shape_node(vcx, deps, g, call_ctx)?;

        let local = g.mir_local();
        let local_snap = snap(local);
        Some(
            vcx.mk_conj(
                &predicates
                    .iter()
                    .map(|p| p.reify(vcx, local_snap))
                    .collect::<Vec<_>>(),
            ),
        )
    }

    pub fn indirect_pres<'a, E: TaskEncoder>(
        &'a self,
        vcx: &'vir vir::VirCtxt<'vir>,
        local_defs: &'a MirLocalDefEncOutput<'vir>,
        deps: &'a mut TaskEncoderDependencies<'vir, E>,
    ) -> impl Iterator<Item = vir::ExprBool<'vir>> + 'a {
        self.inputs().filter_map(|g| {
            self.encode_predicates_for_function_shape_node(vcx, deps, g, None, |i| {
                local_defs[i].impure_snap
            })
        })
    }

    pub fn indirect_posts<'a, E: TaskEncoder>(
        &'a self,
        vcx: &'vir vir::VirCtxt<'vir>,
        local_defs: &'a MirLocalDefEncOutput<'vir>,
        deps: &'a mut TaskEncoderDependencies<'vir, E>,
    ) -> impl Iterator<Item = vir::ExprBool<'vir>> + 'a {
        // The encoded predicates for the input lifetime projections that are
        // not blocked by any of the result lifetime projections. These will be
        // encoded as part of the postcondition of the function (in contrast,
        // the predicates for the blocked inputs will appear on the right-hand
        // side of a magic wand in the postcondition).
        let unblocked_input_posts = self
            .inputs()
            .filter(|i| !self.blocked_inputs().contains(i))
            .filter_map(|lp| {
                self.encode_predicates_for_function_shape_node(vcx, deps, lp, None, |i| {
                    vcx.mk_old_expr(local_defs[i].impure_snap)
                })
            })
            .collect::<Vec<_>>()
            .into_iter();

        let output_posts = self.outputs().filter_map(|g| {
            self.encode_predicates_for_function_shape_node(vcx, deps, g, None, |i| {
                local_defs[i].impure_snap
            })
        });
        unblocked_input_posts.chain(output_posts)
    }

    pub fn wand_posts<'a, E: TaskEncoder>(
        &'a self,
        vcx: &'vir vir::VirCtxt<'vir>,
        local_defs: &'a MirLocalDefEncOutput<'vir>,
        deps: &'a mut TaskEncoderDependencies<'vir, E>,
    ) -> impl Iterator<Item = vir::ExprBool<'vir>> + 'a {
        let wand_result =
            vcx.mk_local_decl("wand_result", local_defs[mir::RETURN_PLACE].local_snap.ty());
        let wand_result_expr = vcx.mk_local_ex(wand_result);
        let args = local_defs
            .args()
            .map(|arg| vcx.mk_old_expr(arg.impure_snap));
        let args = PledgeExpr::pledge_args(wand_result_expr, args);

        // TODO: wands for late-bound regions
        self.viper_wands().into_iter().filter_map(move |wand_data| {
            let wand = self.mk_wand(&wand_data, args, None, None, vcx, deps)?;
            Some(vcx.mk_let_expr(
                wand_result,
                local_defs[mir::RETURN_PLACE].impure_snap,
                vcx.mk_wand_expr(wand),
            ))
        })
    }

    pub fn apply_wands<E: TaskEncoder>(
        &self,
        arguments: &[vir::ExprSnap<'vir>],
        label_pre: &'vir str,
        label_post: &'vir str,
        call_ctx: GArgs<'vir>,
        visitor: &mut ImpureEncVisitor<'vir, '_, E>,
    ) {
        let result = visitor
            .vcx
            .mk_local_labelled_old_expr(arguments[mir::RETURN_PLACE.as_usize()], label_post);
        let args = (1..arguments.len()).map(|l| {
            visitor
                .vcx
                .mk_local_labelled_old_expr(arguments[l], label_pre)
        });
        let args = PledgeExpr::pledge_args(result, args);
        for wand_data in self.viper_wands() {
            let Some(wand) = self.mk_wand(
                &wand_data,
                args,
                Some(label_pre),
                Some(call_ctx),
                visitor.vcx,
                visitor.deps,
            ) else {
                continue;
            };
            visitor.stmt(visitor.vcx.mk_apply_stmt(wand));
        }
    }

    fn mk_wand<E: TaskEncoder>(
        &self,
        wand_data: &WandData<'vir>,
        pledge_args: PledgeArgs<'vir>,
        pledge_old_label: Option<&'vir str>,
        call_ctx: WandCallContext<'vir>,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Option<vir::Wand<'vir>> {
        debug_assert!(!wand_data.lhs.is_empty());
        let generics = self.generics_subst(call_ctx, vcx, deps);
        // Specifications see deep values (see `MirSpecEnc`).
        let fn_sig = self.fn_sig(vcx, call_ctx);
        let g_params = self.g_params(vcx, call_ctx);
        let deep_args = pledge_args.map(|local, snap| {
            let ty = if local == mir::RETURN_PLACE {
                fn_sig.output()
            } else {
                fn_sig.inputs()[local.index() - 1]
            };
            let ty = RustTyDecomposition::from_ty(ty, g_params);
            deep_snap(deps, ty, vcx.mk_null(), snap).unwrap()
        });
        let pledge_expr = |pledge: &PledgeExpr<'vir>| match pledge_old_label {
            Some(label) => {
                vcx.with_local_subst(generics, || pledge.expr_at_label(deep_args, label))
            }
            None => pledge.expr(deep_args),
        };
        let mut rhs = wand_data
            .rhs
            .iter()
            .filter_map(|g| {
                self.encode_predicates_for_function_shape_node(vcx, deps, *g, call_ctx, |i| {
                    pledge_args[i]
                })
            })
            .collect::<Vec<_>>();
        // The interior-mutability state of the returned arguments: the atoms
        // they lent to the result, and the facts about the rest.
        rhs.extend(self.lent_footprints(wand_data, pledge_args, call_ctx, vcx, deps));
        let label = pledge_old_label.map_or(vir::OldLabel::None, vir::OldLabel::Label);
        let mut returned = Vec::new();
        for g in &wand_data.rhs {
            let local = g.mir_local();
            if returned.contains(&local) {
                continue;
            }
            returned.push(local);
            let arg_ty = FunctionShapeNode::from(*g).ty(self.fn_sig(vcx, call_ctx));
            let decomp = RustTyDecomposition::from_ty(arg_ty, self.g_params(vcx, call_ctx));
            rhs.extend(on_expiry(deps, decomp, pledge_args[local], label).unwrap());
        }
        let rhs = rhs
            .into_iter()
            .chain(
                wand_data
                    .pledges
                    .iter()
                    .map(|pledge| pledge_expr(&pledge.expiry_postcondition)),
            )
            .collect::<Vec<_>>();
        if rhs.is_empty() {
            // We skip emitting the wand when there is nothing on the RHS, i.e.,
            // nothing would be unblocked by applying this wand, nor are there
            // any pledge postconditions.
            return None;
        }
        let rhs = vcx.mk_conj(&rhs);
        // What the result was lent goes back with it.
        let lent = wand_data
            .lhs
            .iter()
            .find(|g| g.mir_local() == mir::RETURN_PLACE && g.region_idx().index() == 0)
            .and_then(|g| {
                let decomp = RustTyDecomposition::from_ty(g.ty(fn_sig), g_params);
                lent_footprint(deps, decomp, pledge_args[mir::RETURN_PLACE]).unwrap()
            });
        let lhs = wand_data.lhs.iter().filter_map(|g| {
            self.encode_predicates_for_function_shape_node(vcx, deps, *g, call_ctx, |i| {
                pledge_args[i]
            })
        });
        let lhs = lhs
            .chain(lent)
            .chain(
                wand_data
                    .pledges
                    .iter()
                    .filter_map(|pledge| pledge.expiry_obligation.as_ref().map(pledge_expr)),
            )
            .collect::<Vec<_>>();
        let lhs = vcx.mk_conj(&lhs);
        Some(vcx.mk_wand(lhs, rhs))
    }

    /// The interior-mutability atoms the returned arguments lent to the
    /// result (with the projection of their outermost reference, so once).
    pub fn lent_footprints<E: TaskEncoder>(
        &self,
        wand_data: &WandData<'vir>,
        pledge_args: PledgeArgs<'vir>,
        call_ctx: WandCallContext<'vir>,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Vec<vir::ExprBool<'vir>> {
        let fn_sig = self.fn_sig(vcx, call_ctx);
        let g_params = self.g_params(vcx, call_ctx);
        wand_data
            .rhs
            .iter()
            .filter(|g| g.region_idx().index() == 0)
            .filter_map(|g| {
                let arg_ty = FunctionShapeNode::from(*g).ty(fn_sig);
                let decomp = RustTyDecomposition::from_ty(arg_ty, g_params);
                lent_footprint(deps, decomp, pledge_args[g.mir_local()]).unwrap()
            })
            .collect()
    }

    /// The pledges are encoded once, at the callee's identity substitution,
    /// so they refer to the callee's generic parameters. At a call site, these
    /// are replaced by the call's generic arguments, as Viper does for the
    /// callee's postcondition (from which the caller holds the wand).
    fn generics_subst<E: TaskEncoder>(
        &self,
        call_ctx: WandCallContext<'vir>,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> &'vir FxHashMap<&'vir str, vir::ExprDyn<'vir>> {
        let Some(call_args) = call_ctx else {
            return vcx.alloc(FxHashMap::default());
        };
        let params = deps
            .require_dep::<GenericParamsEnc>(self.g_params(vcx, None))
            .unwrap();
        let args = deps.require_dep::<GArgsTyEnc>(call_args).unwrap();
        let tys = params
            .ty_decls()
            .iter()
            .zip(args.get_ty::<(), !>())
            .map(|(decl, arg)| (decl.name, arg.as_dyn()));
        let consts = params
            .const_decls()
            .iter()
            .zip(args.get_const::<(), !>())
            .map(|(decl, arg)| (decl.name, arg.as_dyn()));
        vcx.alloc(tys.chain(consts).collect())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WandEncTask<'tcx> {
    pub data: FunctionData<'tcx>,
}

impl<'tcx> WandEncTask<'tcx> {
    pub fn def_id(&self) -> DefId {
        self.data.def_id()
    }

    pub fn function_shape(
        &self,
        vcx: &vir::VirCtxt<'tcx>,
    ) -> Result<FunctionShape<Generalized>, MakeFunctionShapeError> {
        self.data.shape(vcx.tcx())
    }
}

pub type WandRhsKey = FunctionShapeInput<Generalized>;
pub type WandLhsKey = FunctionShapeNode<Generalized>;

#[derive(Clone, Debug)]
pub struct WandData<'vir> {
    /// Lifetime projections on the right-hand side of the wand. Guaranteed to be
    /// non-empty.
    rhs: Vec<WandRhsKey>,
    /// Lifetime projections on the left-hand side of the wand. Guaranteed to be
    /// non-empty.
    lhs: Vec<WandLhsKey>,
    pledges: EncodedPledges<'vir>,
}

impl<'vir> WandData<'vir> {
    pub fn new(lhs: Vec<WandLhsKey>, rhs: Vec<WandRhsKey>, pledges: EncodedPledges<'vir>) -> Self {
        debug_assert!(!lhs.is_empty());
        debug_assert!(!rhs.is_empty());
        Self { rhs, lhs, pledges }
    }
}

impl TaskEncoder for WandEnc {
    task_encoder::encoder_cache!(WandEnc);

    type TaskDescription<'vir> = WandEncTask<'vir>;

    type TaskKey<'vir> = WandEncTask<'vir>;

    type OutputFullDependency<'vir> = WandEncOutput<'vir>;

    type EncodingError = WandEncError;

    const ENCODER_NAME: &'static str = "wand encoder";

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        task.clone()
    }

    fn describe_error(error: Self::EncodingError) -> String {
        match error {
            WandEncError::Unsupported(message) => message,
        }
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(task_key.clone(), ())?;
        vir::with_vcx(|vcx| {
            let def_id = task_key.def_id();

            let shape = task_key.function_shape(vcx).map_err(|e| {
                EncodeFullError::EncodingError(
                    WandEncError::Unsupported(format!("function shape: {e:?}")),
                    None,
                )
            })?;

            let coupled_edges = shape.coupled_edges();
            let edges: FxHashSet<_> = shape.edges().map(|e| (e.input(), e.output())).collect();

            let (inputs, outputs) = shape.take_inputs_and_outputs();
            let spec = deps.require_dep::<MirSpecEnc>((def_id, def_id, MirSpecEncMode::Impure))?;
            if coupled_edges.is_empty() {
                assert!(spec.pledges.is_empty());
                return Ok((
                    (),
                    WandEncOutput {
                        function_data: task_key.data,
                        inputs,
                        outputs,
                        wands: vec![],
                    },
                ));
            }
            let pledges = spec.pledges;
            let wands: Vec<WandData<'vir>> = coupled_edges
                .into_iter()
                .filter_map(|hyper_edge| {
                    let (sources, mut targets) = hyper_edge.into_tuple();
                    // We don't want to emit an identity wand, like P --* P. This can happen when
                    // PCG returns self-edges, like for fn(x: &'a mut &'b i32) where 'b is in
                    // invariant position and we therefore have an edge x|'b -> x|'b.
                    // Currently, these edges also prevent us from emitting indirect postconditions.
                    // TODO: we might want to emit these identity wands in the future to attach functional
                    // specifications to them. We still need to emit the resources on the wand's LHS.
                    let mut sources_as_nodes = sources
                        .iter()
                        .map(|&s| s.to_function_shape_node())
                        .collect::<Vec<_>>();
                    sources_as_nodes.sort();
                    targets.sort();
                    if sources_as_nodes == targets {
                        return None;
                    }
                    Some(WandData::new(targets, sources, pledges.clone()))
                })
                .collect();
            let mut output: WandEncOutput<'vir> = WandEncOutput {
                function_data: task_key.data,
                inputs,
                outputs,
                wands: Vec::new(),
            };
            output.wands = output
                .select_wands(wands, !pledges.is_empty(), &edges, vcx, deps)
                .map_err(|err| EncodeFullError::EncodingError(err, None))?;
            Ok(((), output))
        })
    }
}

impl<'vir> WandEncOutput<'vir> {
    /// Selects the wands to emit among those of the coupled edges, rejecting
    /// the shapes that cannot be encoded precisely.
    fn select_wands<E: TaskEncoder>(
        &self,
        wands: Vec<WandData<'vir>>,
        has_pledges: bool,
        edges: &FxHashSet<(WandRhsKey, WandLhsKey)>,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<Vec<WandData<'vir>>, WandEncError> {
        let mut has_resources = |g: FunctionShapeNode<Generalized>| {
            self.predicates_for_function_shape_node(vcx, deps, g, None)
                .is_some()
        };
        // A wand is only needed if it gives back a resource, or to carry the
        // pledges if none does (e.g. for shared references).
        let (mut wands, resourceless): (Vec<_>, Vec<_>) = wands
            .into_iter()
            .partition(|wand_data| wand_data.rhs.iter().any(|g| has_resources((*g).into())));
        // The PCG lets a type parameter in the result capture any region of
        // the arguments, through projections without a counterpart in the
        // encoding. A wand blocked only by such projections is not needed if
        // no value of a type parameter can borrow from what it gives back:
        // the arguments are then given back directly.
        if !has_pledges {
            wands.retain(|wand_data| {
                wand_data
                    .lhs
                    .iter()
                    .any(|g| self.generalized_region(vcx, *g).is_some())
                    || wand_data
                        .rhs
                        .iter()
                        .filter(|g| has_resources((**g).into()))
                        .any(|g| match self.generalized_region(vcx, (*g).into()) {
                            Some(region) => self.type_param_may_capture(vcx, region),
                            None => true,
                        })
            });
        }
        // A wand gives back its sources only once all of its targets have
        // expired. That matches the signature only if each source with
        // resources flows into each target with resources.
        for wand_data in &wands {
            let sources = wand_data
                .rhs
                .iter()
                .filter(|g| has_resources((**g).into()))
                .collect::<Vec<_>>();
            let targets = wand_data
                .lhs
                .iter()
                .filter(|g| has_resources(**g))
                .collect::<Vec<_>>();
            let precise = sources.iter().all(|source| {
                let mut targets = targets.iter();
                targets.all(|target| edges.contains(&(**source, **target)))
            });
            if !precise {
                return Err(WandEncError::Unsupported(
                    "borrows in the result that expire separately but depend on a common \
                     argument are not supported"
                        .to_string(),
                ));
            }
        }
        if has_pledges {
            if wands.is_empty() {
                wands.extend(resourceless.into_iter().take(1));
            } else if wands.len() > 1 {
                // It is unclear which expiry the pledges refer to.
                return Err(WandEncError::Unsupported(
                    "pledges on a function whose result contains borrows that expire \
                     separately are not supported"
                        .to_string(),
                ));
            }
        }
        Ok(wands)
    }

    /// The region of the given node, or `None` if it has no counterpart in
    /// the encoding (e.g. the regions in a type parameter).
    fn generalized_region(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        g: FunctionShapeNode<Generalized>,
    ) -> Option<PcgRegion<'vir>> {
        let ty = g.ty(self.fn_sig(vcx, None));
        let lifetimes: IndexVec<_, GeneralizedLifetime<'vir>> = vcx.tcx().extract_regions(ty);
        match lifetimes.get(g.region_idx())? {
            GeneralizedLifetime::Region(region) => Some(*region),
            GeneralizedLifetime::RegionsIn(_) => None,
        }
    }

    /// Whether a value of a type parameter can be made to borrow from
    /// `region`. That needs an argument or a bound relating the two, which is
    /// approximated by `region` occurring in a bound other than an outlives
    /// bound, or in an argument other than in its outer references.
    fn type_param_may_capture(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        region: PcgRegion<'vir>,
    ) -> bool {
        let in_args = self.fn_sig(vcx, None).inputs().iter().any(|&ty| {
            let mut ty = ty;
            while let ty::TyKind::Ref(_, referent, _) = ty.kind() {
                ty = *referent;
            }
            vcx.tcx()
                .any_free_region_meets(&ty, |r| PcgRegion::from(r) == region)
        });
        let param_env = self.function_data.param_env(vcx.tcx());
        let in_bounds = param_env.caller_bounds().iter().any(|clause| {
            clause.as_type_outlives_clause().is_none()
                && clause.as_region_outlives_clause().is_none()
                && vcx
                    .tcx()
                    .any_free_region_meets(&clause, |r| PcgRegion::from(r) == region)
        });
        in_args || in_bounds
    }

    pub fn viper_wands(&self) -> Vec<WandData<'vir>> {
        self.wands.clone()
    }

    /// All lifetime projections in the arguments that are blocked by any of the
    /// lifetime projections in the function's result.
    pub fn blocked_inputs(&self) -> FxHashSet<FunctionShapeInput<Generalized>> {
        self.wands
            .iter()
            .flat_map(|wand| wand.rhs.iter().copied())
            .collect()
    }

    pub fn inputs(&self) -> impl Iterator<Item = FunctionShapeInput<Generalized>> + '_ {
        self.inputs.iter().copied()
    }

    pub fn outputs(&self) -> impl Iterator<Item = FunctionShapeOutput<Generalized>> + '_ {
        self.outputs.iter().copied()
    }
}
