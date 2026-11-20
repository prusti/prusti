mod method;
mod function;
mod signature;
mod spec_blocks;

pub use function::*;
pub use method::*;
pub use signature::*;
pub use spec_blocks::*;

use crate::encoders::ty::generics::{
    GArgs, GParams,
    r#trait::TraitEnc,
    trait_fn::{TraitFnEnc, TraitFnEncOutputRef},
    trait_impls::TraitImplEnc,
};

use prusti_interface::{PrustiError, specs::specifications::SpecQuery};
use prusti_rustc_interface::{
    hir,
    middle::ty,
    span::{DUMMY_SP, Span, def_id::DefId},
};
use task_encoder::{
    DependencyErrorLink, EncodeFullError, ErrorKind, TaskEncoder, TaskEncoderDependencies,
};

/// The cause of an encoding error, reported when a function or method body, a
/// contract or one of its specs cannot be encoded (e.g. an unsupported
/// feature): the link of the error's chain that holds the actual error, and
/// the innermost span of the chain.
pub(crate) struct DepError {
    pub cause: DependencyErrorLink,
    pub span: Option<Span>,
}

impl DepError {
    /// The error to report at `span` when `what` cannot be encoded. A mistake
    /// in the user's program is reported as an invalid-specification error
    /// stating only the cause; a failure caused by an internal error (e.g. a
    /// panic) as an internal error, which, unlike an unsupported feature, is
    /// not downgraded to a warning when unsupported features are skipped;
    /// anything else is an unsupported-feature error. The latter two also
    /// name `what`.
    pub(crate) fn report(self, what: String, span: Span) -> PrustiError {
        let cause = self.cause;
        let error = match cause.kind {
            ErrorKind::Incorrect => PrustiError::incorrect(cause.message, span.into()),
            ErrorKind::Internal => PrustiError::internal(
                format!("cannot encode {what}: {}", cause.message),
                span.into(),
            ),
            ErrorKind::Unsupported => PrustiError::unsupported(
                format!("cannot encode {what}: {}", cause.message),
                span.into(),
            ),
        };
        match cause.help {
            Some(help) => error.set_help(help),
            None => error,
        }
    }
}

/// Reports the encoding error `err` with the diagnostic that `report` builds
/// from its cause and returns the proof that it was reported. If `err` was
/// already reported, reports nothing and returns that report's proof.
pub(crate) fn report_dep_error<'vir, E: TaskEncoder + ?Sized>(
    vcx: &'vir vir::VirCtxt<'vir>,
    err: &EncodeFullError<'vir, E>,
    report: impl FnOnce(DepError) -> PrustiError,
) -> vir::Reported {
    match err {
        EncodeFullError::Reported(reported) => *reported,
        err => {
            let reported = vcx.emit_early_error(report(dep_error(err)));
            err.mark_reported(reported);
            reported
        }
    }
}

