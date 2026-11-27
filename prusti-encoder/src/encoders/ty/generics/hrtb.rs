use prusti_rustc_interface::{
    data_structures::stable_hash::{FromStableHash, StableHash, StableHasher, StableHasherHash},
    middle::ty::{self, TypeFoldable, TypeSuperFoldable, TypeVisitableExt, Unnormalized, Upcast},
    span::{DUMMY_SP, def_id::DefId, symbol},
    trait_selection::{
        infer::{
            DefineOpaqueTypes, InferCtxt, TyCtxtInferExt, at::ToTrace,
            resolve::OpportunisticRegionResolver,
        },
        traits::{FulfillmentEngine, FulfillmentError, NormalizeExt, ObligationCause},
    },
};
use rustc_hash::FxHashMap;
use task_encoder::{EncodeFullResult, OutputRefAny, TaskEncoder, TaskEncoderDependencies};
use vir::{CallableIdn, CastType, FunctionIdn, ViperIdent, vir_format_identifier};

use crate::encoders::{
    TyUsePureEnc,
    ty::{
        RustTyDecomposition,
        generics::{
            GArgs, GArgsTyEnc, GParams,
            trait_impls::{self, TraitImplEnc},
        },
        lifted::TyConstructorEnc,
    },
};

/// A higher-ranked where-clause, such as `for<'a> &'a U: Tr`, with its
/// maximal subterms that mention none of its bound regions (here `U`)
/// replaced by parameters, in order of occurrence: `for<'a> &'a H0: Tr`.
/// Instantiating the parameters with the subterms gives the clause back, up
/// to its free regions, which the skeleton erases (impls cannot tell free
/// regions apart). So the clause of a callee instantiated at its caller's
/// types has the same skeleton and arguments as the caller's own clause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HrtbSkeleton<'tcx> {
    clause: ty::Clause<'tcx>,
    /// The parameters, types and consts, numbered together.
    params: ty::GenericArgsRef<'tcx>,
    /// The types of the const parameters, in order.
    const_tys: &'tcx ty::List<ty::Ty<'tcx>>,
}

