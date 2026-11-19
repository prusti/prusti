//! The rules for the terms of a quantifier's trigger, on VIR expressions. A
//! term is an application of a function, constructor or destructor, a field
//! access or a collection operation at its top, and contains none of Viper's
//! own operators, conditionals or ADT discriminators: these are Viper's rules,
//! which Viper checks only when it parses a program from text. A term also
//! contains no quantifier, which Viper allows but a pattern of the SMT solver
//! cannot contain.
//!
//! The encoder's triggers contain `let`s. The rules apply to the terms with
//! the `let`s inlined (see [`inline_lets`]), which is also what the program
//! gets (see [`inline_trigger_lets`]): Silicon looks a trigger up among the
//! terms of the quantifier's body, and evaluates it with the body's path
//! conditions, only if it has no `let`s.

use crate::{collect_locals, gendata::*, refs::*, Foldable, Folder, VirCtxt, Visitable, Visitor};
use prusti_rustc_interface::data_structures::fx::FxHashSet;

/// `item` with the values of the `let`s in its triggers in place of their
/// locals.
pub fn inline_trigger_lets<'vir, T: Foldable<'vir, (), !> + Copy>(
    vcx: &'vir VirCtxt<'_>,
    item: T,
) -> T {
    item.fold_with(&mut InlineTriggerLets { vcx })
        .unwrap_or(item)
}

/// `e` with the values of its `let`s in place of their locals, and without
/// the `let`s. Under an `old(..)`, only values that are locals or constants
/// are inlined, and the `let` of a local still used there stays. The encoder
/// gives every local it binds a fresh name, so a value never ends up under a
/// `let` that rebinds one of its own locals.
pub fn inline_lets<'vir>(vcx: &'vir VirCtxt<'_>, e: ExprDyn<'vir>) -> ExprDyn<'vir> {
    let mut inline = Inline {
        vcx,
        bindings: Vec::new(),
    };
    e.fold_with(&mut inline).unwrap_or(e)
}

/// Inlines the `let`s in the triggers of the quantifiers it folds (see
/// [`inline_trigger_lets`]).
struct InlineTriggerLets<'vir, 'tcx> {
    vcx: &'vir VirCtxt<'tcx>,
}

