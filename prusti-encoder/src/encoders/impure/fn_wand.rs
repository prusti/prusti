use crate::encoders::{
    EncodeResult, ImpureEncVisitor, MirLocalDefEncOutput, MirSpecEnc, Pure,
    mir_fn::{RustSignature, report_dep_error},
    pure::spec::{EncodedPledge, MirSpecEncMode, PledgeArgs, PledgeExpr},
    ty::{
        RustTyDecomposition,
        generics::{
            GArgs, GArgsCastEnc, GArgsTyEnc, GParams, GenericParamsEnc,
            lifetime_position::{
                GeneralizedLifetime as TyLifetime, LifetimePosition, LifetimePositionEnc,
            },
            merge_split::{MergeSplitEnc, MergeTarget},
        },
        indirect_nodes::NodePredicates,
    },
};
use pcg::{
    borrow_pcg::{
        FunctionData, FunctionShape, FunctionShapeInput, FunctionShapeNode, FunctionShapeOutput,
        GeneralizedLifetime, MakeFunctionShapeError,
        borrow_pcg_edge::BlockedNode,
        region_projection::{Generalized, LifetimeProjection},
        state::BorrowsState,
    },
    utils::place::remote::RemotePlace,
};
use prusti_interface::PrustiError;
use prusti_rustc_interface::{
    data_structures::fx::{FxHashMap, FxHashSet},
    index::{Idx, IndexVec},
    middle::{mir, ty},
    span::def_id::DefId,
};
use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
use vir::{CastType, HasType, Reify};

/// Encodes the magic wands given a function signature.
pub struct WandEnc;

