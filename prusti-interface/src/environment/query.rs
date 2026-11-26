use std::fmt::Debug;

use crate::data::ProcedureDefId;
use log::debug;
use prusti_rustc_interface::{
    hir::{Attribute, HirId},
    infer::infer::InferCtxt,
    middle::ty::{
        self, GenericArgsRef, PredicatePolarity, TraitPredicate, TyCtxt, TypeVisitableExt,
    },
    span::{
        def_id::{DefId, LocalDefId},
        Span,
    },
    trait_selection::{
        infer::TyCtxtInferExt,
        traits::{
            query::evaluate_obligation::InferCtxtExt, ImplSource, Obligation, ObligationCause,
            SelectionContext,
        },
    },
};
use sealed::{IntoParam, IntoParamTcx};

#[derive(Copy, Clone)]
pub struct EnvQuery<'tcx> {
    tcx: TyCtxt<'tcx>,
}

impl<'tcx> EnvQuery<'tcx> {
    pub fn new(tcx: TyCtxt<'tcx>) -> Self {
        EnvQuery { tcx }
    }

    /// Returns the typing context
    pub(crate) fn tcx(self) -> TyCtxt<'tcx> {
        self.tcx
    }

    fn infer_ctxt(self) -> InferCtxt<'tcx> {
        self.tcx.infer_ctxt().build(ty::TypingMode::PostAnalysis)
    }

    /// Convert HirId to LocalDefId (see `local_def_id` in hir Map)
    pub fn as_local_def_id(self, def_id: HirId) -> LocalDefId {
        IntoParamTcx::into_param(def_id, self.tcx)
    }

    /// Convert LocalDefId to HirId (see `local_def_id_to_hir_id` in hir Map)
    pub fn as_hir_id(self, def_id: LocalDefId) -> HirId {
        IntoParamTcx::into_param(def_id, self.tcx)
    }

    pub fn get_local_attributes(self, def_id: impl IntoParamTcx<'tcx, HirId>) -> &'tcx [Attribute] {
        self.tcx.hir_attrs(def_id.into_param(self.tcx))
    }

    pub fn get_attributes(self, def_id: impl IntoParam<ProcedureDefId>) -> &'tcx [Attribute] {
        let def_id = def_id.into_param();
        if let Some(local_def_id) = def_id.as_local() {
            self.get_local_attributes(local_def_id)
        } else {
            self.tcx.attrs_for_def(def_id)
        }
    }

    /// Get the span of the given definition.
    pub fn get_def_span(self, def_id: impl IntoParam<DefId>) -> Span {
        self.tcx.def_span(def_id.into_param())
    }

    /// Returns true iff `def_id` has an MIR body which we may want to access
    pub fn has_body(self, def_id: impl IntoParam<DefId>) -> bool {
        let id = def_id.into_param();
        self.tcx.is_mir_available(id) && !self.tcx.is_constructor(id)
    }

    /// If the given DefId describes an item belonging to a trait, returns the DefId
    /// of the trait that the trait item belongs to; otherwise, returns None.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn get_trait_of_assoc(
        self,
        def_id: impl IntoParam<ProcedureDefId> + Debug,
    ) -> Option<DefId> {
        self.tcx.trait_of_assoc(def_id.into_param())
    }

    /// Returns true iff `def_id` is an implementation of a trait method
    pub fn is_trait_method_impl(self, def_id: impl IntoParam<ProcedureDefId>) -> bool {
        self.tcx
            .impl_of_assoc(def_id.into_param())
            .and_then(|impl_id| self.tcx.impl_opt_trait_id(impl_id))
            .is_some()
    }

    /// Given some trait item `proc_def_id` which is called with `substs`
    /// (under the caller's `typing_env`), returns the `DefId` of the trait
    /// `impl` block that the call selects. Returns `None` if `proc_def_id` is
    /// no trait item or selection does not yield a user-defined impl (e.g.
    /// the `Self` type is generic).
    #[tracing::instrument(level = "debug", skip(self, typing_env))]
    pub fn find_trait_impl_of_method_call(
        self,
        typing_env: ty::TypingEnv<'tcx>,
        proc_def_id: impl IntoParam<ProcedureDefId> + Debug,
        substs: GenericArgsRef<'tcx>,
    ) -> Option<DefId> {
        let proc_def_id = proc_def_id.into_param();
        let trait_id = self.get_trait_of_assoc(proc_def_id)?;
        debug!(
            "Fetching implementations of method '{:?}' defined in trait '{}' with substs '{:?}'",
            proc_def_id,
            self.tcx.def_path_str(trait_id),
            substs
        );
        // Selection runs in a fresh inference context: region variables from
        // the caller's context (e.g. borrowck'd MIR) do not affect which impl
        // matches and must be erased, while type/const inference variables
        // would be dangling indices - a caller bug.
        assert!(
            !substs.has_non_region_infer(),
            "trait selection on substs with inference variables: {substs:?}"
        );
        let substs = self.tcx.erase_and_anonymize_regions(substs);
        let infcx = self.infer_ctxt();
        let mut sc = SelectionContext::new(&infcx);
        let trait_ref = ty::TraitRef::new(self.tcx, trait_id, substs);
        let obligation = Obligation::new(
            self.tcx,
            ObligationCause::dummy(),
            typing_env.param_env,
            TraitPredicate {
                trait_ref,
                polarity: PredicatePolarity::Positive,
            },
        );
        let result = sc.select(&obligation);
        match result {
            Ok(Some(ImplSource::UserDefined(data))) => Some(data.impl_def_id),
            _ => None,
        }
    }

    /// Returns true iff the trait solver proves that `clause` holds in
    /// `param_env`. Region constraints that remain (e.g. from instantiating a
    /// higher-ranked bound) are not decided, so the result is only meaningful
    /// for clauses whose regions do not matter, such as erased ones.
    pub fn clause_holds_modulo_regions(
        self,
        param_env: ty::ParamEnv<'tcx>,
        clause: ty::Clause<'tcx>,
    ) -> bool {
        let obligation = Obligation::new(self.tcx, ObligationCause::dummy(), param_env, clause);
        self.infer_ctxt()
            .predicate_must_hold_modulo_regions(&obligation)
    }

    /// Given the `called_def_id` of a function, called from `caller_def_id`,
    /// and some substitutions `call_substs` applied for that call, returns
    /// `true` if the call is to a function in the specified crate
    /// `crate_name`. A trait method call is attributed to the crate of the
    /// `impl` it relies on (e.g. `PartialOrd::le` on `Int` belongs to
    /// `prusti_contracts`, even though the default `le` body lives in
    /// `core`); other calls, and trait calls whose impl is unknown (e.g. a
    /// generic `Self`), to the crate of the called definition itself.
    pub fn is_function_in_crate(
        self,
        caller_def_id: impl IntoParam<ProcedureDefId>, // where are we calling from?
        called_def_id: impl IntoParam<ProcedureDefId>, // what are we calling?
        call_substs: GenericArgsRef<'tcx>,
        crate_name: &'tcx str,
    ) -> bool {
        let called_def_id = called_def_id.into_param();
        let typing_env = ty::TypingEnv::post_analysis(self.tcx, caller_def_id.into_param());
        let def_id = self
            .find_trait_impl_of_method_call(typing_env, called_def_id, call_substs)
            .unwrap_or(called_def_id);
        self.tcx.crate_name(def_id.krate).as_str() == crate_name
    }

    /// Given an adt definition (adt), this method returns if the adt is defined in the crate specified (crate_name)
    pub fn is_adt_in_crate(self, adt: ty::AdtDef<'tcx>, crate_name: &'tcx str) -> bool {
        let did_crate_name = self.tcx.crate_name(adt.did().krate);
        did_crate_name.as_str() == crate_name
    }

    /// Given a call to `called_def_id` from within `caller_def_id`, returns
    /// the `DefId` that will actually be called if known (i.e. if a trait
    /// method call actually resolves to a concrete implementation), as well as
    /// the correct substitutions for that call. If a method is not resolved,
    /// returns the original `called_def_id` and `call_substs`.
    pub fn resolve_method_call(
        self,
        caller_def_id: impl IntoParam<ProcedureDefId>, // where are we calling from?
        called_def_id: impl IntoParam<ProcedureDefId>, // what are we calling?
        call_substs: GenericArgsRef<'tcx>,
    ) -> (ProcedureDefId, GenericArgsRef<'tcx>) {
        let called_def_id = called_def_id.into_param();

        (|| {
            let typing_env = ty::TypingEnv::post_analysis(self.tcx, caller_def_id.into_param());
            let instance =
                ty::Instance::try_resolve(self.tcx, typing_env, called_def_id, call_substs)
                    .ok()??;
            let resolved_def_id = instance.def_id();
            let resolved_substs = if resolved_def_id == called_def_id {
                // if no trait resolution occurred, we can keep the non-erased substs
                call_substs
            } else {
                instance.args
            };

            Some((resolved_def_id, resolved_substs))
        })()
        .unwrap_or((called_def_id, call_substs))
    }

    /// Return the default substitutions for a particular item, i.e. where each
    /// generic maps to itself.
    pub fn identity_substs(self, def_id: impl IntoParam<ProcedureDefId>) -> GenericArgsRef<'tcx> {
        ty::List::identity_for_item(self.tcx, def_id.into_param())
    }
}

