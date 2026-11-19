use pcg::borrow_pcg::FunctionData;
use prusti_rustc_interface::{hir::def::DefKind, middle::ty, span::def_id::DefId};
use vir::ViperIdent;

use crate::encoders::ty::generics::{GArgs, GParams, trait_impls::ImplFn};

/// The built-in `Fn*` impls of a local closure or fn item, as [`TraitImplEnc`]
/// checks them, with the prefix of the names of the check methods; `None` for
/// anything else (a trait impl).
///
/// [`TraitImplEnc`]: super::trait_impls::TraitImplEnc
pub(crate) fn fn_trait_impl<'vir>(
    vcx: &'vir vir::VirCtxt<'vir>,
    def_id: DefId,
) -> Option<(&'vir str, Vec<ImplFn<'vir>>)> {
    match vcx.tcx().def_kind(def_id) {
        DefKind::Closure => Some(closure_impl_fn(vcx, def_id)),
        DefKind::Fn | DefKind::AssocFn => Some(fn_item_impl_fn(vcx, def_id)),
        _ => None,
    }
}

/// The built-in `Fn*` impls of a local closure. A call through the `Fn*` trait
/// method of the closure's own kind runs the closure body, so the body is checked as the implementation of that trait
/// method. The impls of the other kinds that the closure implements call the
/// body with the environment taken by value, or by `&mut` where the body takes
/// it by `&`, so they need no more than it.
fn closure_impl_fn<'vir>(
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
    let trait_fn = fn_trait_method(vcx, closure_args.as_closure().kind());
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

/// The built-in `Fn*` impls of a local fn item. A fn item implements all
/// three `Fn*` traits, each calling the function with the arguments untupled,
/// so the function is checked as the implementation of `Fn::call`, at `Self` =
/// the fn item's type and `Args` = the tuple of its parameters.
/// `IndirectResources` reads the trait method's resources from the
/// implementation's arguments at the same positions, which for a fn item are
/// off by one, as it has no `self`. This is harmless: `Fn::call` has no
/// indirect resources, since `self` is a shared reference and `Args` a type
/// parameter.
fn fn_item_impl_fn<'vir>(
    vcx: &'vir vir::VirCtxt<'vir>,
    function: DefId,
) -> (&'vir str, Vec<ImplFn<'vir>>) {
    let sig = FunctionData::new(function).identity_fn_sig(vcx.tcx());
    let fn_item_ty = vcx
        .tcx()
        .type_of(function)
        .instantiate_identity()
        .skip_normalization();
    let trait_args = GArgs::new(
        GParams::from(function),
        vcx.tcx().mk_args(&[
            fn_item_ty.into(),
            ty::Ty::new_tup(vcx.tcx(), sig.inputs()).into(),
        ]),
    );
    let name = ViperIdent::from_def_id(vcx, function);
    (
        vir::vir_format!(vcx, "fn_{name}_impl"),
        vec![ImplFn {
            def_id: function,
            trait_fn: fn_trait_method(vcx, ty::ClosureKind::Fn),
            trait_args,
        }],
    )
}

/// The method of the `Fn*` trait of the given kind.
fn fn_trait_method(vcx: &vir::VirCtxt<'_>, kind: ty::ClosureKind) -> DefId {
    let fn_trait = vcx.tcx().fn_trait_kind_to_def_id(kind).unwrap();
    vcx.tcx()
        .associated_items(fn_trait)
        .in_definition_order()
        .find(|item| item.is_fn())
        .unwrap()
        .def_id
}

/// The fn items in `ty` whose built-in `Fn*` impls `TraitImplEnc` checks: local
/// functions and methods that have such impls, by rustc's condition for them
/// (`assemble_fn_pointer_candidates`: a safe, non-variadic Rust function
/// without target features). A constructor takes values and has no
/// specification, so it needs no check.
pub(crate) fn fn_items_with_fn_impls(ty: ty::Ty<'_>) -> impl Iterator<Item = DefId> {
    ty.walk().filter_map(|arg| {
        let ty::TyKind::FnDef(function, _) = *arg.as_type()?.kind() else {
            return None;
        };
        vir::with_vcx(|vcx| {
            (function.is_local()
                && matches!(vcx.tcx().def_kind(function), DefKind::Fn | DefKind::AssocFn)
                && vcx
                    .tcx()
                    .fn_sig(function)
                    .skip_binder()
                    .is_fn_trait_compatible()
                && vcx
                    .tcx()
                    .codegen_fn_attrs(function)
                    .target_features
                    .is_empty())
            .then_some(function)
        })
    })
}