impl WandEnc {
    /// Reports the failure `err` to generate the wands of the function
    /// `def_id` at the function, and returns the proof that it was reported.
    pub(crate) fn report_failure<'vir, E: TaskEncoder + ?Sized>(
        vcx: &'vir vir::VirCtxt<'vir>,
        def_id: DefId,
        err: &EncodeFullError<'vir, E>,
    ) -> vir::Reported {
        let name = vcx.tcx().def_path_str(def_id);
        report_dep_error(vcx, err, |err| {
            err.report(format!("method `{name}`"), vcx.tcx().def_span(def_id))
        })
    }
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
            let Some(wand) = self.wands.mk_wand(
                &wand_data,
                args,
                None,
                LhsKeys::AtLabel(vir::OldLabel::Label(label)),
                None,
                self.vcx,
                self.deps,
            ) else {
                continue;
            };
            let rhs = wand_data
                .rhs
                .iter()
                .map(|g| {
                    let resources = self.wands.encode_predicates_for_function_shape_node(
                        self.vcx,
                        self.deps,
                        *g,
                        None,
                        |i| args[i],
                        Some(vir::OldLabel::None),
                    );
                    // The caller's view of the argument's lifetime projection,
                    // which the PCG blocks by the argument's own projection
                    // (`InitialBorrows`): unblocking it gives the argument's
                    // predicates back along the edges into the result.
                    let node: BlockedNode<'vir> =
                        LifetimeProjection::<'vir, RemotePlace>::from_index(
                            RemotePlace::new(g.mir_local()),
                            g.region_idx(),
                        )
                        .into();
                    (node, resources.into_iter().collect())
                })
                .collect();
            let mut package_script = self.pcs_package_script(final_borrow_state, rhs, label)?;

            for EncodedPledge {
                expiry_postcondition,
                ..
            } in &wand_data.pledges
            {
                let span = expiry_postcondition.span();
                self.vcx.with_span(span, |vcx| {
                    vcx.handle_error("exhale.failed:assertion.false", move |_| {
                        Some(vec![PrustiError::verification(
                            "pledge in the postcondition might not hold.",
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

/// How the keys of a wand's left side are fixed. A wand must be
/// self-framing, but a key can read the predicates of other lifetimes, which
/// are not in its left side: it is read under a label, or bound outside the
/// wand.
enum LhsKeys<'vir, 'a> {
    /// The keys as they are, for a wand that is only displayed.
    #[cfg(test)]
    InPlace,
    AtLabel(vir::OldLabel<'vir>),
    Bound(&'a mut Vec<(vir::LocalDecl<'vir, vir::Snap>, vir::ExprSnap<'vir>)>),
}

/// Not tied to a caller or callee context. `indirect_pres`, `indirect_posts`,
/// `wand_posts`, and `package_wands` are identity-substituted and intended for
/// use in the callee's own contract; `call_wands` is for caller use and
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

/// Whether a call's predicates are turned into the callee's before the call,
/// or back into the caller's after it.
#[derive(Clone, Copy)]
enum CallSide {
    Before,
    After,
}

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
            ),
        }
    }

    /// The node predicates of the type of `g`'s local and the PCG lifetime of
    /// `g`, in the callee's own context, or `None` if `g` has no positions:
    /// e.g. the lifetime of a shared reference, or a lifetime in the trait
    /// bounds of a type parameter, which is part of `RegionsIn` of the
    /// parameter. The PCG's lifetimes are matched by value, which needs the
    /// distinct regions of the callee's signature; at a call site, the
    /// regions of the generic arguments are erased.
    fn node<E: TaskEncoder>(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
        g: FunctionShapeNode<Generalized>,
    ) -> Option<(NodePredicates<'vir>, GeneralizedLifetime<'vir>)> {
        let ty = g.ty(self.fn_sig(vcx, None));
        let lifetime = self
            .function_data
            .shape_node_lifetime(g, vcx.tcx())
            .ok()??;
        let decomp = RustTyDecomposition::from_ty(ty, self.g_params(vcx, None));
        let nodes = NodePredicates::new(decomp, deps).unwrap();
        nodes.has_positions(lifetime).then_some((nodes, lifetime))
    }

    /// The predicates of `g` on the snapshot `snap` of its local, with keys
    /// that extend the snapshot in the state `state`. An input node refers to
    /// the state before the call, so the extension of its key must read that
    /// state's heap, not only the local's snapshot.
    fn encode_predicates_for_function_shape_node<E: TaskEncoder>(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
        g: impl Into<FunctionShapeNode<Generalized>>,
        call_ctx: WandCallContext<'vir>,
        snap: impl FnMut(mir::Local) -> vir::ExprSnap<'vir>,
        state: Option<vir::OldLabel<'vir>>,
    ) -> Option<vir::ExprBool<'vir>> {
        self.encode_predicates_with_keys(vcx, deps, g, call_ctx, snap, |key| {
            vcx.maybe_apply_label(key, state)
        })
    }

    /// Like `encode_predicates_for_function_shape_node`, with each key given
    /// by `key` applied to its extension of the snapshot.
    fn encode_predicates_with_keys<E: TaskEncoder>(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
        g: impl Into<FunctionShapeNode<Generalized>>,
        call_ctx: WandCallContext<'vir>,
        mut snap: impl FnMut(mir::Local) -> vir::ExprSnap<'vir>,
        key: impl FnMut(vir::ExprSnap<'vir>) -> vir::ExprSnap<'vir>,
    ) -> Option<vir::ExprBool<'vir>> {
        let g = g.into();
        let (nodes, lifetime) = self.node(vcx, deps, g)?;
        // The predicates are in terms of the callee's generic parameters, the
        // snapshot in the caller's.
        let (placeholder, subst) = self.placeholder_subst(call_ctx, snap(g.mir_local()), vcx, deps);
        let predicates = vcx.mk_conj(&nodes.of(lifetime, placeholder, key));
        Some(vcx.with_local_subst(subst, || {
            predicates.lazy::<(), vir::ExprKind<'vir>>().reify(vcx, ())
        }))
    }

    /// The statements that give a call with generic arguments `call_ctx` the
    /// predicates its precondition requires of the arguments, whose snapshots
    /// `arguments` are in order, where they differ from the caller's.
    pub fn before_call<E: TaskEncoder>(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
        arguments: &[vir::ExprSnap<'vir>],
        call_ctx: GArgs<'vir>,
    ) -> Result<Vec<vir::Stmt<'vir>>, EncodeFullError<'vir, E>> {
        let mut stmts = Vec::new();
        for g in self.inputs() {
            // The callee's argument locals start after the return place.
            let arg = arguments[g.mir_local().as_usize() - 1];
            stmts.extend(self.call_adjustments(
                vcx,
                deps,
                g.into(),
                arg,
                call_ctx,
                CallSide::Before,
            )?);
        }
        Ok(stmts)
    }

    /// The statements that turn the predicates a call's postcondition gives
    /// back into the caller's: those of the inputs not blocked by the result,
    /// on the arguments' snapshots before the call `arguments`, and those of
    /// the result, with snapshot `result`. The predicates of nested lifetimes
    /// in the arguments that the postcondition gives back are not handled yet.
    pub fn after_call<E: TaskEncoder>(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
        arguments: &[vir::ExprSnap<'vir>],
        result: vir::ExprSnap<'vir>,
        call_ctx: GArgs<'vir>,
    ) -> Result<Vec<vir::Stmt<'vir>>, EncodeFullError<'vir, E>> {
        let blocked = self.blocked_inputs();
        let inputs = self
            .inputs()
            .filter(|g| !blocked.contains(g))
            .map(|g| (g.into(), arguments[g.mir_local().as_usize() - 1]));
        let outputs = self
            .outputs()
            .filter(|g| g.mir_local() == mir::RETURN_PLACE)
            .map(|g| (g, result));
        let mut stmts = Vec::new();
        for (g, snap) in inputs.chain(outputs).collect::<Vec<_>>() {
            stmts.extend(self.call_adjustments(vcx, deps, g, snap, call_ctx, CallSide::After)?);
        }
        Ok(stmts)
    }

    /// The statements that turn the caller's predicates of the node `g` on a
    /// value with snapshot `snap` into the callee's before a call with generic
    /// arguments `call_ctx`, or the callee's back into the caller's after it.
    /// A callee position of the lifetimes in a type parameter covers the block
    /// of the caller's positions that the parameter's instantiation occupies,
    /// which may be empty or have several positions: it is merged into the
    /// callee's predicate before the call, and split after it. For a value of
    /// a type parameter's type, the callee's predicate is the parameter's.
    fn call_adjustments<E: TaskEncoder>(
        &self,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
        g: FunctionShapeNode<Generalized>,
        snap: vir::ExprSnap<'vir>,
        call_ctx: GArgs<'vir>,
        side: CallSide,
    ) -> Result<Vec<vir::Stmt<'vir>>, EncodeFullError<'vir, E>> {
        let Some((nodes, lifetime)) = self.node(vcx, deps, g) else {
            return Ok(Vec::new());
        };
        if !matches!(lifetime, GeneralizedLifetime::RegionsIn(_)) {
            return Ok(Vec::new());
        }
        let callee_ty = g.ty(self.fn_sig(vcx, None));
        let target = if matches!(callee_ty.kind(), ty::TyKind::Param(_)) {
            MergeTarget::Param
        } else {
            MergeTarget::Own
        };
        let callee_decomp = RustTyDecomposition::from_ty(callee_ty, self.g_params(vcx, None));
        let graph = deps.require_dep::<LifetimePositionEnc>(callee_decomp)?;
        let caller_ty = g.ty(self.fn_sig(vcx, Some(call_ctx)));
        let caller_decomp = RustTyDecomposition::from_ty(caller_ty, call_ctx.context());
        let mut blocks: IndexVec<LifetimePosition, (usize, usize)> = IndexVec::new();
        let mut end = 0;
        for p in graph.positions() {
            let width = match graph.lifetime_in(p, caller_ty, call_ctx.context()) {
                TyLifetime::Region(_) => 1,
                TyLifetime::RegionsIn(ty) => deps
                    .require_dep::<LifetimePositionEnc>(ty)?
                    .positions()
                    .count(),
            };
            blocks.push((end, end + width));
            end += width;
        }
        let caller_graph = deps.require_dep::<LifetimePositionEnc>(caller_decomp)?;
        assert_eq!(
            end,
            caller_graph.positions().count(),
            "the callee's positions do not cover the caller's"
        );
        // The merge/split methods are shared by the instantiations with the
        // same canonical form and take its type parameters.
        let (_, params) = caller_decomp
            .canonical_with_params()
            .expect("the merge/split encoder rejects const parameters");
        let params = vcx
            .tcx()
            .mk_args_from_iter(params.into_iter().map(ty::GenericArg::from));
        let args = deps.require_dep::<GArgsTyEnc>(GArgs::new(call_ctx.context(), params))?;
        let (placeholder, subst) = self.placeholder_subst(Some(call_ctx), snap, vcx, deps);
        let mut stmts = Vec::new();
        for p in nodes.positions_of(lifetime) {
            let (first, end) = blocks[p];
            let block = (
                caller_decomp,
                LifetimePosition::new(first),
                LifetimePosition::new(end),
                target,
            );
            let merge_split = deps.require_ref::<MergeSplitEnc>(block)?;
            // A type parameter's only position is keyed by the value's
            // snapshot, which the methods cast to the generic one themselves.
            let key = match target {
                MergeTarget::Param => snap,
                MergeTarget::Own => {
                    let key = nodes.key(p, placeholder);
                    vcx.with_local_subst(subst, || {
                        key.lazy::<(), vir::ExprKind<'vir>>().reify(vcx, ())
                    })
                }
            };
            let method = match side {
                CallSide::Before => merge_split.merge,
                CallSide::After => merge_split.split,
            };
            stmts.push(method(key, args.get_ty(), args.get_const()).alloc());
        }
        Ok(stmts)
    }

    pub fn indirect_pres<'a, E: TaskEncoder>(
        &'a self,
        vcx: &'vir vir::VirCtxt<'vir>,
        local_defs: &'a MirLocalDefEncOutput<'vir>,
        deps: &'a mut TaskEncoderDependencies<'vir, E>,
    ) -> impl Iterator<Item = vir::ExprBool<'vir>> + 'a {
        self.inputs().filter_map(|g| {
            self.encode_predicates_for_function_shape_node(
                vcx,
                deps,
                g,
                None,
                |i| local_defs[i].impure_snap,
                None,
            )
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
                // Unblocked inputs are given back in the post-state.
                self.encode_predicates_for_function_shape_node(
                    vcx,
                    deps,
                    lp,
                    None,
                    |i| vcx.mk_old_expr(local_defs[i].impure_snap),
                    None,
                )
            })
            .collect::<Vec<_>>()
            .into_iter();

        let output_posts = self.outputs().filter_map(|g| {
            self.encode_predicates_for_function_shape_node(
                vcx,
                deps,
                g,
                None,
                |i| local_defs[i].impure_snap,
                None,
            )
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
            // The keys of the left side are bound outside the wand, which
            // must be self-framing: they read the result's predicates of the
            // other lifetimes, which are not in its left side.
            let mut bindings = Vec::new();
            let wand = self.mk_wand(
                &wand_data,
                args,
                None,
                LhsKeys::Bound(&mut bindings),
                None,
                vcx,
                deps,
            )?;
            let wand = bindings
                .into_iter()
                .rev()
                .fold(vcx.mk_wand_expr(wand), |expr, (decl, key)| {
                    vcx.mk_let_expr(decl, key, expr)
                });
            Some(vcx.mk_let_expr(wand_result, local_defs[mir::RETURN_PLACE].impure_snap, wand))
        })
    }

    pub fn call_wands<E: TaskEncoder>(
        &self,
        arguments: &[vir::ExprSnap<'vir>],
        label_pre: &'vir str,
        label_post: &'vir str,
        call_ctx: GArgs<'vir>,
        visitor: &mut ImpureEncVisitor<'vir, '_, E>,
    ) -> Result<Vec<vir::Wand<'vir>>, EncodeFullError<'vir, E>> {
        // The wands are in the callee's terms.
        let arguments = arguments
            .iter()
            .enumerate()
            .map(|(local, &snap)| self.snap_in_callee(visitor.deps, local.into(), snap, call_ctx))
            .collect::<Result<Vec<_>, _>>()?;
        let result = visitor
            .vcx
            .mk_local_labelled_old_expr(arguments[mir::RETURN_PLACE.as_usize()], label_post);
        let args = (1..arguments.len()).map(|l| {
            visitor
                .vcx
                .mk_local_labelled_old_expr(arguments[l], label_pre)
        });
        let args = PledgeExpr::pledge_args(result, args);
        let mut wands = Vec::new();
        for wand_data in self.viper_wands() {
            let Some(wand) = self.mk_wand(
                &wand_data,
                args,
                Some(label_pre),
                LhsKeys::AtLabel(vir::OldLabel::Label(label_post)),
                Some(call_ctx),
                visitor.vcx,
                visitor.deps,
            ) else {
                continue;
            };
            wands.push(wand);
        }
        Ok(wands)
    }

    /// `snap`, the caller's snapshot of the callee's local `local` at a call
    /// with generic arguments `call_ctx`, in the callee's terms: the value of a
    /// type parameter of the callee is a generic snapshot, cast as for the
    /// call's arguments.
    fn snap_in_callee<E: TaskEncoder>(
        &self,
        deps: &mut TaskEncoderDependencies<'vir, E>,
        local: mir::Local,
        snap: vir::ExprSnap<'vir>,
        call_ctx: GArgs<'vir>,
    ) -> Result<vir::ExprSnap<'vir>, EncodeFullError<'vir, E>> {
        let signature = RustSignature::new(self.function_data.def_id());
        let ty = match local.as_usize() {
            0 => signature.output,
            arg => signature.inputs[arg - 1],
        };
        let normalized = ty.decompose_compare_normalize(signature.gparams, call_ctx);
        let caster = deps.require_dep::<GArgsCastEnc<Pure>>(normalized)?;
        Ok(caster.cast_to_callee_ctx(snap))
    }

    #[allow(clippy::too_many_arguments)]
    fn mk_wand<E: TaskEncoder>(
        &self,
        wand_data: &WandData<'vir>,
        pledge_args: PledgeArgs<'vir>,
        pledge_old_label: Option<&'vir str>,
        mut lhs_keys: LhsKeys<'vir, '_>,
        call_ctx: WandCallContext<'vir>,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Option<vir::Wand<'vir>> {
        debug_assert!(!wand_data.lhs.is_empty());
        let generics = self.generics_subst(call_ctx, vcx, deps);
        let pledge_expr = |pledge: &PledgeExpr<'vir>| match pledge_old_label {
            Some(label) => {
                vcx.with_local_subst(generics, || pledge.expr_at_label(pledge_args, label))
            }
            None => pledge.expr(pledge_args),
        };
        // The right side gives back inputs, as they were before the call.
        let pre = Some(pledge_old_label.map_or(vir::OldLabel::None, vir::OldLabel::Label));
        let rhs = wand_data.rhs.iter().filter_map(|g| {
            self.encode_predicates_for_function_shape_node(
                vcx,
                deps,
                *g,
                call_ctx,
                |i| pledge_args[i],
                pre,
            )
        });
        let rhs = rhs
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
        let mut lhs = Vec::new();
        for g in &wand_data.lhs {
            let key = |key: vir::ExprSnap<'vir>| match &mut lhs_keys {
                #[cfg(test)]
                LhsKeys::InPlace => key,
                LhsKeys::AtLabel(label) => vcx.maybe_apply_label(key, Some(*label)),
                LhsKeys::Bound(bindings) => {
                    let name = vir::vir_format!(vcx, "wand_key_{}", bindings.len());
                    let decl = vcx.mk_local_decl(name, key.ty());
                    bindings.push((decl, key));
                    vcx.mk_local_ex(decl)
                }
            };
            lhs.extend(self.encode_predicates_with_keys(
                vcx,
                deps,
                *g,
                call_ctx,
                |i| pledge_args[i],
                key,
            ));
        }
        let lhs = lhs
            .into_iter()
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

    /// A placeholder for `snap`, a snapshot in the caller's terms, and the
    /// substitution of `generics_subst` that also replaces the placeholder by
    /// `snap`. An expression built on the placeholder in the callee's terms
    /// and reified under the substitution has `snap` in it, which the
    /// substitution does not apply to: its type parameters are the caller's,
    /// which may have the names of the callee's. Without a call context,
    /// nothing is substituted, and `snap` is its own placeholder.
    fn placeholder_subst<E: TaskEncoder>(
        &self,
        call_ctx: WandCallContext<'vir>,
        snap: vir::ExprSnap<'vir>,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> (
        vir::ExprSnap<'vir>,
        &'vir FxHashMap<&'vir str, vir::ExprDyn<'vir>>,
    ) {
        if call_ctx.is_none() {
            return (snap, self.generics_subst(call_ctx, vcx, deps));
        }
        let placeholder = vcx.mk_local_decl("call_arg", snap.ty());
        let mut subst = self.generics_subst(call_ctx, vcx, deps).clone();
        subst.insert(placeholder.name, snap.as_dyn());
        (vcx.mk_local_ex(placeholder), vcx.alloc(subst))
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

    const ENCODER_NAME: &'static str = "wand encoder";

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        task.clone()
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(task_key.clone(), ())?;
        vir::with_vcx(|vcx| {
            let def_id = task_key.def_id();

            let shape = task_key.function_shape(vcx).map_err(|e| {
                EncodeFullError::unsupported(format!("function shape: {e:?}"), Vec::new())
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
                    // The PCG gives an argument's lifetime projection in an
                    // invariant position an edge to the argument's own
                    // post-state (e.g. `x|'b -> x|'b` for `x: &'a mut &'b i32`,
                    // or `x|RegionsIn(T) -> x|RegionsIn(T)` for `x: T`). The
                    // encoding gives the post-state the predicate of the
                    // pre-state, so such a target is the source the wand gives
                    // back: requiring it on the left would make the wand
                    // unappliable (the argument's predicate was given to the
                    // callee, e.g. moved into the result). These targets are
                    // left out, and a wand without other targets is not emitted.
                    // TODO: we might want to emit these identity wands in the future to attach functional
                    // specifications to them.
                    let sources_as_nodes = sources
                        .iter()
                        .map(|&s| s.to_function_shape_node())
                        .collect::<Vec<_>>();
                    targets.retain(|target| !sources_as_nodes.contains(target));
                    if targets.is_empty() {
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
                .map_err(|message| EncodeFullError::unsupported(message, Vec::new()))?;
            Ok(((), output))
        })
    }
}

impl<'vir> WandEncOutput<'vir> {
    /// Selects the wands to emit among those of the coupled edges, rejecting
    /// the shapes that cannot be encoded precisely. Pledges that cannot be
    /// attached to one wand are reported and left out.
    fn select_wands<E: TaskEncoder>(
        &self,
        wands: Vec<WandData<'vir>>,
        has_pledges: bool,
        edges: &FxHashSet<(WandRhsKey, WandLhsKey)>,
        vcx: &'vir vir::VirCtxt<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<Vec<WandData<'vir>>, String> {
        let mut has_resources =
            |g: FunctionShapeNode<Generalized>| self.node(vcx, deps, g).is_some();
        // A wand is only needed if it gives back a resource, or to carry the
        // pledges if none does (e.g. for shared references).
        let (mut wands, resourceless): (Vec<_>, Vec<_>) = wands
            .into_iter()
            .partition(|wand_data| wand_data.rhs.iter().any(|g| has_resources((*g).into())));
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
                return Err(
                    "borrows in the result that expire separately but depend on a common \
                     argument are not supported"
                        .to_string(),
                );
            }
        }
        if has_pledges {
            if wands.is_empty() {
                wands.extend(resourceless.into_iter().take(1));
            } else if wands.len() > 1 {
                // It is unclear which expiry the pledges refer to. They are
                // left out, each with its expiry obligation: then neither the
                // function nor its callers rely on any of them, so both are
                // still encoded. The encoding goes on, so the proof that the
                // error was reported is not needed.
                let span = wands[0].pledges[0].expiry_postcondition.span();
                let _ = vcx.emit_early_error(
                    PrustiError::unsupported(
                        "pledges on a function whose result contains borrows that expire \
                         separately are not supported",
                        span.into(),
                    )
                    .add_note("the pledges of the function are skipped", None),
                );
                for wand in &mut wands {
                    wand.pledges.clear();
                }
            }
        }
        Ok(wands)
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

/// Tests that show the wands `WandEnc` builds for a function's signature.
#[cfg(test)]
mod tests {
    use pcg::borrow_pcg::FunctionData;
    use prusti_rustc_interface::{
        hir::def::DefKind,
        span::{DUMMY_SP, def_id::DefId},
    };
    use task_encoder::{EncodeFullError, EncodeFullResult, TaskEncoder, TaskEncoderDependencies};
    use vir::CastType;

    use super::{LhsKeys, WandEnc, WandEncTask};
    use crate::encoders::{
        MirLocalDefEnc, MirLocalDefEncTask, TyUseImpureEnc,
        pure::spec::PledgeExpr,
        ty::{
            RustTyDecomposition,
            test_util::{dedent, simplified, with_crate},
        },
    };

    /// The wands and the contract of a function, as shown by `assert_wands`
    /// and `assert_contract`.
    #[derive(Clone)]
    struct Shown {
        wands: Vec<String>,
        contract: Vec<String>,
    }

    impl Shown {
        fn error(message: &str) -> Self {
            let lines = vec![format!("error: {message}")];
            Self {
                wands: lines.clone(),
                contract: lines,
            }
        }
    }

    /// The wands and the contract of a function, or the error of `WandEnc`.
    /// As in `wand_posts`, the wands are on the result's snapshot and the old
    /// snapshots of the arguments, each named after its parameter. The
    /// contract consists of the predicates that `method.rs` adds to the
    /// method's pre- and postconditions with `indirect_pres` and
    /// `indirect_posts`, on the method's own locals. Building them requires
    /// the dependencies of an encoder.
    struct WandTestEnc;

    impl TaskEncoder for WandTestEnc {
        task_encoder::encoder_cache!(WandTestEnc);
        const ENCODER_NAME: &'static str = "wand test encoder";

        type TaskDescription<'vir> = DefId;
        type OutputFullDependency<'vir> = Shown;

        fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
            *task
        }

        fn do_encode_full<'vir>(
            task_key: &Self::TaskKey<'vir>,
            deps: &mut TaskEncoderDependencies<'vir, Self>,
        ) -> EncodeFullResult<'vir, Self> {
            let def_id = *task_key;
            deps.emit_output_ref(def_id, ())?;
            let task = WandEncTask {
                data: FunctionData::new(def_id),
            };
            let wands = match deps.require_dep::<WandEnc>(task) {
                Ok(wands) => wands,
                // The error of `WandEnc` itself comes last, after its context.
                Err(EncodeFullError::DependencyError(errors)) => {
                    let message = &errors.last().unwrap().message;
                    return Ok(((), Shown::error(message)));
                }
                Err(error) => return Err(error),
            };
            vir::with_vcx(|vcx| {
                let sig = wands.fn_sig(vcx, None);
                let params = vcx.tcx().fn_arg_idents(def_id).iter();
                let names = std::iter::once("result".to_string())
                    .chain(params.map(|ident| ident.unwrap().to_string()));
                let tys = std::iter::once(sig.output()).chain(sig.inputs().iter().copied());
                let mut snaps = Vec::new();
                for (ty, name) in tys.zip(names) {
                    let ty = RustTyDecomposition::from_ty(ty, def_id);
                    let snapshot = deps.require_dep::<TyUseImpureEnc>(ty)?.snapshot();
                    let decl = vcx.mk_local_decl(vcx.alloc_str(&name), snapshot);
                    snaps.push(vcx.mk_local_ex(decl));
                }
                let args = snaps[1..].iter().map(|&snap| vcx.mk_old_expr(snap));
                let args = PledgeExpr::pledge_args(snaps[0], args);
                let mut lines = Vec::new();
                for wand_data in wands.viper_wands() {
                    if let Some(wand) =
                        wands.mk_wand(&wand_data, args, None, LhsKeys::InPlace, None, vcx, deps)
                    {
                        let mut wand_lines = show(wand.lhs);
                        wand_lines.push("--*".to_string());
                        wand_lines.extend(show(wand.rhs));
                        lines.push(wand_lines.join("\n"));
                    }
                }
                let local_defs = deps.require_dep::<MirLocalDefEnc>(MirLocalDefEncTask::Local {
                    def_id,
                    all_locals: false,
                })?;
                let pres = wands
                    .indirect_pres(vcx, &local_defs, deps)
                    .collect::<Vec<_>>();
                let posts = wands
                    .indirect_posts(vcx, &local_defs, deps)
                    .collect::<Vec<_>>();
                let pres = pres
                    .into_iter()
                    .flat_map(show)
                    .map(|c| format!("requires {c}"));
                let posts = posts
                    .into_iter()
                    .flat_map(show)
                    .map(|c| format!("ensures {c}"));
                let contract = pres.chain(posts).collect();
                Ok((
                    (),
                    Shown {
                        wands: lines,
                        contract,
                    },
                ))
            })
        }
    }

    /// Replaces `inhabited(..)` by `true`, which the simplifier then drops:
    /// it is not about borrows.
    struct DropInhabited;

    impl<'vir> vir::Folder<'vir, (), !> for DropInhabited {
        fn fold_expr(
            &mut self,
            e: vir::ExprGenDyn<'vir, (), !>,
        ) -> Option<vir::ExprGenDyn<'vir, (), !>> {
            match e.kind {
                vir::ExprKindGenData::FuncApp(app) if app.target == "inhabited" => {
                    Some(vir::with_vcx(|vcx| vcx.mk_bool::<true>().as_dyn()))
                }
                _ => e.super_fold_with(self),
            }
        }
    }

    /// The conjuncts of the simplified `e`, one per line.
    fn show(e: vir::ExprBool<'_>) -> Vec<String> {
        use vir::Foldable;
        let e = simplified(e.fold_with(&mut DropInhabited).unwrap_or(e));
        let mut conjuncts = Vec::new();
        collect_conjuncts(e.as_dyn(), &mut conjuncts);
        conjuncts.iter().map(|c| format!("{c:?}")).collect()
    }

    fn collect_conjuncts<'vir>(e: vir::ExprDyn<'vir>, conjuncts: &mut Vec<vir::ExprDyn<'vir>>) {
        match e.kind {
            vir::ExprKindGenData::BinOp(b) if b.kind == vir::BinOpKind::And => {
                collect_conjuncts(b.lhs, conjuncts);
                collect_conjuncts(b.rhs, conjuncts);
            }
            _ => conjuncts.push(e),
        }
    }

    /// What `WandTestEnc` shows for the function `f` defined in `src`.
    fn shown(src: &str) -> Shown {
        with_crate(src, |tcx| {
            let f = tcx
                .hir_crate_items(())
                .definitions()
                .find(|&def_id| {
                    tcx.def_kind(def_id) == DefKind::Fn && tcx.item_name(def_id).as_str() == "f"
                })
                .expect("the input defines no function `f`")
                .to_def_id();
            let (_, _, shown) = WandTestEnc::encode(f, true, DUMMY_SP).unwrap().unwrap();
            shown
        })
    }

    /// Asserts that the wands of the function `f` defined in `src` are
    /// `expected`: each wand lists the conjuncts of its left side, `--*`, and
    /// the conjuncts of its right side on lines of their own, and wands are
    /// separated by an empty line.
    fn assert_wands(src: &str, expected: &str) {
        assert_eq!(shown(src).wands.join("\n\n"), dedent(expected));
    }

    /// Asserts that the contract of the function `f` defined in `src` is
    /// `expected`, one conjunct of a pre- or postcondition per line.
    fn assert_contract(src: &str, expected: &str) {
        assert_eq!(shown(src).contract.join("\n"), dedent(expected));
    }

    #[test]
    fn reborrow() {
        assert_wands(
            "fn f<'a>(x: &'a mut i32) -> &'a mut i32 { x }",
            r#"
            p_Ref_mutable_ind(result, s_Int_i32_type(), 0, 1)
            --*
            p_Ref_mutable_ind(old(x), s_Int_i32_type(), 0, 1)
            "#,
        );
    }

    #[test]
    fn nested_references_reborrow() {
        assert_wands(
            "fn f<'a, 'b>(x: &'a mut &'b mut i32) -> &'a mut i32 { *x }",
            r#"
            error: borrows in the result that expire separately but depend on a common argument are not supported
            "#,
        );
    }

    #[test]
    fn lifetime_in_struct() {
        assert_wands(
            "struct S<'a> { r: &'a mut i32 }
             fn f<'a, 'b>(s: &'b mut S<'a>) -> &'b mut i32 { s.r }",
            r#"
            error: borrows in the result that expire separately but depend on a common argument are not supported
            "#,
        );
    }

    /// The lifetimes in `T` are nested in `*x`, like `'b` in
    /// `&'a mut S<'b>`, and expire separately from `'a`.
    #[test]
    fn generic_referent() {
        assert_wands(
            "fn f<'a, T>(x: &'a mut T) -> &'a mut T { x }",
            r#"
            error: borrows in the result that expire separately but depend on a common argument are not supported
            "#,
        );
    }

    #[test]
    fn reborrow_into_adt() {
        assert_wands(
            "fn f<'a, T>(x: &'a mut Option<T>) -> Option<&'a mut T> { x.as_mut() }",
            r#"
            p_Option_ind(result, s_Ref_mutable_type(T$0), 0, 1)
            --*
            p_Ref_mutable_ind(old(x), s_Option_type(T$0), 0, 1)

            p_Ref_mutable_ind(extend_Ref_mutable(old(x), s_Option_type(T$0), 0, 1), s_Option_type(T$0), 1, (1) + (arity(T$0)))
            p_Option_ind(extend_Option(result, s_Ref_mutable_type(T$0), 0, 1), s_Ref_mutable_type(T$0), 1, (1) + (arity(T$0)))
            --*
            p_Ref_mutable_ind(old(extend_Ref_mutable(old(x), s_Option_type(T$0), 0, 1)), s_Option_type(T$0), 1, (1) + (arity(T$0)))
            "#,
        );
    }
    /// Result borrows that expire separately get a wand each.
    #[test]
    fn two_wands() {
        assert_wands(
            "fn f<'a, 'b>(x: &'a mut i32, y: &'b mut i32) -> (&'a mut i32, &'b mut i32) { (x, y) }",
            r#"
            p_2_Tuple_ind(result, s_Ref_mutable_type(s_Int_i32_type()), s_Ref_mutable_type(s_Int_i32_type()), 0, 1)
            --*
            p_Ref_mutable_ind(old(x), s_Int_i32_type(), 0, 1)

            p_2_Tuple_ind(result, s_Ref_mutable_type(s_Int_i32_type()), s_Ref_mutable_type(s_Int_i32_type()), 1, 2)
            --*
            p_Ref_mutable_ind(old(y), s_Int_i32_type(), 0, 1)
            "#,
        );
    }

    /// The result may borrow from both arguments, which have different
    /// lifetimes.
    #[test]
    fn two_sources() {
        assert_wands(
            "fn f<'a, 'b: 'a>(x: &'a mut i32, y: &'b mut i32) -> &'a mut i32 { x }",
            r#"
            p_Ref_mutable_ind(result, s_Int_i32_type(), 0, 1)
            --*
            p_Ref_mutable_ind(old(x), s_Int_i32_type(), 0, 1)
            p_Ref_mutable_ind(old(y), s_Int_i32_type(), 0, 1)
            "#,
        );
    }

    /// Two result lifetimes that both borrow from the same argument are given
    /// back together.
    #[test]
    fn two_targets() {
        assert_wands(
            "fn f<'a: 'b + 'c, 'b, 'c>(x: &'a mut i32) -> (&'b mut i32, &'c mut i32) { (x, todo!()) }",
            r#"
            p_2_Tuple_ind(result, s_Ref_mutable_type(s_Int_i32_type()), s_Ref_mutable_type(s_Int_i32_type()), 0, 1)
            p_2_Tuple_ind(result, s_Ref_mutable_type(s_Int_i32_type()), s_Ref_mutable_type(s_Int_i32_type()), 1, 2)
            --*
            p_Ref_mutable_ind(old(x), s_Int_i32_type(), 0, 1)
            "#,
        );
    }

    /// A lifetime that occurs twice in the result is one node.
    #[test]
    fn lifetime_twice() {
        assert_wands(
            "fn f<'a>(x: &'a mut (i32, i32)) -> (&'a mut i32, &'a mut i32) { (&mut x.0, &mut x.1) }",
            r#"
            p_2_Tuple_ind(result, s_Ref_mutable_type(s_Int_i32_type()), s_Ref_mutable_type(s_Int_i32_type()), 0, 1)
            p_2_Tuple_ind(result, s_Ref_mutable_type(s_Int_i32_type()), s_Ref_mutable_type(s_Int_i32_type()), 1, 2)
            --*
            p_Ref_mutable_ind(old(x), s_2_Tuple_type(s_Int_i32_type(), s_Int_i32_type()), 0, 1)
            "#,
        );
    }

    /// The callee stores `y` in `*x`, so the caller gets back `**x` from
    /// before the call, and `*y`, once it gives up `**x` after the call. The
    /// key of `**x` on the right side reads `*x` before the call.
    #[test]
    fn store_into_argument() {
        assert_wands(
            "fn f<'a, 'b>(x: &'a mut &'b mut i32, y: &'b mut i32) { *x = y; }",
            r#"
            p_Ref_mutable_ind(extend_Ref_mutable(old(x), s_Ref_mutable_type(s_Int_i32_type()), 0, 1), s_Ref_mutable_type(s_Int_i32_type()), 1, 2)
            --*
            p_Ref_mutable_ind(old(extend_Ref_mutable(old(x), s_Ref_mutable_type(s_Int_i32_type()), 0, 1)), s_Ref_mutable_type(s_Int_i32_type()), 1, 2)
            p_Ref_mutable_ind(old(y), s_Int_i32_type(), 0, 1)
            "#,
        );
    }

    #[test]
    fn contract_reborrow() {
        assert_contract(
            "fn f<'a>(x: &'a mut i32) -> &'a mut i32 { x }",
            r#"
            requires p_Ref_mutable_ind(p_Ref_mutable_snap(_1p, s_Int_i32_type()), s_Int_i32_type(), 0, 1)
            ensures p_Ref_mutable_ind(p_Ref_mutable_snap(_0p, s_Int_i32_type()), s_Int_i32_type(), 0, 1)
            "#,
        );
    }

    /// The nested lifetime is given back by the postcondition, on the referent
    /// of `x` after the call.
    #[test]
    fn contract_nested_references() {
        assert_contract(
            "fn f<'a, 'b>(x: &'a mut &'b mut i32) {}",
            r#"
            requires p_Ref_mutable_ind(p_Ref_mutable_snap(_1p, s_Ref_mutable_type(s_Int_i32_type())), s_Ref_mutable_type(s_Int_i32_type()), 0, 1)
            requires p_Ref_mutable_ind(extend_Ref_mutable(p_Ref_mutable_snap(_1p, s_Ref_mutable_type(s_Int_i32_type())), s_Ref_mutable_type(s_Int_i32_type()), 0, 1), s_Ref_mutable_type(s_Int_i32_type()), 1, 2)
            ensures p_Ref_mutable_ind(old(p_Ref_mutable_snap(_1p, s_Ref_mutable_type(s_Int_i32_type()))), s_Ref_mutable_type(s_Int_i32_type()), 0, 1)
            ensures p_Ref_mutable_ind(extend_Ref_mutable(old(p_Ref_mutable_snap(_1p, s_Ref_mutable_type(s_Int_i32_type()))), s_Ref_mutable_type(s_Int_i32_type()), 0, 1), s_Ref_mutable_type(s_Int_i32_type()), 1, 2)
            "#,
        );
    }

    /// The nested lifetimes are given back by the wand, not the postcondition.
    #[test]
    fn contract_store_into_argument() {
        assert_contract(
            "fn f<'a, 'b>(x: &'a mut &'b mut i32, y: &'b mut i32) { *x = y; }",
            r#"
            requires p_Ref_mutable_ind(p_Ref_mutable_snap(_1p, s_Ref_mutable_type(s_Int_i32_type())), s_Ref_mutable_type(s_Int_i32_type()), 0, 1)
            requires p_Ref_mutable_ind(extend_Ref_mutable(p_Ref_mutable_snap(_1p, s_Ref_mutable_type(s_Int_i32_type())), s_Ref_mutable_type(s_Int_i32_type()), 0, 1), s_Ref_mutable_type(s_Int_i32_type()), 1, 2)
            requires p_Ref_mutable_ind(p_Ref_mutable_snap(_2p, s_Int_i32_type()), s_Int_i32_type(), 0, 1)
            ensures p_Ref_mutable_ind(old(p_Ref_mutable_snap(_1p, s_Ref_mutable_type(s_Int_i32_type()))), s_Ref_mutable_type(s_Int_i32_type()), 0, 1)
            "#,
        );
    }

    #[test]
    fn contract_generic_referent() {
        assert_contract(
            "fn f<T>(x: &mut T) {}",
            r#"
            requires p_Ref_mutable_ind(p_Ref_mutable_snap(_1p, T$0), T$0, 0, 1)
            requires p_Ref_mutable_ind(extend_Ref_mutable(p_Ref_mutable_snap(_1p, T$0), T$0, 0, 1), T$0, 1, (1) + (arity(T$0)))
            ensures p_Ref_mutable_ind(old(p_Ref_mutable_snap(_1p, T$0)), T$0, 0, 1)
            ensures p_Ref_mutable_ind(extend_Ref_mutable(old(p_Ref_mutable_snap(_1p, T$0)), T$0, 0, 1), T$0, 1, (1) + (arity(T$0)))
            "#,
        );
    }
}
