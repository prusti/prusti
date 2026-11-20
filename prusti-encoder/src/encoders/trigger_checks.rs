//! The checks that a quantifier's triggers are valid, which report the
//! problems `vir::check_trigger_set` finds in the encoded terms (see
//! `PureKind::Trigger` for how they are encoded). Rust's operators on
//! primitive types are encoded as applications of functions, so a trigger may
//! use them; the snapshot equality `===` and the operations of `Int` and
//! `Real` are encoded as Viper's own operators, so it may not.

use crate::encoders::{MirPureEnc, mir_shared::EncodeResult};
use prusti_rustc_interface::{
    middle::{mir, ty},
    span::{Span, Symbol, def_id::DefId},
};
use task_encoder::EncodeFullError;
use vir::TriggerSetError;

/// What the outermost operation of a trigger can be.
const HEADS: &str = "a call, an operator, a constructor, a field access, an indexing or a cast";

/// Checks that the outermost operation of the trigger that the closure
/// `body` computes is one the user writes (see [`HEADS`]), reported at
/// `span`. The simplifier folds away the borrows and dereferences that rustc
/// and the encoding insert, but no application of what the user writes (see
/// `task_encoder::Program::add_user_adt`), so only the latter is sure to
/// remain a term the trigger can match. Dereferences are looked through:
/// rustc inserts them for reads through references and for overloaded
/// indexing, a call to `Index::index` whose result it dereferences.
pub(super) fn check_head<'vir>(
    body: &mir::Body<'vir>,
    span: Span,
) -> EncodeResult<'vir, (), MirPureEnc> {
    let error = |message: String| Err(EncodeFullError::incorrect(message, vec![span]));
    let variable = || {
        error(format!(
            "a trigger must be {HEADS}, not a bare variable or constant"
        ))
    };
    let mut place = mir::Place::from(mir::RETURN_PLACE);
    loop {
        let mut projection = place.projection.as_slice();
        while let [rest @ .., mir::ProjectionElem::Deref] = projection {
            projection = rest;
        }
        match projection {
            // A captured place (a variable, or e.g. the field of a variable
            // read by value), a field of the closure.
            [mir::ProjectionElem::Deref, mir::ProjectionElem::Field(..)]
                if place.local == ty::CAPTURE_STRUCT_LOCAL =>
            {
                return error(format!(
                    "a trigger must be {HEADS}, not a value from outside the quantifier"
                ));
            }
            [.., mir::ProjectionElem::Deref] => unreachable!("dereferences are stripped"),
            [..] if !projection.is_empty() => return Ok(()),
            [..] => {}
        }
        let Some(definition) = definition(body, place.local) else {
            // An argument, the closure itself or a bound variable, or else a
            // local after a call that never returns, which the encoding
            // reports.
            return if (1..=body.arg_count).contains(&place.local.as_usize()) {
                variable()
            } else {
                Ok(())
            };
        };
        let Definition::Assign(rvalue) = definition else {
            // A call, or a local assigned in several places, which is a
            // conditional the encoding reports.
            return Ok(());
        };
        place = match rvalue {
            mir::Rvalue::Use(mir::Operand::Copy(p) | mir::Operand::Move(p), _)
            | mir::Rvalue::CopyForDeref(p) => *p,
            // A reborrow `&*x` is inserted by rustc.
            mir::Rvalue::Ref(_, _, p)
                if p.projection.as_slice() == [mir::ProjectionElem::Deref] =>
            {
                mir::Place::from(p.local)
            }
            mir::Rvalue::Cast(
                mir::CastKind::PointerCoercion(..) | mir::CastKind::Transmute,
                mir::Operand::Copy(p) | mir::Operand::Move(p),
                _,
            ) => *p,
            mir::Rvalue::Use(mir::Operand::Constant(_) | mir::Operand::RuntimeChecks(_), _) => {
                return variable();
            }
            mir::Rvalue::Ref(..) | mir::Rvalue::RawPtr(..) => {
                return error(format!(
                    "a trigger cannot be a borrow: it must be {HEADS} (a borrow can be the \
                     argument of a call)"
                ));
            }
            // Rustc and Prusti build tuples and closures themselves (e.g.
            // the arguments of a call to a closure), so the simplifier folds
            // their constructors away too.
            mir::Rvalue::Aggregate(
                box (mir::AggregateKind::Tuple | mir::AggregateKind::Closure(..)),
                _,
            ) => {
                return error(format!(
                    "a trigger cannot be a tuple or a closure: it must be {HEADS}"
                ));
            }
            _ => return Ok(()),
        };
    }
}

/// How a local of a trigger closure's body gets its value.
enum Definition<'a, 'tcx> {
    Assign(&'a mir::Rvalue<'tcx>),
    Call,
    /// Assigned in several places.
    Several,
}

/// How the local `local` of `body` gets its value, `None` for an argument.
fn definition<'a, 'tcx>(
    body: &'a mir::Body<'tcx>,
    local: mir::Local,
) -> Option<Definition<'a, 'tcx>> {
    let mut found = None;
    let mut record = |definition| {
        found = Some(match found {
            None => definition,
            Some(_) => Definition::Several,
        });
    };
    for data in body.basic_blocks.iter() {
        for statement in &data.statements {
            if let mir::StatementKind::Assign(box (dest, rvalue)) = &statement.kind
                && dest.as_local() == Some(local)
            {
                record(Definition::Assign(rvalue));
            }
        }
        if let mir::TerminatorKind::Call { destination, .. } = &data.terminator().kind
            && destination.as_local() == Some(local)
        {
            record(Definition::Call);
        }
    }
    found
}

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
            "a trigger set must contain at least one expression".to_string(),
            vec![span],
        ));
    };
    let names = qvars.iter().map(|qvar| qvar.name).collect::<Vec<_>>();
    let Err(error) = vir::check_trigger_set(&names, shapes) else {
        return Ok(());
    };
    let term_span = |idx: usize| vcx.tcx().def_span(closures[idx]);
    let (message, span) = match error {
        TriggerSetError::Head(idx) => (
            format!("a trigger must be {HEADS}, not a bare variable or constant"),
            term_span(idx),
        ),
        TriggerSetError::Forbidden(idx) => (
            "a trigger cannot contain the snapshot equality `===` or an operation on Prusti's \
             mathematical integers and reals (`Int`, `Real`)"
                .to_string(),
            term_span(idx),
        ),
        TriggerSetError::NoQuantifiedVariable(idx) => (
            "every expression of a trigger set must mention a bound variable of the quantifier"
                .to_string(),
            term_span(idx),
        ),
        TriggerSetError::MissingQuantifiedVariable(missing) => (
            match bound_name(vcx, first, missing) {
                Some(name) => {
                    format!("this trigger set does not mention the bound variable `{name}`")
                }
                None => "this trigger set does not mention every bound variable of the quantifier"
                    .to_string(),
            },
            term_span(0).to(term_span(closures.len() - 1)),
        ),
    };
    Err(EncodeFullError::incorrect(message, vec![span]))
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
