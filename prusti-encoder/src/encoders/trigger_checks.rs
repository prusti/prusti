//! The checks that a quantifier's triggers are valid, which report the
//! problems `vir::check_trigger_set` finds in the encoded terms (see
//! `PureKind::Trigger` for how they are encoded). Rust's operators on
//! primitive types are encoded as applications of functions, so a trigger may
//! use them; the snapshot equality `===` and the operations of ghost types
//! such as `Int` are encoded as Viper's own operators, so it may not.

use crate::encoders::{MirPureEnc, mir_shared::EncodeResult};
use prusti_rustc_interface::span::{Span, Symbol, def_id::DefId};
use task_encoder::EncodeFullError;
use vir::TriggerSetError;

/// Checks a trigger set of a quantifier over `qvars`, whose terms are the
/// bodies of the closures `closures`, with the shapes `shapes`. `span` is
/// where to report an empty set.
pub(super) fn check_set<'vir>(
    vcx: &'vir vir::VirCtxt<'vir>,
    qvars: &[vir::LocalDeclSnap<'vir>],
    closures: &[DefId],
    shapes: &[vir::TriggerTermShape<'vir>],
    span: Span,
) -> EncodeResult<'vir, (), MirPureEnc> {
    let Some(&first) = closures.first() else {
        return Err(EncodeFullError::incorrect(
            "a trigger set must contain at least one term".to_string(),
            span,
        ));
    };
    let names = qvars.iter().map(|qvar| qvar.name).collect::<Vec<_>>();
    let Err(error) = vir::check_trigger_set(&names, shapes) else {
        return Ok(());
    };
    let term_span = |idx: usize| vcx.tcx().def_span(closures[idx]);
    let (message, span) = match error {
        TriggerSetError::Head(idx) => (
            "a trigger must be a function or method call, an operator, a field access or an \
             indexing"
                .to_string(),
            term_span(idx),
        ),
        TriggerSetError::Forbidden(idx) => (
            "a trigger cannot contain `===` or an operation on a ghost type such as `Int`, \
             which are encoded as operators that Viper does not allow in triggers"
                .to_string(),
            term_span(idx),
        ),
        TriggerSetError::NoQuantifiedVariable(idx) => (
            "every term of a trigger set must mention a variable bound by its quantifier"
                .to_string(),
            term_span(idx),
        ),
        TriggerSetError::MissingQuantifiedVariable(missing) => (
            match bound_name(vcx, first, missing) {
                Some(name) => {
                    format!("this trigger set does not mention the quantified variable `{name}`")
                }
                None => "this trigger set does not mention every quantified variable".to_string(),
            },
            term_span(0).to(term_span(closures.len() - 1)),
        ),
    };
    Err(EncodeFullError::incorrect(message, span))
}

/// The name of the quantified variable at position `idx` of the trigger
/// closure `cl_def_id`, for error messages. Crate metadata records the
/// parameter names of functions only, so a closure of another crate has none.
fn bound_name<'vir>(vcx: &'vir vir::VirCtxt<'vir>, cl_def_id: DefId, idx: usize) -> Option<Symbol> {
    if !cl_def_id.is_local() {
        return None;
    }
    let ident = vcx
        .tcx()
        .fn_arg_idents(cl_def_id)
        .get(idx)
        .copied()
        .flatten()?;
    Some(ident.name)
}