impl<'tcx> HrtbSkeleton<'tcx> {
    /// The skeleton of `clause`, a where-clause in force in `ctx`, and its
    /// arguments, if the clause is higher-ranked (its binder binds a region
    /// that it mentions) and its skeleton has no projections (see the
    /// comment in the body). Errs on a const in the clause that is neither a
    /// value nor a parameter.
    pub fn of(
        ctx: GParams<'tcx>,
        clause: ty::Binder<'tcx, ty::ClauseKind<'tcx>>,
    ) -> Result<Option<(Self, ty::GenericArgsRef<'tcx>)>, String> {
        if !clause.skip_binder().has_escaping_bound_vars() {
            return Ok(None);
        }
        struct Abstract<'tcx> {
            tcx: ty::TyCtxt<'tcx>,
            ctx: GParams<'tcx>,
            params: Vec<ty::GenericArg<'tcx>>,
            args: Vec<ty::GenericArg<'tcx>>,
            const_tys: Vec<ty::Ty<'tcx>>,
            unsupported: Option<ty::Const<'tcx>>,
        }
        impl<'tcx> ty::TypeFolder<ty::TyCtxt<'tcx>> for Abstract<'tcx> {
            fn cx(&self) -> ty::TyCtxt<'tcx> {
                self.tcx
            }
            fn fold_ty(&mut self, t: ty::Ty<'tcx>) -> ty::Ty<'tcx> {
                // `t` mentions a region bound outside it, by the clause's
                // binder or by a binder inside the clause, so only its
                // subterms can be abstracted.
                if t.has_escaping_bound_vars() {
                    return t.super_fold_with(self);
                }
                let param = param_ty(self.tcx, self.params.len() as u32);
                self.params.push(param.into());
                self.args.push(t.into());
                param
            }
            fn fold_region(&mut self, r: ty::Region<'tcx>) -> ty::Region<'tcx> {
                if r.is_bound() {
                    r
                } else {
                    self.tcx.lifetimes.re_erased
                }
            }
            fn fold_const(&mut self, c: ty::Const<'tcx>) -> ty::Const<'tcx> {
                let ty = match c.kind() {
                    ty::ConstKind::Value(v) => v.ty,
                    ty::ConstKind::Param(p) => self.ctx.expect_const(p.index as usize).1,
                    _ if !c.has_param() => return c,
                    _ => {
                        self.unsupported = Some(c);
                        return c;
                    }
                };
                let param = param_const(self.tcx, self.params.len() as u32);
                self.params.push(param.into());
                self.args.push(c.into());
                self.const_tys.push(ty);
                param
            }
        }
        vir::with_vcx(|vcx| {
            let mut abstract_ = Abstract {
                tcx: vcx.tcx(),
                ctx,
                params: Vec::new(),
                args: Vec::new(),
                const_tys: Vec::new(),
                unsupported: None,
            };
            let kind = clause.skip_binder().fold_with(&mut abstract_);
            if let Some(c) = abstract_.unsupported {
                return Err(format!(
                    "the const `{c}` in a higher-ranked where-clause is not supported"
                ));
            }
            // A projection that mentions a bound region (one without was
            // abstracted into a parameter above), as in
            // `for<'a> <U as Lend<'a>>::Item: Tr`, is decided by normalizing
            // it, for which an axiom per impl of `Tr` would have to solve for
            // `U`. Such a clause is stated with its bound regions erased only,
            // which as an impl guard is weaker than the clause: with
            // `type Item = &'a u8` and `impl Tr for &'static u8`, the impl
            // applies where rustc rejects it (known unsoundness).
            if kind.has_aliases() {
                return Ok(None);
            }
            let skeleton = vcx
                .tcx()
                .anonymize_bound_vars(ty::Binder::bind_with_vars(kind, clause.bound_vars()));
            Ok(Some((
                HrtbSkeleton {
                    clause: skeleton.upcast(vcx.tcx()),
                    params: vcx.tcx().mk_args(&abstract_.params),
                    const_tys: vcx.tcx().mk_type_list(&abstract_.const_tys),
                },
                vcx.tcx().mk_args(&abstract_.args),
            )))
        })
    }

    fn bound_var_count(self) -> usize {
        self.clause.kind().bound_vars().len()
    }

    fn trait_did(self) -> DefId {
        vir::with_vcx(|vcx| match self.clause.kind().skip_binder() {
            ty::ClauseKind::Trait(pred) => pred.def_id(),
            ty::ClauseKind::Projection(pred) => pred.trait_def_id(vcx.tcx()),
            other => unreachable!("not a trait or projection clause: {other:?}"),
        })
    }
}

fn param_ty<'tcx>(tcx: ty::TyCtxt<'tcx>, index: u32) -> ty::Ty<'tcx> {
    ty::Ty::new_param(tcx, index, symbol::Symbol::intern(&format!("H{index}")))
}

fn param_const<'tcx>(tcx: ty::TyCtxt<'tcx>, index: u32) -> ty::Const<'tcx> {
    let name = symbol::Symbol::intern(&format!("H{index}"));
    ty::Const::new_param(tcx, ty::ParamConst::new(index, name))
}

/// Encodes a higher-ranked where-clause as an opaque predicate of the
/// arguments of its skeleton (see [`HrtbSkeleton`]), which holds where
/// rustc proves the clause: for every instantiation of its bound regions,
/// i.e. with placeholders for them. Instantiating the bound regions with
/// erased ones would be weaker: `for<'a> &'a u8: Tr` does not follow from
/// `impl Tr for &'static u8`, but `&'erased u8: Tr` does, so an impl guarded
/// by the clause would apply where rustc rejects it (and selects another).
///
/// Each impl of the clause's trait that rustc accepts for the clause's
/// placeholder instantiation contributes an axiom (see [`HrtbImplEnc`]).
pub struct HrtbEnc;

