use prusti_rustc_interface::{
    errors::MultiSpan,
    hir::{
        def_id::{DefId, LocalDefId},
        intravisit::{self, Visitor},
    },
    span::Span,
};

use crate::{
    environment::{EnvDiagnostic, EnvName, EnvQuery, Environment},
    specs::specifications::find_trait_method_substs,
    PrustiError,
};
use prusti_rustc_interface::{
    data_structures::fx::{FxHashMap, FxHashSet},
    hir::attrs::LangItem,
    middle::ty::{self, ClauseKind, GenericArgsRef},
    trait_selection::traits::{normalize_param_env_or_error, ObligationCause},
};
use prusti_specs::ExternSpecKind;
use std::cmp::{Eq, PartialEq};

pub enum ExternSpecResolverError {
    /// Occurs when the user declares an extern spec in an impl block but the
    /// annotated method is a trait method.
    ///
    /// # Example:
    /// ```
    /// trait A { fn a(); }
    /// struct S;
    /// impl A for S { fn a() {...} }
    /// #[extern_spec]
    /// impl S {
    ///     // specs
    ///     fn a();
    /// }
    /// ```
    InvalidExternSpecForTraitImpl(DefId, Span),

    /// Occurs when the type and const parameters of the extern spec are not
    /// those of the target, in the same order.
    ///
    /// # Example:
    /// ```
    /// #[extern_spec]
    /// impl<E, T> Result<T, E> {
    ///     fn is_ok(&self) -> bool;
    /// }
    /// ```
    /// The strings render the spec's parameters and the target's arguments in
    /// the spec's call to it.
    InvalidGenerics(DefId, Span, String, String),

    /// Occurs when a trait impl extern spec resolves to the trait method.
    ResolvedToDefault(DefId, Span),

    /// Occurs when the extern spec declares a trait bound that does not follow
    /// from the target definition's. The spec would otherwise be applied at
    /// every use of the target, including instantiations where the extra
    /// bound does not hold (and the spec expressions are not even well-formed).
    /// The span is the bound's, and the strings render the bound and the
    /// (sorted) bounds of the target. A bound of an impl block or a trait is
    /// checked for each of its methods but reported once.
    MismatchedBounds(DefId, Span, String, String),
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum ExternSpecDeclaration {
    /// An external specification for inherent methods of impl blocks.
    Inherent(DefId),

    /// An external specification for a trait method (first `DefId`), resolved to its implementation
    /// method (second `DefId`).
    TraitImpl(DefId, DefId),

    /// An external specification for a trait method.
    Trait(DefId),

    /// An external specification for a free-standing method.
    Method(DefId),
}

impl ExternSpecDeclaration {
    /// Constructs [ExternSpecDeclaration] from a method call (within the
    /// extern-spec function `caller_def_id`) with the given substitutions.
    fn from_method_call<'tcx>(
        caller_def_id: DefId,
        def_id: DefId,
        substs: GenericArgsRef<'tcx>,
        env_query: EnvQuery<'tcx>,
    ) -> Self {
        let is_impl_method = env_query.is_trait_method_impl(def_id);
        let is_trait_method = env_query.get_trait_of_assoc(def_id).is_some();
        let maybe_impl_def_id =
            env_query.find_impl_of_trait_method_call(caller_def_id, def_id, substs);

        if is_trait_method && maybe_impl_def_id.is_none() {
            Self::Trait(def_id)
        } else if is_trait_method && maybe_impl_def_id.is_some() {
            Self::TraitImpl(def_id, maybe_impl_def_id.unwrap())
        } else if is_impl_method {
            Self::Inherent(def_id)
        } else {
            Self::Method(def_id)
        }
    }

    /// Returns the **target** [DefId] for which an external spec was declared.
    pub fn get_target_def_id(&self) -> DefId {
        match self {
            Self::Inherent(did)
            | Self::TraitImpl(_, did)
            | Self::Trait(did)
            | Self::Method(did) => *did,
        }
    }
}

/// This struct is used to build a mapping of external functions to their
/// Prusti specifications (see `extern_fn_map`).
pub struct ExternSpecResolver<'tcx> {
    env_name: EnvName<'tcx>,
    env_query: EnvQuery<'tcx>,

    /// Maps real functions to Prusti-generated fake functions with specifications.
    pub extern_fn_map: FxHashMap<ExternSpecDeclaration, DefId>,

    /// Duplicate specifications detected, keyed by the `DefId` of the function
    /// to be specified.
    spec_duplicates: FxHashMap<DefId, Vec<(DefId, Span)>>,

    /// Encountered errors while resolving external specs.
    errors: Vec<ExternSpecResolverError>,
}

