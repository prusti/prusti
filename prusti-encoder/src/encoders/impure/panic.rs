//! Panics are control flow: every method returns, besides its result (through
//! the return place argument), whether it panicked, i.e. unwound instead of
//! returning, and the snapshot of its result. The parts of its postcondition
//! about the result hold only if it returned; everything else (e.g. the
//! referents of `&mut` arguments, which an unwinding callee leaves valid)
//! holds either way.
//!
//! The left-hand sides of the wands of a method's postcondition refer to the
//! result through the returned snapshot rather than through the return place:
//! guarded by the call having returned, they are then well-defined (and can
//! be applied) whether it returned or not, as no permission to the result is
//! needed to evaluate them.

use prusti_utils::config;
use vir::CastType;

use crate::encoders::LocalDef;

/// The name of a method's return variable that records whether it panicked.
const PANICKED: &str = "panicked";

/// The name of a method's return variable holding the snapshot of its result
/// if it returned (`result` is a Viper keyword).
const RESULT_SNAP: &str = "res_snap";

/// The declaration of a method's `panicked` return variable.
pub(crate) fn panicked_decl<'vir>(
    vcx: &'vir vir::VirCtxt<'vir>,
) -> vir::LocalDecl<'vir, vir::Bool> {
    vcx.mk_local_decl(PANICKED, vir::TYPE_BOOL)
}

/// The declaration of a method's `res_snap` return variable, for a result
/// whose snapshot has type `ty`.
pub(crate) fn result_snap_decl<'vir>(
    vcx: &'vir vir::VirCtxt<'vir>,
    ty: vir::TypeSnap<'vir>,
) -> vir::LocalDeclSnap<'vir> {
    vcx.mk_local_decl(RESULT_SNAP, ty)
}

/// The condition that the call whose `panicked` value is `panicked` returned
/// normally.
pub(crate) fn returned<'vir>(
    vcx: &'vir vir::VirCtxt<'vir>,
    panicked: vir::ExprBool<'vir>,
) -> vir::ExprBool<'vir> {
    vcx.mk_unary_op_expr(vir::UnOpKind::Not, panicked.upcast_ty())
        .downcast_ty()
}

/// `returned ==> expr`: `expr` holds if the call returned normally.
pub(crate) fn if_returned<'vir>(
    vcx: &'vir vir::VirCtxt<'vir>,
    returned: vir::ExprBool<'vir>,
    expr: vir::ExprBool<'vir>,
) -> vir::ExprBool<'vir> {
    vcx.mk_bin_op_expr(vir::BinOpKind::Implies, returned, expr)
        .downcast_ty()
}

/// The return variables of a method and the postconditions about them.
pub(crate) struct MethodReturns<'vir> {
    /// The declarations of the return variables `panicked` and `res_snap`.
    pub decls: &'vir [vir::LocalDeclDyn<'vir>],
    /// The condition that the method returned normally.
    pub returned: vir::ExprBool<'vir>,
    /// The declaration of the `res_snap` return variable.
    pub result_snap: vir::LocalDeclSnap<'vir>,
    /// Unless only the core proof is verified, that the method does not
    /// panic; and that if it returned, its return place holds its result,
    /// whose snapshot it returns in `res_snap`.
    pub posts: Vec<vir::ExprBool<'vir>>,
}

/// The return variables of a method whose return place is `ret`.
pub(crate) fn method_returns<'vir>(
    vcx: &'vir vir::VirCtxt<'vir>,
    ret: &LocalDef<'vir>,
) -> MethodReturns<'vir> {
    let panicked = panicked_decl(vcx);
    let returned = returned(vcx, vcx.mk_local_ex(panicked));
    let result_snap = result_snap_decl(vcx, ret.local_snap.ty);
    let mut posts = Vec::new();
    if !config::only_core_proof() {
        posts.push(returned);
    }
    posts.push(if_returned(
        vcx,
        returned,
        vcx.mk_conj(&[
            ret.impure_pred,
            vcx.mk_eq_expr(ret.impure_snap, vcx.mk_local_ex(result_snap)),
        ]),
    ));
    MethodReturns {
        decls: vcx.alloc_slice(&[panicked.as_dyn(), result_snap.as_dyn()]),
        returned,
        result_snap,
        posts,
    }
}