#[derive(Debug, Clone, Copy)]
pub struct HrtbEncOutputRef<'vir> {
    pub holds: FunctionIdn<'vir, (vir::ManyTyVal, vir::ManyCSnap), vir::Bool>,
}

impl<'vir> OutputRefAny for HrtbEncOutputRef<'vir> {}

/// rustc's stable hash of a value, which identifies it structurally (paths
/// by their `DefPathHash`), as for `TypeId`s.
struct StableHash128(u128);

impl FromStableHash for StableHash128 {
    type Hash = StableHasherHash;

    fn from(StableHasherHash([low, high]): Self::Hash) -> Self {
        StableHash128(u128::from(low) | (u128::from(high) << 64))
    }
}

impl TaskEncoder for HrtbEnc {
    task_encoder::encoder_cache!(HrtbEnc);
    const ENCODER_NAME: &'static str = "higher-ranked clause encoder";

    type TaskDescription<'vir> = HrtbSkeleton<'vir>;
    type OutputRef<'vir> = HrtbEncOutputRef<'vir>;
    type OutputFullLocal<'vir> = vir::Domain<'vir>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        vir::with_vcx(|vcx| {
            let trait_did = task_key.trait_did();
            let trait_name = ViperIdent::from_def_id(vcx, trait_did);
            // The skeleton's stable hash tells skeletons apart (its printed
            // form does not: paths are printed without disambiguators). Only
            // the clause is hashed: its parameters and the types of its
            // consts are determined by it.
            let StableHash128(hash) = vcx.tcx().with_stable_hashing_context(|mut hcx| {
                let mut hasher = StableHasher::new();
                task_key.clause.stable_hash(&mut hcx, &mut hasher);
                hasher.finish()
            });
            let ty_count = task_key.params.len() - task_key.const_tys.len();
            let ty_args = vcx.alloc_slice(&vec![vir::TYPE_TYVAL; ty_count]);
            let const_args = task_key
                .const_tys
                .iter()
                .map(|ty| {
                    let ty = RustTyDecomposition::from_ty(ty, GParams::empty());
                    Ok(deps.require_ref::<TyUsePureEnc>(ty)?.snapshot.downcast_ty())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let holds = FunctionIdn::new(
                vir_format_identifier!(vcx, "Hrtb_{trait_name}_{hash:032x}"),
                (ty_args, vcx.alloc_slice(&const_args)),
                vir::TYPE_BOOL,
            );
            deps.emit_output_ref(*task_key, HrtbEncOutputRef { holds })?;

            let skeleton = *task_key;
            for impl_did in trait_impls::implementing_impls(vcx.tcx(), trait_did) {
                let keys = trait_impls::impl_unlock_keys(impl_did);
                let span = vcx.tcx().def_span(impl_did);
                TyConstructorEnc::on_all_requested(keys, move || {
                    let _ = HrtbImplEnc::encode((skeleton, impl_did), false, span);
                });
            }

            let domain = vcx.mk_domain(
                vir_format_identifier!(vcx, "hrtb_{}", holds.name()),
                &[],
                &[],
                vcx.alloc_slice(&[vcx.mk_domain_function(holds, false, None)]),
                None,
            );
            Ok((domain, ()))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for domain in Self::all_outputs_local(program) {
            program.add_domain(domain);
        }
    }
}

/// The axiom by which an impl makes a higher-ranked clause hold:
/// `forall vars :: {holds(args)} where-clauses ==> holds(args)`, where
/// `args` and `vars` come from matching the clause against the impl as
/// rustc's `match_impl` does. The clause's bound regions become placeholders
/// first, and the impl's parameters inference variables afterwards, so that
/// they can name the placeholders; the leak check then rejects a match that
/// relates a placeholder to anything else (e.g. `'static`). The arguments
/// left open by the match become the axiom's variables, let-bound to a
/// projection if one of the where-clauses determines them (as in
/// `TraitImplEnc::guarded_forall`). A where-clause of the impl that mentions
/// a placeholder holds for every placeholder, so it becomes a higher-ranked
/// clause itself. An impl that cannot be matched this way contributes no
/// axiom, which only loses facts: its header or associated type has
/// aliases, the associated type is generic or its value comes from
/// elsewhere (a trait default or a specialized impl), the clause projects to
/// an associated const, a const whose type mentions a parameter is left
/// open, or a placeholder is in an outlives clause.
pub struct HrtbImplEnc;

impl TaskEncoder for HrtbImplEnc {
    task_encoder::encoder_cache!(HrtbImplEnc);
    const ENCODER_NAME: &'static str = "higher-ranked clause impl encoder";

    type TaskDescription<'vir> = (HrtbSkeleton<'vir>, DefId);
    type OutputFullLocal<'vir> = Option<vir::Domain<'vir>>;

    fn task_to_key<'vir>(task: &Self::TaskDescription<'vir>) -> Self::TaskKey<'vir> {
        *task
    }