impl<'vir> Folder<'vir, (), !> for InlineTriggerLets<'vir, '_> {
    fn fold_trigger(&mut self, t: Trigger<'vir>) -> Option<Trigger<'vir>> {
        t.super_fold_with(&mut Inline {
            vcx: self.vcx,
            bindings: Vec::new(),
        })
    }
}

/// Replaces the locals that the `let`s of a term bind by their values (see
/// [`inline_lets`]).
struct Inline<'vir, 'tcx> {
    vcx: &'vir VirCtxt<'tcx>,
    bindings: Vec<(&'vir str, ExprDyn<'vir>)>,
}

impl<'vir> Folder<'vir, (), !> for Inline<'vir, '_> {
    fn fold_expr(&mut self, e: ExprDyn<'vir>) -> Option<ExprDyn<'vir>> {
        match e.kind {
            ExprKindGenData::Let(l) => {
                let val = l.val.fold_with(self).unwrap_or(l.val);
                self.bindings.push((l.name, val));
                let body = l.expr.fold_with(self).unwrap_or(l.expr);
                self.bindings.pop();
                let mut locals = FxHashSet::default();
                collect_locals(body, &mut locals);
                // The `let` stays if its local is still used, under an
                // `old(..)`; Silicon then evaluates the trigger on its own
                // instead of finding it in the body. Otherwise the body takes
                // its place, and keeps
                // the `let`'s span: the verifier's warnings about a trigger
                // point at the span of its root.
                let (kind, debug_info) = if locals.contains(l.name) {
                    let l = LetGenData {
                        name: l.name,
                        val,
                        expr: body,
                    };
                    let kind = ExprKindGenData::Let(self.vcx.alloc(l));
                    (self.vcx.alloc(kind), e.debug_info)
                } else {
                    (body.kind, body.debug_info)
                };
                Some(
                    self.vcx
                        .alloc(ExprGenData::new_inner(kind, debug_info, e.span, body.ty())),
                )
            }
            // A local keeps its value under `old(..)`, but a heap read in the
            // value would read the old heap there. As in the simplifier, only
            // the values that are locals or constants are inlined under it.
            ExprKindGenData::Old(_) => {
                let outer = self.bindings.clone();
                self.bindings.retain(|b| {
                    matches!(
                        b.1.kind,
                        ExprKindGenData::Local(_) | ExprKindGenData::Const(_)
                    )
                });
                let folded = e.super_fold_with(self);
                self.bindings = outer;
                folded
            }
            ExprKindGenData::Local(local) => self
                .bindings
                .iter()
                .rev()
                .find(|b| b.0 == local.name)
                .map(|b| b.1),
            _ => e.super_fold_with(self),
        }
    }
}

/// A trigger term with its `let`s inlined (see [`inline_lets`]), as the rules
/// see it.
pub struct TriggerTermShape<'vir> {
    /// Whether the term is an application or an access at its top, possibly
    /// in `old(..)`s and in the `let`s that the inlining keeps.
    pub valid_head: bool,
    /// Whether the term contains a Viper operator, an ADT discriminator, a
    /// conditional or a quantifier, none of which a trigger term may contain
    /// anywhere.
    pub forbidden: bool,
    /// The locals the term mentions.
    pub locals: FxHashSet<&'vir str>,
}

/// Why terms are not a valid trigger set, the first reason found.
pub enum TriggerSetError {
    /// The term at this position is not an application or an access at its
    /// top.
    Head(usize),
    /// The term at this position contains a node no trigger term may
    /// contain.
    Forbidden(usize),
    /// The term at this position mentions no quantified variable.
    NoQuantifiedVariable(usize),
    /// No term mentions the quantified variable at this position.
    MissingQuantifiedVariable(usize),
}

/// Checks that terms of the shapes `shapes` are a valid trigger set of a
/// quantifier over the variables named `qvars`: each is a valid trigger term
/// that mentions a quantified variable, and together they mention all of
/// them.
pub fn check_trigger_set(
    qvars: &[&str],
    shapes: &[TriggerTermShape<'_>],
) -> Result<(), TriggerSetError> {
    for (idx, shape) in shapes.iter().enumerate() {
        if !shape.valid_head {
            return Err(TriggerSetError::Head(idx));
        }
        if shape.forbidden {
            return Err(TriggerSetError::Forbidden(idx));
        }
        if !qvars.iter().any(|q| shape.locals.contains(q)) {
            return Err(TriggerSetError::NoQuantifiedVariable(idx));
        }
    }
    match qvars
        .iter()
        .position(|q| shapes.iter().all(|shape| !shape.locals.contains(q)))
    {
        Some(idx) => Err(TriggerSetError::MissingQuantifiedVariable(idx)),
        None => Ok(()),
    }
}

/// The shape of the trigger term `e` (see [`TriggerTermShape`]), whose `let`s
/// are expected to be inlined (see [`inline_lets`]).
pub fn trigger_term_shape<'vir>(e: ExprDyn<'vir>) -> TriggerTermShape<'vir> {
    let mut head = e;
    loop {
        head = match head.kind {
            ExprKindGenData::Old(old) => old.expr,
            ExprKindGenData::Let(l) => l.expr,
            _ => break,
        };
    }
    let valid_head = matches!(
        head.kind,
        ExprKindGenData::FuncApp(_)
            | ExprKindGenData::AdtDestructor(..)
            | ExprKindGenData::Field(..)
            | ExprKindGenData::CollectionBinOp(_)
            | ExprKindGenData::CollectionLen(_)
            | ExprKindGenData::MapDomain(_)
            | ExprKindGenData::MapRange(_)
    );
    let mut forbidden = Forbidden(false);
    e.visit_with(&mut forbidden);
    let mut locals = FxHashSet::default();
    collect_locals(e, &mut locals);
    TriggerTermShape {
        valid_head,
        forbidden: forbidden.0,
        locals,
    }
}

/// Finds a node that no trigger term may contain.
struct Forbidden(bool);

impl<'vir> Visitor<'vir, (), !> for Forbidden {
    fn visit_expr(&mut self, e: ExprDyn<'vir>) {
        // A negative integer literal is encoded as the negation of a
        // constant, which the SMT solver reads as a literal.
        let negative_literal = matches!(
            e.kind,
            ExprKindGenData::UnOp(op)
                if op.kind == crate::UnOpKind::Neg
                    && matches!(op.expr.kind, ExprKindGenData::Const(_))
        );
        self.0 |= !negative_literal
            && matches!(
                e.kind,
                ExprKindGenData::UnOp(_)
                    | ExprKindGenData::BinOp(_)
                    | ExprKindGenData::AdtDiscriminator(..)
                    | ExprKindGenData::Ternary(_)
                    | ExprKindGenData::Forall(_)
                    | ExprKindGenData::Exists(_)
            );
        e.super_visit_with(self);
    }
}
