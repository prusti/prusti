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
use task_encoder::{EncodeFullError, TaskEncoder, TaskEncoderDependencies};

/// The cause of an encoding error, reported when a function or method body or
/// a contract cannot be encoded (e.g. an unsupported feature) and an abstract
/// stub is used instead.
pub(crate) struct DepError {
    pub message: String,
    pub help: Option<String>,
    pub span: Option<Span>,
    pub incorrect: bool,
}

impl DepError {
    /// The error to report at `span` when `what` cannot be encoded. A mistake
    /// in the user's program is reported as an invalid-specification error
    /// stating only the cause; anything else is an unsupported-feature error
    /// that also names `what`.
    pub(crate) fn report(self, what: String, span: Span) -> PrustiError {
        let error = if self.incorrect {
            PrustiError::incorrect(self.message, span.into())
        } else {
            PrustiError::unsupported(
                format!("cannot encode {what}: {}", self.message),
                span.into(),
            )
        };
        match self.help {
            Some(help) => error.set_help(help),
            None => error,
        }
    }
}

/// Extracts the cause of an encoding error. For a dependency error, the
/// message, the help and whether the user's program is at fault come from the
/// root cause (the last link of the chain), which holds the actual error, and
/// the span is the innermost one in the chain.
pub(crate) fn dep_error<'vir, E: TaskEncoder + ?Sized>(err: &EncodeFullError<'vir, E>) -> DepError {
    match err {
        EncodeFullError::DependencyError(chain) => {
            let root = chain.last();
            DepError {
                message: root.map_or_else(
                    || "encoding dependency error".to_string(),
                    |link| link.message.clone(),
                ),
                help: root.and_then(|link| link.help.clone()),
                span: chain
                    .iter()
                    .rev()
                    .find_map(|link| link.spans.first().copied()),
                incorrect: root.is_some_and(|link| link.incorrect),
            }
        }
        other => DepError {
            message: format!("{other:?}"),
            help: None,
            span: None,
            incorrect: false,
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
                // A closure body implements the closure's built-in `Fn*`
                // impls, which `TraitImplEnc` checks.
                if tcx.type_of(def_id).skip_binder().is_closure() {
                    let _ = TraitImplEnc::encode(def_id, false, DUMMY_SP);
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
        match tcx.def_kind(def_id) {
            hir::def::DefKind::Trait => {
                TraitEnc::encode(def_id.to_def_id(), false, DUMMY_SP).unwrap();
            }
            hir::def::DefKind::Impl { of_trait: true } => {
                TraitImplEnc::encode(def_id.to_def_id(), false, DUMMY_SP).unwrap();
            }
            _ => (),
        }
    }
}