/// Extracts the cause of an encoding error. For a dependency error, the cause
/// is the root (the last link of the chain), which holds the actual error.
fn dep_error<'vir, E: TaskEncoder + ?Sized>(err: &EncodeFullError<'vir, E>) -> DepError {
    match err {
        EncodeFullError::DependencyError(chain) => DepError {
            cause: chain.last().cloned().unwrap_or_else(|| {
                DependencyErrorLink::new(
                    E::ENCODER_NAME,
                    "encoding dependency error".to_string(),
                    Vec::new(),
                    ErrorKind::Unsupported,
                )
            }),
            span: chain
                .iter()
                .rev()
                .find_map(|link| link.spans.first().copied()),
        },
        other => DepError {
            cause: DependencyErrorLink::new(
                E::ENCODER_NAME,
                format!("{other:?}"),
                Vec::new(),
                ErrorKind::Unsupported,
            ),
            span: None,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallTaskDescription<'tcx> {
    gargs: GArgs<'tcx>,
    callee: DefId,
    resolve_trait_calls: bool,
}

impl<'tcx> CallTaskDescription<'tcx> {
    pub fn new(
        context: impl Into<GParams<'tcx>>,
        args: &'tcx [ty::GenericArg<'tcx>],
        callee: DefId,
    ) -> Self {
        Self {
            gargs: GArgs::new(context, args),
            callee,
            resolve_trait_calls: true,
        }
    }

    pub fn resolve_trait_calls(self, resolve_trait_calls: bool) -> Self {
        Self {
            resolve_trait_calls,
            ..self
        }
    }

    fn trait_call<'vir, E: TaskEncoder>(
        &self,
        deps: &mut TaskEncoderDependencies<'vir, E>,
    ) -> Result<(DefId, Option<TraitFnEncOutputRef<'vir>>), EncodeFullError<'vir, E>> {
        let tcx = vir::with_vcx(|vcx| vcx.tcx());
        if self.resolve_trait_calls
            && let Some(assoc_item) = tcx.opt_associated_item(self.callee)
            && assoc_item.trait_container(tcx).is_some()
        {
            let assoc_enc = deps.require_ref::<TraitFnEnc>(assoc_item.def_id)?;
            Ok((assoc_item.def_id, Some(assoc_enc)))
        } else {
            Ok((self.callee, None))
        }
    }
}

// TODO: this should be a "crate" encoder, which will deps.require all the methods in the crate
pub fn encode_all_in_crate<'tcx>(tcx: ty::TyCtxt<'tcx>) {
    for def_id in tcx.hir_body_owners() {
        tracing::debug!("test_entrypoint item: {def_id:?}");
        match tcx.def_kind(def_id) {
            // Closure bodies are verified like `fn` bodies, whether or not
            // they carry a `closure!` specification; only the closures of
            // specifications themselves are exempt.
            hir::def::DefKind::Fn | hir::def::DefKind::AssocFn | hir::def::DefKind::Closure => {
                let def_id = def_id.to_def_id();
                if prusti_interface::specs::is_spec_item(tcx, def_id) {
                    continue;
                }
                // A closure inside a trusted function is part of its
                // (unencoded) body.
                let root = tcx.typeck_root_def_id(def_id);
                if root != def_id && crate::encoders::is_function_trusted(root) {
                    continue;
                }
                // Extern-spec stubs are macro-generated forwarding bodies whose
                // spec is transplanted onto the foreign target: verifying the
                // stub against it would be circular, and by this point the stub
                // has no spec of its own left to verify against.
                if prusti_interface::utils::get_prusti_attr_of(tcx, def_id, "extern_spec").is_some()
                {
                    continue;
                }

                let (is_pure, is_trusted) = crate::encoders::with_proc_spec(
                    SpecQuery::GetProcKind(def_id, ty::List::identity_for_item(tcx, def_id)),
                    |proc_spec| {
                        // Report an invalid trait-to-impl kind refinement once,
                        // here, rather than on every purity query.
                        crate::encoders::report_kind_refinement_error(def_id, &proc_spec.kind);
                        let is_pure = crate::encoders::kind_is_pure(&proc_spec.kind);
                        let is_trusted = crate::encoders::spec_is_trusted(proc_spec, def_id);
                        (is_pure, is_trusted)
                    },
                )
                .unwrap_or_default();

                if !(is_trusted && is_pure) {
                    let _ = method::MethodEnc::encode(def_id, false, DUMMY_SP);
                }
            }
            unsupported_item_kind => {
                tracing::debug!("unsupported item: {unsupported_item_kind:?}");
            }
        }
    }

    // This creates the impl encoding for all traits in the crate
    // To iterate over all _visible_ impl blocks,
    // use tcx.visible_traits and tcx.all_impls(trait_id)
    for def_id in tcx.hir_crate_items(()).definitions() {
        // A failed encoding stays in the encoder's cache, from which its
        // `emit_outputs` reports it at the first non-dummy span of the item's
        // requests.
        match tcx.def_kind(def_id) {
            hir::def::DefKind::Trait => {
                let _ = TraitEnc::encode(def_id.to_def_id(), false, tcx.def_span(def_id));
            }
            hir::def::DefKind::Impl { of_trait: true } => {
                let _ = TraitImplEnc::encode(def_id.to_def_id(), false, tcx.def_span(def_id));
            }
            _ => (),
        }
    }
}
