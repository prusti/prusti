use prusti_rustc_interface::{middle::ty, span::def_id::DefId};

/// Whether `def_id` is a function declared in a trait (rather than in an impl
/// or outside of any). A call of it goes through the trait (see `TraitFnEnc`).
pub fn is_trait_fn(tcx: ty::TyCtxt<'_>, def_id: DefId) -> bool {
    tcx.opt_associated_item(def_id)
        .is_some_and(|item| item.trait_container(tcx).is_some())
}

pub fn is_function_with_body(tcx: ty::TyCtxt<'_>, def_id: DefId) -> bool {
    if let Some(trait_def_id) = tcx.trait_of_assoc(def_id) {
        let associated_items = tcx.associated_items(trait_def_id);
        if let Some(assoc_item) = associated_items
            .in_definition_order()
            .find(|item| item.def_id == def_id)
        {
            return assoc_item.defaultness(tcx).has_value();
        }
    }
    true
}
