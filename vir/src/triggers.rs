//! The rules for the terms of a quantifier's trigger, on VIR expressions. A
//! term is an application of a function, constructor or destructor, a field
//! access or a collection operation at its top, and contains none of Viper's
//! own operators, conditionals or ADT discriminators: these are Viper's rules,
//! which Viper checks only when it parses a program from text. A term also
//! contains no quantifier, which Viper allows but a pattern of the SMT solver
//! cannot contain.
//!
//! The encoder's triggers contain `let`s, which [`inline_trigger_lets`]
//! inlines in the whole program before the simplifier runs: Silicon looks a
//! trigger up among the terms of the quantifier's body, and evaluates it with
//! the body's path conditions, only if it has no `let`s. The rules apply to
//! the terms with the `let`s inlined.

use crate::{
    collect_locals, gendata::*, genrefs::ExprGenDyn, refs::*, Foldable, Folder, VirCtxt, Visitable,
    Visitor,
};
use prusti_rustc_interface::data_structures::fx::FxHashSet;

/// `item` with the values of the `let`s in its triggers in place of their
/// locals, and without the `let`s. Under an `old(..)`, only values that are
/// locals or constants are inlined, and the `let` of a local still used
/// there stays. The encoder gives every local it binds a fresh name, so a
/// value never ends up under a `let` that rebinds one of its own locals.
pub fn inline_trigger_lets<'vir, T: Foldable<'vir, (), !> + Copy>(
    vcx: &'vir VirCtxt<'_>,
    item: T,
) -> T {
    item.fold_with(&mut InlineTriggerLets { vcx })
        .unwrap_or(item)
}

/// `e` with its `let`s inlined like a trigger term (see
/// [`inline_trigger_lets`]).
#[cfg(test)]
pub(crate) fn inline_lets<'vir>(vcx: &'vir VirCtxt<'_>, e: ExprDyn<'vir>) -> ExprDyn<'vir> {
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
/// [`inline_trigger_lets`]).
struct Inline<'vir, 'tcx> {
    vcx: &'vir VirCtxt<'tcx>,
    bindings: Vec<(&'vir str, ExprDyn<'vir>)>,
}

impl<'vir> Folder<'vir, (), !> for Inline<'vir, '_> {
    fn fold_expr(&mut self, e: ExprDyn<'vir>) -> Option<ExprDyn<'vir>> {
        match e.kind {
            ExprKindGenData::Let(l) => {
                let folded_val = l.val.fold_with(self);
                let val = folded_val.unwrap_or(l.val);
                self.bindings.push((l.name, val));
                let folded_body = l.expr.fold_with(self);
                let body = folded_body.unwrap_or(l.expr);
                self.bindings.pop();
                let mut locals = FxHashSet::default();
                collect_locals(body, &mut locals);
                let used = locals.contains(l.name);
                if used && folded_val.is_none() && folded_body.is_none() {
                    return None;
                }
                // The `let` stays if its local is still used, under an
                // `old(..)`; Silicon then evaluates the trigger on its own
                // instead of finding it in the body. Otherwise the body takes
                // its place, and keeps the `let`'s span: the verifier's
                // warnings about a trigger point at the span of its root.
                let (kind, debug_info) = if used {
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
                self.bindings.retain(|b| b.1.is_local_or_const());
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

/// Whether `terms` are a valid trigger set of a quantifier over `qvars`: each
/// is a valid trigger term that mentions a quantified variable, and together
/// they mention all of them.
pub(crate) fn is_trigger_set<'vir>(qvars: &[LocalDeclDyn<'vir>], terms: &[ExprDyn<'vir>]) -> bool {
    let shapes = terms
        .iter()
        .map(|&term| trigger_term_shape(term))
        .collect::<Vec<_>>();
    let mentions =
        |shape: &TriggerTermShape<'_>, qvar: &LocalDeclDyn<'_>| shape.locals.contains(qvar.name);
    shapes.iter().all(|shape| {
        !shape.forbidden && shape.valid_head && qvars.iter().any(|q| mentions(shape, q))
    }) && qvars
        .iter()
        .all(|q| shapes.iter().any(|shape| mentions(shape, q)))
}

/// A trigger term with its `let`s inlined (see [`inline_trigger_lets`]), as
/// the rules see it.
pub(crate) struct TriggerTermShape<'vir> {
    /// Whether the term is an application or an access at its top, possibly
    /// in `old(..)`s and in the `let`s that the inlining keeps.
    pub(crate) valid_head: bool,
    /// Whether the term contains a Viper operator, an ADT discriminator, a
    /// conditional or a quantifier, none of which a trigger term may contain
    /// anywhere.
    pub(crate) forbidden: bool,
    /// The locals the term mentions.
    pub(crate) locals: FxHashSet<&'vir str>,
}

/// The shape of the trigger term `e` (see [`TriggerTermShape`]), whose `let`s
/// are expected to be inlined (see [`inline_trigger_lets`]).
pub(crate) fn trigger_term_shape<'vir>(e: ExprDyn<'vir>) -> TriggerTermShape<'vir> {
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
            | ExprKindGenData::CollectionLiteral(_)
            | ExprKindGenData::CollectionUpdate(_)
            | ExprKindGenData::CollectionLen(_)
            | ExprKindGenData::MapDomain(_)
            | ExprKindGenData::MapRange(_)
    );
    let mut locals = FxHashSet::default();
    collect_locals(e, &mut locals);
    TriggerTermShape {
        valid_head,
        forbidden: has_forbidden_trigger_node(e),
        locals,
    }
}

/// Whether `e` contains a Viper operator, an ADT discriminator, a
/// conditional or a quantifier, none of which a trigger term may contain
/// anywhere. The `Lazy` nodes of an expression that is not yet instantiated
/// are not looked into.
pub fn has_forbidden_trigger_node<'vir, Curr, Next>(e: ExprGenDyn<'vir, Curr, Next>) -> bool {
    let mut forbidden = Forbidden(false);
    e.visit_with(&mut forbidden);
    forbidden.0
}

/// Finds a node that no trigger term may contain.
struct Forbidden(bool);

impl<'vir, Curr, Next> Visitor<'vir, Curr, Next> for Forbidden {
    fn visit_expr(&mut self, e: ExprGenDyn<'vir, Curr, Next>) {
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