impl<'tcx> ExternSpecResolver<'tcx> {
    pub fn new(env: &Environment<'tcx>) -> Self {
        Self {
            env_name: env.name,
            env_query: env.query,
            extern_fn_map: FxHashMap::default(),
            spec_duplicates: FxHashMap::default(),
            errors: vec![],
        }
    }

    /// Registers an external function specification. The arguments for this
    /// function are the same as arguments given to a function visit in an
    /// intravisit visitor.
    ///
    /// In case of duplicates, the function is added to `spec_duplicates`, and
    /// will later (in `check_duplicates`) be reported as an error. Otherwise,
    /// the function is added to `extern_fn_map`.
    pub fn add_extern_fn(
        &mut self,
        fn_kind: intravisit::FnKind<'tcx>,
        fn_decl: &'tcx prusti_rustc_interface::hir::FnDecl,
        body_id: prusti_rustc_interface::hir::BodyId,
        span: Span,
        local_id: LocalDefId,
        extern_spec_kind: ExternSpecKind,
    ) {
        let mut visitor = ExternSpecVisitor {
            env_query: self.env_query,
            spec_found: None,
        };
        visitor.visit_fn(fn_kind, fn_decl, body_id, span, local_id);
        let current_def_id = local_id.to_def_id();
        if let Some((target_def_id, substs, span)) = visitor.spec_found {
            let extern_spec_decl = ExternSpecDeclaration::from_method_call(
                current_def_id,
                target_def_id,
                substs,
                self.env_query,
            );

            if matches!(extern_spec_kind, ExternSpecKind::Trait)
                && !matches!(extern_spec_decl, ExternSpecDeclaration::Trait(_))
            {
                unreachable!(
                    "External specification declared on a trait did not resolve to a trait method"
                );
            } else if matches!(extern_spec_kind, ExternSpecKind::TraitImpl)
                && !matches!(extern_spec_decl, ExternSpecDeclaration::TraitImpl(_, _))
            {
                unreachable!("External specification declared on a trait implementation did not resolve to a concrete type");
            }

            {
                // TODO: this resolution happens here but also in SpecCollector
                // maybe it can be done once only?
                let (resolved_def_id, resolved_substs) =
                    self.env_query
                        .resolve_method_call(current_def_id, target_def_id, substs);
                if matches!(extern_spec_kind, ExternSpecKind::TraitImpl)
                    && resolved_def_id == target_def_id
                {
                    // should have resolved but did not: resolved to default
                    self.errors.push(ExternSpecResolverError::ResolvedToDefault(
                        target_def_id,
                        span,
                    ));
                } else {
                    // The spec is applied at every use of the target, and its
                    // items are instantiated with the target's generic
                    // arguments by position. So the stub must pass its own type
                    // and const parameters to the target, in order. Lifetimes
                    // are erased in Prusti, so they need not match.
                    let non_regions = |args: GenericArgsRef<'tcx>| {
                        args.iter()
                            .filter(|arg| arg.as_region().is_none())
                            .collect::<Vec<_>>()
                    };
                    let declared = non_regions(self.env_query.identity_substs(current_def_id));
                    let target = non_regions(resolved_substs);
                    if declared != target {
                        let render = |args: Vec<ty::GenericArg<'tcx>>| {
                            let _trimmed_paths = ty::print::ForceTrimmedGuard::new();
                            args.into_iter()
                                .map(|arg| arg.to_string())
                                .collect::<Vec<_>>()
                                .join(", ")
                        };
                        self.errors.push(ExternSpecResolverError::InvalidGenerics(
                            resolved_def_id,
                            span,
                            render(declared),
                            render(target),
                        ));
                    } else if let Some((unimplied, target)) =
                        self.bounds_mismatch(current_def_id, span, resolved_def_id, resolved_substs)
                    {
                        for (bound_span, bound) in unimplied {
                            self.errors.push(ExternSpecResolverError::MismatchedBounds(
                                resolved_def_id,
                                bound_span,
                                bound,
                                target.clone(),
                            ));
                        }
                    }
                }
            }