    fn do_encode_full<'vir>(
        task_key: &Self::TaskKey<'vir>,
        deps: &mut TaskEncoderDependencies<'vir, Self>,
    ) -> EncodeFullResult<'vir, Self> {
        deps.emit_output_ref(*task_key, ())?;
        let (skeleton, impl_did) = *task_key;
        let Some(matched) = match_impl(skeleton, impl_did) else {
            return Ok((None, ()));
        };
        vir::with_vcx(|vcx| {
            let holds = deps.require_ref::<HrtbEnc>(skeleton)?.holds;
            let ctx = matched.context;
            let args = deps.require_dep::<GArgsTyEnc>(GArgs::new(ctx, matched.args))?;
            let app = holds(args.get_ty(), args.get_const());
            // The variables occurring in the arguments are bound by the
            // trigger; the where-clauses' projections bind the others
            // (rustc rejects an impl parameter determined by neither, E0207).
            let axiom = TraitImplEnc::guarded_forall_pinned(
                vcx,
                deps,
                ctx,
                ctx,
                matched.args,
                &[],
                app.upcast_ty(),
                app,
            )?;
            let name = vir_format_identifier!(
                vcx,
                "{}_{}",
                holds.name(),
                trait_impls::impl_name(vcx, impl_did)
            );
            let domain = vcx.mk_domain(
                vir_format_identifier!(vcx, "hrtb_{name}"),
                &[],
                vcx.alloc_slice(&[vcx.mk_domain_axiom(name, axiom)]),
                &[],
                None,
            );
            Ok((Some(domain), ()))
        })
    }

    fn emit_outputs<'vir>(program: &mut task_encoder::Program<'vir>) {
        for domain in Self::all_outputs_local(program).into_iter().flatten() {
            program.add_domain(domain);
        }
    }
}

/// The result of matching a skeleton against an impl.
struct Matched<'tcx> {
    /// One parameter per variable the match leaves open; its param env holds
    /// the impl's where-clauses at the match.
    context: GParams<'tcx>,
    /// The skeleton's arguments at the match, over those parameters.
    args: ty::GenericArgsRef<'tcx>,
}