mod sealed {
    use prusti_rustc_interface::{
        hir::{HirId, OwnerId},
        middle::ty::TyCtxt,
        span::def_id::{DefId, LocalDefId},
    };

    pub trait IntoParam<P> {
        fn into_param(self) -> P;
    }
    impl<P> IntoParam<P> for P {
        #[inline(always)]
        fn into_param(self) -> P {
            self
        }
    }
    impl<'a, P: Copy> IntoParam<P> for &'a P {
        #[inline(always)]
        fn into_param(self) -> P {
            *self
        }
    }
    impl IntoParam<DefId> for LocalDefId {
        #[inline(always)]
        fn into_param(self) -> DefId {
            self.to_def_id()
        }
    }
    impl IntoParam<DefId> for OwnerId {
        #[inline(always)]
        fn into_param(self) -> DefId {
            self.to_def_id()
        }
    }
    impl IntoParam<LocalDefId> for OwnerId {
        #[inline(always)]
        fn into_param(self) -> LocalDefId {
            self.def_id
        }
    }

    pub trait IntoParamTcx<'tcx, P> {
        fn into_param(self, tcx: TyCtxt<'tcx>) -> P;
    }
    impl<'tcx, T: IntoParam<U>, U> IntoParamTcx<'tcx, U> for T {
        #[inline(always)]
        fn into_param(self, _tcx: TyCtxt<'tcx>) -> U {
            self.into_param()
        }
    }
    impl<'tcx> IntoParamTcx<'tcx, HirId> for OwnerId {
        #[inline(always)]
        fn into_param(self, tcx: TyCtxt<'tcx>) -> HirId {
            tcx.local_def_id_to_hir_id(self.def_id)
        }
    }
    impl<'tcx> IntoParamTcx<'tcx, HirId> for LocalDefId {
        #[inline(always)]
        fn into_param(self, tcx: TyCtxt<'tcx>) -> HirId {
            tcx.local_def_id_to_hir_id(self)
        }
    }
    impl<'tcx> IntoParamTcx<'tcx, LocalDefId> for HirId {
        #[inline(always)]
        fn into_param(self, _tcx: TyCtxt<'tcx>) -> LocalDefId {
            self.owner.def_id
        }
    }
}
