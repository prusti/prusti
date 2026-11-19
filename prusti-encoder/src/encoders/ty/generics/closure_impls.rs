use pcg::borrow_pcg::FunctionData;
use prusti_rustc_interface::{middle::ty, span::def_id::DefId};
use vir::ViperIdent;

use crate::encoders::ty::generics::{GArgs, GParams, trait_impls::ImplFn};

/// A local closure's built-in `Fn*` impls, as [`TraitImplEnc`] checks them,
/// with the prefix of the names of the check methods. A call through the
/// `Fn*` trait method of the closure's own kind runs the closure body, so the
/// body is checked as the implementation of that trait method. The impls of
/// the other kinds that the closure implements call the body with the
/// environment taken by value, or by `&mut` where the body takes it by `&`,
/// so they need no more than it.
///
/// [`TraitImplEnc`]: super::trait_impls::TraitImplEnc
pub(crate) fn closure_impl_fn<'vir>(
    vcx: &'vir vir::VirCtxt<'vir>,
    closure: DefId,
) -> (&'vir str, Vec<ImplFn<'vir>>) {
    // The trait method is that of the closure's own kind, at `Self` = the
    // closure and `Args` = the tuple of its parameters, as the body's
    // signature has them.
    let sig = FunctionData::new(closure).identity_fn_sig(vcx.tcx());
    let closure_ty = sig.inputs()[0].peel_refs();
    let ty::TyKind::Closure(_, closure_args) = *closure_ty.kind() else {
        unreachable!("the first argument of a closure body is its environment")
    };
    let fn_trait = vcx
        .tcx()
        .fn_trait_kind_to_def_id(closure_args.as_closure().kind())
        .unwrap();
    let trait_fn = vcx
        .tcx()
        .associated_items(fn_trait)
        .in_definition_order()
        .find(|item| item.is_fn())
        .unwrap()
        .def_id;
    let trait_args = GArgs::new(
        GParams::from(closure),
        vcx.tcx().mk_args(&[
            closure_ty.into(),
            ty::Ty::new_tup(vcx.tcx(), &sig.inputs()[1..]).into(),
        ]),
    );
    let name = ViperIdent::from_def_id(vcx, closure);
    (
        vir::vir_format!(vcx, "closure_{name}_impl"),
        vec![ImplFn {
            def_id: closure,
            trait_fn,
            trait_args,
        }],
    )
}