/// Matches `skeleton` against `impl_did` as described at [`HrtbImplEnc`];
/// `None` if the impl does not apply or cannot be matched.
fn match_impl<'tcx>(skeleton: HrtbSkeleton<'tcx>, impl_did: DefId) -> Option<Matched<'tcx>> {
    vir::with_vcx(|vcx| {
        let infcx = vcx.tcx().infer_ctxt().build(ty::TypingMode::PostAnalysis);
        let vars = vcx.tcx().mk_args_from_iter(skeleton.params.iter().map(
            |param| -> ty::GenericArg<'tcx> {
                match param.kind() {
                    ty::GenericArgKind::Type(_) => infcx.next_ty_var(DUMMY_SP).into(),
                    ty::GenericArgKind::Const(_) => infcx.next_const_var(DUMMY_SP).into(),
                    ty::GenericArgKind::Lifetime(_) => {
                        unreachable!("regions are not abstracted")
                    }
                }
            },
        ));
        let clause = ty::EarlyBinder::bind(vcx.tcx(), skeleton.clause.kind())
            .instantiate(vcx.tcx(), vars)
            .skip_normalization();
        // Equates `a` and `b`, requiring nothing further.
        fn eq<'tcx, T: ToTrace<'tcx>>(infcx: &InferCtxt<'tcx>, a: T, b: T) -> Option<()> {
            let ok = infcx
                .at(&ObligationCause::dummy(), ty::ParamEnv::empty())
                .eq(DefineOpaqueTypes::No, a, b)
                .ok()?;
            ok.obligations.is_empty().then_some(())
        }
        infcx.probe(|snapshot| {
            let outer_universe = infcx.universe();
            let kind = infcx.enter_forall_and_leak_universe(clause);
            let placeholder_universe = infcx.universe();
            let impl_args = infcx.fresh_args_for_item(DUMMY_SP, impl_did);
            let impl_trait_ref = vcx
                .tcx()
                .impl_trait_ref(impl_did)
                .instantiate(vcx.tcx(), impl_args)
                .skip_normalization();
            if impl_trait_ref.has_aliases() {
                return None;
            }
            match kind {
                ty::ClauseKind::Trait(pred) => {
                    if pred.polarity != ty::PredicatePolarity::Positive {
                        return None;
                    }
                    eq(&infcx, pred.trait_ref, impl_trait_ref)?;
                }
                ty::ClauseKind::Projection(pred) => {
                    let trait_ref = pred.projection_term.trait_ref(vcx.tcx());
                    // Generic associated types are not supported.
                    if pred.projection_term.args.len() != trait_ref.args.len() {
                        return None;
                    }
                    eq(&infcx, trait_ref, impl_trait_ref)?;
                    // The value is instantiated with this impl's arguments,
                    // so it must be this impl's own item.
                    let leaf = trait_impls::final_leaf_def(vcx.tcx(), impl_did, pred.def_id())?;
                    if leaf.item.container_id(vcx.tcx()) != impl_did {
                        return None;
                    }
                    let value = vcx
                        .tcx()
                        .type_of(leaf.item.def_id)
                        .instantiate(vcx.tcx(), impl_args)
                        .skip_normalization();
                    if value.has_aliases() {
                        return None;
                    }
                    eq(&infcx, value, pred.term.as_type()?)?;
                }
                _ => return None,
            }
            infcx.leak_check(outer_universe, Some(snapshot)).ok()?;

            // The types of the const variables: the skeleton's and the
            // impl's const parameters. One whose type mentions a parameter
            // stays untyped, so the match fails if it is left open.
            let skeleton_consts = vars
                .iter()
                .filter_map(ty::GenericArg::as_const)
                .zip(skeleton.const_tys);
            let impl_consts = vcx
                .tcx()
                .generics_of(impl_did)
                .own_params
                .iter()
                .filter(|param| matches!(param.kind, ty::GenericParamDefKind::Const { .. }))
                .map(|param| {
                    let ty = vcx.tcx().type_of(param.def_id).instantiate_identity();
                    (
                        impl_args[param.index as usize].expect_const(),
                        ty.skip_normalization(),
                    )
                });
            let const_tys = skeleton_consts
                .chain(impl_consts)
                .filter(|(_, ty)| !ty.has_param())
                .filter_map(|(c, ty)| match c.kind() {
                    ty::ConstKind::Infer(ty::InferConst::Var(vid)) => {
                        Some((infcx.root_const_var(vid), ty))
                    }
                    _ => None,
                })
                .collect();
            let mut generalize = Generalize {
                infcx: &infcx,
                placeholder_universe,
                depth: ty::INNERMOST,
                extra_vars: 0,
                params: Vec::new(),
                ty_roots: FxHashMap::default(),
                const_roots: FxHashMap::default(),
                const_tys,
                const_clauses: Vec::new(),
                failed: false,
            };
            // The impl's where-clauses, normalized where the match determines
            // enough, as rustc normalizes them once the impl is selected;
            // otherwise a clause stays as it is, and no impl satisfies it.
            // Normalizing may infer the match's variables, and may relate
            // the placeholders to other regions, so the where-clauses are
            // normalized, and the leak check rerun, before the match is
            // generalized.
            let kinds = vcx
                .tcx()
                .clauses_of(impl_did)
                .instantiate(vcx.tcx(), impl_args)
                .clauses
                .into_iter()
                .map(|clause| {
                    let kind = clause.skip_normalization().kind();
                    infcx
                        .commit_if_ok(|_| {
                            let mut fulfill_cx: FulfillmentEngine<'tcx, FulfillmentError<'tcx>> =
                                FulfillmentEngine::new(&infcx);
                            infcx
                                .at(&ObligationCause::dummy(), ty::ParamEnv::empty())
                                .deeply_normalize(Unnormalized::new(kind), &mut fulfill_cx)
                        })
                        .unwrap_or(kind)
                })
                .collect::<Vec<_>>();
            infcx.leak_check(outer_universe, Some(snapshot)).ok()?;
            // The skeleton's variables are of the outer universe, so they
            // cannot name the placeholders.
            let args = generalize.fold(vars);
            let mut clauses = Vec::new();
            for kind in kinds {
                let resolved = generalize.resolve(kind);
                if resolved.has_placeholders()
                    && matches!(
                        kind.skip_binder(),
                        ty::ClauseKind::RegionOutlives(..) | ty::ClauseKind::TypeOutlives(..)
                    )
                {
                    return None;
                }
                // The clause's own bound variables come first; the
                // placeholders are bound after them.
                generalize.extra_vars = kind.bound_vars().len() as u32;
                let inner = generalize.fold(kind.skip_binder());
                let mut bound_vars = kind.bound_vars().to_vec();
                bound_vars.extend(
                    (0..skeleton.bound_var_count())
                        .map(|_| ty::BoundVariableKind::Region(ty::BoundRegionKind::Anon)),
                );
                let kind = ty::Binder::bind_with_vars(
                    inner,
                    vcx.tcx().mk_bound_variable_kinds(&bound_vars),
                );
                clauses.push(vcx.tcx().anonymize_bound_vars(kind).upcast(vcx.tcx()));
            }
            if generalize.failed {
                return None;
            }
            // Skip the `ConstArgHasType` clauses that the impl's where-clauses
            // already state.
            for clause in generalize.const_clauses {
                if !clauses.contains(&clause) {
                    clauses.push(clause);
                }
            }
            let env = ty::ParamEnv::new(vcx.tcx().mk_clauses(&clauses));
            Some(Matched {
                context: GParams::new(vcx.tcx().mk_args(&generalize.params), env),
                args,
            })
        })
    })
}