            if self.extern_fn_map.contains_key(&extern_spec_decl) {
                self.register_duplicate_spec(target_def_id, current_def_id, span);
            } else {
                self.check_validity(extern_spec_kind, &extern_spec_decl, span);
                self.extern_fn_map
                    .insert(extern_spec_decl.clone(), current_def_id);
            }
        }
    }

    /// Checks the trait bounds declared by the extern spec (`spec_def_id`,
    /// including its impl block's bounds) against the target's. The spec is
    /// applied at every use of the target, so each declared bound must follow
    /// from the target's bounds; it may leave some of them out (some cannot
    /// even be named, e.g. a private sealing supertrait). `target_args` are
    /// the target's generic arguments in the stub's call to it, which state
    /// the target's bounds in the spec's parameters. The bounds of an impl
    /// method of a trait include those of the trait method, which may be
    /// stronger than the impl method's own: every use of the impl method goes
    /// through the trait. Returns `None` if every declared bound follows, and
    /// otherwise the declared bounds that do not, each rendered with its span
    /// (or `spec_span` if it has none), and the target's bounds, rendered.
    ///
    /// Only trait, projection and const-argument clauses are compared;
    /// regions are erased in Prusti, so outlives clauses are ignored.
    fn bounds_mismatch(
        &self,
        spec_def_id: DefId,
        spec_span: Span,
        target_def_id: DefId,
        target_args: GenericArgsRef<'tcx>,
    ) -> Option<(Vec<(Span, String)>, String)> {
        let compared_clauses = |clauses: ty::GenericClauses<'tcx>, args: GenericArgsRef<'tcx>| {
            clauses
                .instantiate(self.env_query.tcx(), args)
                .into_iter()
                .map(|(clause, span)| {
                    (
                        self.env_query
                            .tcx()
                            .erase_and_anonymize_regions(clause.skip_normalization()),
                        span,
                    )
                })
                .filter(|(clause, _)| {
                    matches!(
                        clause.kind().skip_binder(),
                        ClauseKind::Trait(_)
                            | ClauseKind::Projection(_)
                            | ClauseKind::ConstArgHasType(..)
                    )
                })
        };
        // The sizedness hierarchy below `Sized` is not compared, though it is
        // part of the target's environment: surface `?Sized` means
        // `MetaSized`, while std traits may use the unstable `PointeeSized`
        // (which leaves no clause at all), and the two differ only for extern
        // types.
        let below_sized = |clause: &ty::Clause<'tcx>| {
            matches!(
                clause.kind().skip_binder(),
                ClauseKind::Trait(pred) if matches!(
                    self.env_query.tcx().as_lang_item(pred.def_id()),
                    Some(LangItem::MetaSized | LangItem::PointeeSized)
                )
            )
        };
        let trait_method =
            find_trait_method_substs(self.env_query.tcx(), target_def_id, target_args);
        let target = compared_clauses(self.env_query.tcx().clauses_of(target_def_id), target_args)
            .chain(
                trait_method
                    .into_iter()
                    .flat_map(|(trait_method, trait_args)| {
                        compared_clauses(
                            self.env_query.tcx().clauses_of(trait_method),
                            self.env_query.tcx().mk_args(trait_args),
                        )
                    }),
            )
            .map(|(clause, _)| clause)
            .collect::<FxHashSet<_>>();
        let target_env = normalize_param_env_or_error(
            self.env_query.tcx(),
            ty::ParamEnv::new(
                self.env_query
                    .tcx()
                    .mk_clauses_from_iter(target.iter().copied()),
            ),
            ObligationCause::dummy(),
        );
        let typing_env = ty::TypingEnv::new(target_env, ty::TypingMode::PostAnalysis);
        // Every use of the target needs its by-value inputs and output to be
        // `Sized`, so a bound of the spec that only requires this always
        // holds. A trait spec has to declare such a bound when `Self` or a
        // parameter of the trait may be unsized, since the stub and the spec
        // items take the arguments by value. A struct or tuple is `Sized`
        // exactly if its last field is (the only one that may be unsized), so
        // the bound is on the struct tail: `T: Sized` for an input of type
        // `T`, `Cell<T>` or `(u8, T)`.
        let by_value_tails = self
            .env_query
            .tcx()
            .erase_and_anonymize_regions(
                self.env_query
                    .tcx()
                    .fn_sig(target_def_id)
                    .instantiate(self.env_query.tcx(), target_args)
                    .skip_normalization(),
            )
            .skip_binder()
            .inputs_and_output
            .iter()
            .map(|ty| {
                let ty = self
                    .env_query
                    .tcx()
                    .try_normalize_erasing_regions(typing_env, ty::Unnormalized::new(ty))
                    .unwrap_or(ty);
                self.env_query.tcx().struct_tail_raw(
                    ty,
                    &ObligationCause::dummy(),
                    |ty| {
                        self.env_query
                            .tcx()
                            .try_normalize_erasing_regions(typing_env, ty)
                            .unwrap_or_else(|_| ty.skip_normalization())
                    },
                    || {},
                )
            })
            .collect::<Vec<_>>();
        let sized_by_value = |clause: &ty::Clause<'tcx>| {
            matches!(
                clause.kind().skip_binder(),
                ClauseKind::Trait(pred) if self.env_query.tcx().is_lang_item(pred.def_id(), LangItem::Sized)
                    && by_value_tails.contains(&pred.self_ty())
            )
        };
        // Each declared clause, normalized in the target's environment, with
        // the clause as written (for rendering) and its span. A clause
        // declared twice (e.g. on the impl block and on the method) is checked
        // once.
        let mut seen = FxHashSet::default();
        let declared = compared_clauses(
            self.env_query.tcx().clauses_of(spec_def_id),
            self.env_query.identity_substs(spec_def_id),
        )
        .map(|(clause, span)| {
            let normalized = self
                .env_query
                .tcx()
                .try_normalize_erasing_regions(typing_env, ty::Unnormalized::new(clause))
                .unwrap_or(clause);
            (normalized, clause, span)
        })
        .filter(|(normalized, clause, _)| {
            !below_sized(clause) && !sized_by_value(normalized) && seen.insert(*clause)
        })
        .collect::<Vec<_>>();
        // The clauses' regions are erased, so they are checked modulo regions.
        let unimplied = declared
            .into_iter()
            .filter(|(normalized, _, _)| {
                !self
                    .env_query
                    .clause_holds_modulo_regions(target_env, *normalized)
            })
            .collect::<Vec<_>>();
        if unimplied.is_empty() {
            return None;
        }
        // The spec's type and const parameters are the target's, in order, so
        // the bounds are shown with the target's parameters, as the target
        // names them (e.g. `Self` for the parameter a trait spec stands for).
        let mut target_params = self
            .env_query
            .identity_substs(target_def_id)
            .iter()
            .filter(|arg| arg.as_region().is_none());
        let target_names =
            ty::GenericArgs::for_item(self.env_query.tcx(), spec_def_id, |param, _| {
                match param.kind {
                    ty::GenericParamDefKind::Lifetime => {
                        self.env_query.tcx().lifetimes.re_erased.into()
                    }
                    _ => target_params
                        .next()
                        .expect("the extern spec has the target's type and const parameters"),
                }
            });
        let render = |clause: ty::Clause<'tcx>| {
            let _trimmed_paths = ty::print::ForceTrimmedGuard::new();
            ty::EarlyBinder::bind(self.env_query.tcx(), clause)
                .instantiate(self.env_query.tcx(), target_names)
                .skip_normalization()
                .to_string()
        };
        let mut target = target
            .into_iter()
            .filter(|clause| !below_sized(clause))
            .map(render)
            .collect::<Vec<_>>();
        target.sort();
        let unimplied = unimplied
            .into_iter()
            .map(|(_, clause, span)| {
                let span = if span.is_dummy() { spec_span } else { span };
                (span, render(clause))
            })
            .collect();
        Some((unimplied, target.join(", ")))
    }

    fn register_duplicate_spec(&mut self, decl_def_id: DefId, dup_spec_def_id: DefId, span: Span) {
        self.spec_duplicates
            .entry(decl_def_id)
            .or_default()
            .push((dup_spec_def_id, span));
    }

    /// Checks whether the encoded method call (call to `spec_for_def_id`) is valid.
    /// See [ExternSpecResolverError] for possible errors (including examples)
    fn check_validity(
        &mut self,
        extern_spec_kind: ExternSpecKind,
        declared_spec: &ExternSpecDeclaration,
        span: Span,
    ) {
        if matches!(extern_spec_kind, ExternSpecKind::InherentImpl) {
            if let ExternSpecDeclaration::TraitImpl(def_id, _) = declared_spec {
                self.errors
                    .push(ExternSpecResolverError::InvalidExternSpecForTraitImpl(
                        *def_id, span,
                    ));
            }
        }
        // Note: A check for matches!(extern_spec_kind, ExternSpecKind::TraitImpl) && !is_trait_method
        // is not needed because this does not typecheck (using UFCS call syntax in encoding).
    }

    /// Report errors encountered when resolving extern specs
    pub fn check_errors(&self, env_diagnostic: &EnvDiagnostic<'tcx>) {
        self.check_duplicates(env_diagnostic);

        let mut reported_bounds = FxHashSet::default();
        for error in self.errors.iter() {
            match error {
                // TODO: branches look very similar
                ExternSpecResolverError::InvalidExternSpecForTraitImpl(def_id, span) => {
                    let function_name = self.env_name.get_item_name(*def_id);
                    let err_note = format!("Defined an external spec for trait method '{function_name}'. Use '#[extern_spec] impl TheTrait for TheStruct {{ ... }}' syntax instead.");
                    PrustiError::incorrect(
                        "Invalid external specification",
                        MultiSpan::from_span(*span),
                    )
                    .add_note(err_note, None)
                    .emit(env_diagnostic);
                }
                ExternSpecResolverError::InvalidGenerics(def_id, span, declared, target) => {
                    let function_name = self.env_name.get_item_name(*def_id);
                    let err_note = format!(
                        "The type and const parameters of the external spec for \
                        '{function_name}' must be those of the target, in the same order, \
                        since the spec is applied at every use of the target. Parameters of \
                        the spec: [{declared}]; arguments of the target as written in the \
                        spec: [{target}]."
                    );
                    PrustiError::incorrect(
                        "Invalid external specification",
                        MultiSpan::from_span(*span),
                    )
                    .add_note(err_note, None)
                    .emit(env_diagnostic);
                }
                ExternSpecResolverError::ResolvedToDefault(def_id, span) => {
                    let function_name = self.env_name.get_item_name(*def_id);
                    let err_note = format!("Specified method ('{function_name}') resolved to the trait's implementation. Add specification to the trait instead.");
                    PrustiError::incorrect(
                        "Invalid external specification",
                        MultiSpan::from_span(*span),
                    )
                    .add_note(err_note, None)
                    .emit(env_diagnostic);
                }
                ExternSpecResolverError::MismatchedBounds(def_id, span, declared, target) => {
                    if !reported_bounds.insert((*span, declared)) {
                        continue;
                    }
                    let function_name = self.env_name.get_item_name(*def_id);
                    let err_note = format!(
                        "The bound `{declared}` does not follow from the bounds of \
                        '{function_name}' ([{target}]). An external spec must not declare such \
                        bounds, since it is applied at every use of its target."
                    );
                    PrustiError::incorrect(
                        "Invalid external specification",
                        MultiSpan::from_span(*span),
                    )
                    .add_note(err_note, None)
                    .emit(env_diagnostic);
                }
            }
        }
    }

    /// Report errors for duplicate specifications found during specification
    /// collection.
    fn check_duplicates(&self, env_diagnostic: &EnvDiagnostic<'tcx>) {
        for (&def_id, specs) in self.spec_duplicates.iter() {
            let function_name = self.env_name.get_item_name(def_id);
            PrustiError::incorrect(
                format!("duplicate specification for {function_name}"),
                MultiSpan::from_spans(specs.iter().map(|s| s.1).collect()),
            )
            .emit(env_diagnostic);
        }
    }
}

