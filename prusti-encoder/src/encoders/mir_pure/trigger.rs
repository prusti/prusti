//! The triggers of quantifiers (see `PureKind::Trigger` for how their terms
//! are encoded), and the checks that they are valid. The encoding of a
//! trigger closure rejects the conditionals and the calls that never return
//! in it (`Enc::encode_cfg`), and its quantifiers, specification blocks and
//! states other than `old` (`Enc::encode_pure_only_builtin`). The encoded
//! trigger is then checked for Viper's own operators and for its outermost
//! operation (`Enc::check_trigger`), and each trigger set for the bound
//! variables its triggers mention (`Enc::encode_trigger_sets`).
//!
//! Rust's operators on primitive types are encoded as applications of
//! functions, so a trigger may use them; the snapshot equality `===` and the
//! operations of `Int` and `Real` are encoded as Viper's own operators, so it
//! may not.

use super::*;
use prusti_rustc_interface::{
    middle::{
        middle::resolve_bound_vars::Set1,
        mir::visit::{PlaceContext, Visitor},
    },
    span::Symbol,
};

/// What the outermost operation of a trigger can be.
const HEADS: &str = "a call, an operator, a constructor, a field access, an indexing or a cast";

impl<'vir: 'enc, 'enc> Enc<'vir, 'enc> {
    /// Encodes the trigger sets of a quantifier builtin over `qvars`, whose
    /// type `sets_ty` is a tuple of trigger sets, each itself a tuple of
    /// references to spec closures whose arguments are the quantified
    /// variables and whose body is one trigger expression. Like the body
    /// closure, the trigger closures get their captures from their
    /// constructions among `closures` (see [`Self::encode_spec_closure`]).
    /// Reports a set that is empty, an expression that reads no quantified
    /// variable, and a set whose expressions do not read every quantified
    /// variable; the rest of each expression is checked where it is encoded
    /// (see [`Self::check_trigger`]).
    pub(super) fn encode_trigger_sets(
        &mut self,
        name: &str,
        sets_ty: ty::Ty<'vir>,
        qvars: &'vir [vir::LocalDeclSnap<'vir>],
        closures: &FxHashMap<DefId, ClosureConstruction<'vir>>,
    ) -> EncodeResult<'vir, &'vir [TriggerRet<'vir>], MirPureEnc> {
        let TyKind::Tuple(set_tys) = sets_ty.kind() else {
            panic!("illegal prusti::{name}: expected a trigger-set tuple, got {sets_ty:?}")
        };
        let mut sets = Vec::with_capacity(set_tys.len());
        for set_ty in set_tys.iter() {
            let TyKind::Tuple(cl_tys) = set_ty.kind() else {
                panic!("illegal prusti::{name}: expected a trigger tuple, got {set_ty:?}")
            };
            let mut exprs = Vec::with_capacity(cl_tys.len());
            let mut def_ids = Vec::with_capacity(cl_tys.len());
            let mut mentioned = vec![false; qvars.len()];
            for cl_ty in cl_tys.iter() {
                let &TyKind::Ref(_, cl_ty, _) = cl_ty.kind() else {
                    panic!("illegal prusti::{name}: expected a trigger closure, got {cl_ty:?}")
                };
                let &TyKind::Closure(cl_def_id, cl_args) = cl_ty.kind() else {
                    panic!("illegal prusti::{name}: expected a trigger closure, got {cl_ty:?}")
                };
                let cl_span = self.vcx.tcx().def_span(cl_def_id);
                let kind = PureKind::Trigger {
                    context: self.context_def_id,
                };
                let body =
                    self.encode_closure_body(name, kind, cl_def_id, cl_args, qvars, closures)?;
                if !body.reads.contains(&true) {
                    return Err(EncodeFullError::incorrect(
                        "every expression of a trigger set must mention a bound variable of the \
                         quantifier"
                            .to_string(),
                        vec![cl_span],
                    ));
                }
                for (mentioned, &read) in mentioned.iter_mut().zip(body.reads) {
                    *mentioned |= read;
                }
                // Created under the trigger's span, so that the reified term
                // carries it and the verifier's warnings about the trigger
                // point at it.
                let trigger = self.vcx.with_span(cl_span, |vcx| {
                    body.embed(vcx, vir::vir_format!(vcx, "trigger ({name})"))
                });
                exprs.push(trigger);
                def_ids.push(cl_def_id);
            }
            let (Some(&first), Some(&last)) = (def_ids.first(), def_ids.last()) else {
                return Err(EncodeFullError::incorrect(
                    "a trigger set must contain at least one expression".to_string(),
                    vec![self.current_span()],
                ));
            };
            if let Some(missing) = mentioned.iter().position(|&mentioned| !mentioned) {
                let message = match bound_name(self.vcx, first, missing) {
                    Some(name) => {
                        format!("this trigger set does not mention the bound variable `{name}`")
                    }
                    None => "this trigger set does not mention every bound variable of the \
                             quantifier"
                        .to_string(),
                };
                let span = self
                    .vcx
                    .tcx()
                    .def_span(first)
                    .to(self.vcx.tcx().def_span(last));
                return Err(EncodeFullError::incorrect(message, vec![span]));
            }
            sets.push(self.vcx.mk_trigger(&exprs));
        }
        Ok(self.vcx.alloc_slice(&sets))
    }

    /// Checks that `expr`, the encoding of this trigger closure's body, is a
    /// valid trigger term. Its outermost operation must be one the user
    /// writes (see [`Self::check_trigger_head`]), and it must contain no
    /// Viper operator; the encoding has already rejected the conditionals and
    /// quantifiers in it, so the operators are all this check finds. The
    /// captures and the quantified variables are holes in `expr`, which the
    /// check does not look into.
    pub(super) fn check_trigger(&self, expr: ExprRet<'vir>) -> EncodeResult<'vir, (), MirPureEnc> {
        let span = self.vcx.tcx().def_span(self.body.source.def_id());
        if vir::has_forbidden_trigger_node(expr.as_dyn()) {
            return Err(EncodeFullError::incorrect(
                "a trigger cannot contain the snapshot equality `===` or an operation on \
                 Prusti's mathematical integers and reals (`Int`, `Real`)"
                    .to_string(),
                vec![span],
            ));
        }
        self.check_trigger_head(span)
    }

    /// Checks that the outermost operation of the trigger that this body
    /// computes is one the user writes (see [`HEADS`]), reported at `span`.
    /// The simplifier folds away the borrows and dereferences that rustc and
    /// the encoding insert, but no application of what the user writes (see
    /// `task_encoder::Program::add_user_adt`), so only the latter is sure to
    /// remain a term the trigger can match. Dereferences are looked through:
    /// rustc inserts them for reads through references and for overloaded
    /// indexing, a call to `Index::index` whose result it dereferences.
    fn check_trigger_head(&self, span: Span) -> EncodeResult<'vir, (), MirPureEnc> {
        let error = |message: String| Err(EncodeFullError::incorrect(message, vec![span]));
        let variable = || {
            error(format!(
                "a trigger must be {HEADS}, not a bare variable or constant"
            ))
        };
        let mut definitions = Definitions(IndexVec::from_elem(Set1::Empty, &self.body.local_decls));
        definitions.visit_body(self.body);
        // The value of `local`, if one assignment defines it.
        let assigned = |local: mir::Local| match definitions.0[local] {
            Set1::One(location) => match &self.body.stmt_at(location).left()?.kind {
                mir::StatementKind::Assign(box (_, rvalue)) => Some(rvalue),
                _ => None,
            },
            Set1::Empty | Set1::Many => None,
        };
        let mut place = mir::Place::from(mir::RETURN_PLACE);
        loop {
            let mut projection = place.projection.as_slice();
            while let [rest @ .., mir::ProjectionElem::Deref] = projection {
                projection = rest;
            }
            match projection {
                // A capture, i.e. a field of the closure: a captured variable,
                // or a field of one that the closure captures by itself (a
                // disjoint field capture).
                [mir::ProjectionElem::Deref, mir::ProjectionElem::Field(..)]
                    if place.local == ty::CAPTURE_STRUCT_LOCAL =>
                {
                    return error(format!(
                        "a trigger must be {HEADS}, not a value from outside the quantifier"
                    ));
                }
                // A field of a tuple or a closure that the trigger builds,
                // which the simplifier folds away with the constructor (see
                // below).
                [mir::ProjectionElem::Field(..), ..] => {
                    return match assigned(place.local) {
                        Some(mir::Rvalue::Aggregate(
                            box (mir::AggregateKind::Tuple | mir::AggregateKind::Closure(..)),
                            _,
                        )) => error(format!(
                            "a trigger cannot be a field of a tuple or a closure it builds: it \
                             must be {HEADS}"
                        )),
                        _ => Ok(()),
                    };
                }
                [_, ..] => return Ok(()),
                [] => {}
            }
            let Some(rvalue) = assigned(place.local) else {
                // An argument (the closure itself or a bound variable), which
                // nothing assigns; or else the result of a call, a local
                // assigned several times, or the result left unassigned
                // because the body ends in a call that never returns, which
                // the encoding reports.
                let argument = (1..=self.body.arg_count).contains(&place.local.as_usize());
                return if argument && matches!(definitions.0[place.local], Set1::Empty) {
                    variable()
                } else {
                    Ok(())
                };
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
                // So is a pointer coercion, e.g. unsizing `&[T; N]` to `&[T]`.
                mir::Rvalue::Cast(
                    mir::CastKind::PointerCoercion(..),
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
}

/// The locations that define each local of a body, as rustc's SSA analysis
/// finds them: an assignment of the whole local, or the call it receives the
/// result of. Any other mutation, such as an assignment of one of its fields,
/// makes its value no single definition's.
struct Definitions(IndexVec<mir::Local, Set1<mir::Location>>);

impl<'tcx> Visitor<'tcx> for Definitions {
    fn visit_local(&mut self, local: mir::Local, context: PlaceContext, location: mir::Location) {
        if context.is_place_assignment() {
            self.0[local].insert(location);
        } else if let PlaceContext::MutatingUse(_) = context {
            self.0[local] = Set1::Many;
        }
    }
}

/// The name of the quantified variable at position `idx` of the trigger
/// closure `cl_def_id`, for error messages. Crate metadata records the
/// parameter names of functions only, so a closure of another crate has none.
fn bound_name<'vir>(vcx: &'vir vir::VirCtxt<'vir>, cl_def_id: DefId, idx: usize) -> Option<Symbol> {
    if !cl_def_id.is_local() {
        return None;
    }
    vcx.tcx()
        .fn_arg_idents(cl_def_id)
        .get(idx)
        .copied()
        .flatten()
        .map(|ident| ident.name)
}