/// Replaces the inference variables left open by a match with parameters
/// (numbered by first occurrence, typed by `const_tys` for consts), the
/// placeholders with variables bound after the `extra_vars` of the innermost
/// enclosing binder of the value being folded, and every other region with
/// the erased one.
struct Generalize<'a, 'tcx> {
    infcx: &'a InferCtxt<'tcx>,
    placeholder_universe: ty::UniverseIndex,
    depth: ty::DebruijnIndex,
    extra_vars: u32,
    /// The parameters, by index.
    params: Vec<ty::GenericArg<'tcx>>,
    ty_roots: FxHashMap<ty::TyVid, ty::Ty<'tcx>>,
    const_roots: FxHashMap<ty::ConstVid, ty::Const<'tcx>>,
    /// The types of the const variables, by root.
    const_tys: FxHashMap<ty::ConstVid, ty::Ty<'tcx>>,
    /// The types of the const parameters, as a param env states them.
    const_clauses: Vec<ty::Clause<'tcx>>,
    failed: bool,
}

impl<'a, 'tcx> Generalize<'a, 'tcx> {
    /// `value` with its inference variables resolved as far as the match
    /// determines them.
    fn resolve<T: TypeFoldable<ty::TyCtxt<'tcx>>>(&self, value: T) -> T {
        let value = self.infcx.resolve_vars_if_possible(value);
        value.fold_with(&mut OpportunisticRegionResolver::new(self.infcx))
    }

    fn fold<T: TypeFoldable<ty::TyCtxt<'tcx>>>(&mut self, value: T) -> T {
        let value = self.resolve(value);
        value.fold_with(self)
    }
}