/// A visitor that is called on external specification methods, as generated by
/// the external spec rewriter, looking specifically for the call to the
/// external function.
///
/// TODO: is the HIR representation stable enought that this could be
/// accomplished by a nested match rather than a full visitor?
struct ExternSpecVisitor<'tcx> {
    env_query: EnvQuery<'tcx>,
    spec_found: Option<(DefId, GenericArgsRef<'tcx>, Span)>,
}

impl<'tcx> Visitor<'tcx> for ExternSpecVisitor<'tcx> {
    type NestedFilter = prusti_rustc_interface::middle::hir::nested_filter::All;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.env_query.tcx()
    }

    fn visit_expr(&mut self, ex: &'tcx prusti_rustc_interface::hir::Expr<'tcx>) {
        if self.spec_found.is_some() {
            return;
        }
        if let prusti_rustc_interface::hir::ExprKind::Call(callee_expr, _arguments) = ex.kind {
            if let prusti_rustc_interface::hir::ExprKind::Path(ref qself) = callee_expr.kind {
                let tyck_res = self.env_query.tcx().typeck(callee_expr.hir_id.owner.def_id);
                let substs = tyck_res.node_args(callee_expr.hir_id);
                let res = tyck_res.qpath_res(qself, callee_expr.hir_id);
                if let prusti_rustc_interface::hir::def::Res::Def(_, def_id) = res {
                    self.spec_found = Some((def_id, substs, ex.span));
                    return;
                }
            }
        }
        intravisit::walk_expr(self, ex);
    }
}