impl<'a, 'tcx> ty::TypeFolder<ty::TyCtxt<'tcx>> for Generalize<'a, 'tcx> {
    fn cx(&self) -> ty::TyCtxt<'tcx> {
        self.infcx.tcx
    }

    fn fold_binder<T: TypeFoldable<ty::TyCtxt<'tcx>>>(
        &mut self,
        t: ty::Binder<'tcx, T>,
    ) -> ty::Binder<'tcx, T> {
        self.depth.shift_in(1);
        let t = t.super_fold_with(self);
        self.depth.shift_out(1);
        t
    }

    fn fold_ty(&mut self, t: ty::Ty<'tcx>) -> ty::Ty<'tcx> {
        match *t.kind() {
            ty::TyKind::Infer(ty::InferTy::TyVar(vid)) => {
                match self.infcx.shallow_resolve_ty_var_or_get_root(vid) {
                    Ok(t) => t.fold_with(self),
                    Err(root) => {
                        let (tcx, params) = (self.infcx.tcx, &mut self.params);
                        *self.ty_roots.entry(root).or_insert_with(|| {
                            let param = param_ty(tcx, params.len() as u32);
                            params.push(param.into());
                            param
                        })
                    }
                }
            }
            ty::TyKind::Infer(_) | ty::TyKind::Placeholder(_) => {
                self.failed = true;
                t
            }
            _ => t.super_fold_with(self),
        }
    }

    fn fold_region(&mut self, r: ty::Region<'tcx>) -> ty::Region<'tcx> {
        match r.kind() {
            ty::ReBound(..) => r,
            ty::RePlaceholder(p) if p.universe == self.placeholder_universe => {
                let var = ty::BoundVar::from_u32(self.extra_vars + p.bound.var.as_u32());
                let br = ty::BoundRegion {
                    var,
                    kind: ty::BoundRegionKind::Anon,
                };
                ty::Region::new_bound(self.infcx.tcx, self.depth, br)
            }
            ty::RePlaceholder(_) => {
                self.failed = true;
                r
            }
            _ => self.infcx.tcx.lifetimes.re_erased,
        }
    }

    fn fold_const(&mut self, c: ty::Const<'tcx>) -> ty::Const<'tcx> {
        let ty::ConstKind::Infer(infer) = c.kind() else {
            return c.super_fold_with(self);
        };
        let ty::InferConst::Var(vid) = infer else {
            self.failed = true;
            return c;
        };
        let root = self.infcx.root_const_var(vid);
        if let Some(&param) = self.const_roots.get(&root) {
            return param;
        }
        let Some(&ty) = self.const_tys.get(&root) else {
            self.failed = true;
            return c;
        };
        let param = param_const(self.infcx.tcx, self.params.len() as u32);
        self.params.push(param.into());
        self.const_roots.insert(root, param);
        let has_type = ty::ClauseKind::ConstArgHasType(param, ty);
        self.const_clauses
            .push(ty::Binder::dummy(has_type).upcast(self.infcx.tcx));
        param
    }
}
