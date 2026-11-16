// © 2019, ETH Zurich
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

//! Helpers for finding and reading Prusti's `prusti::<name>` attributes.

use prusti_rustc_interface::{
    hir,
    middle::ty::TyCtxt,
    span::{def_id::DefId, Span, Symbol},
};
use std::borrow::Borrow;

/// Returns an iterator over all Prusti attributes (i.e. `prusti::<attr_name>="...")`.
fn get_prusti_attrs<T: Borrow<hir::Attribute>>(
    attrs: &[T],
) -> impl Iterator<Item = &hir::AttrItem> {
    attrs.iter().filter_map(|attr| match attr.borrow() {
        hir::Attribute::Unparsed(item)
            if item.path.segments.len() == 2 && item.path.segments[0].as_str() == "prusti" =>
        {
            Some(&**item)
        }
        _ => None,
    })
}

fn get_prusti_attr<'a, T: Borrow<hir::Attribute>>(
    attrs: &'a [T],
    attr_name: &str,
) -> Option<&'a hir::AttrItem> {
    get_prusti_attrs(attrs).find(|item| item.path.segments[1].as_str() == attr_name)
}

/// The `prusti::<attr_name>` marker on `def_id`, for local and external items.
/// Prusti's attributes are never parsed into an `AttributeKind`, so `find_attr!`
/// cannot see them; matching the whole path finds exactly what we emit.
pub fn get_prusti_attr_of<'tcx>(
    tcx: TyCtxt<'tcx>,
    def_id: DefId,
    attr_name: &str,
) -> Option<&'tcx hir::AttrItem> {
    let path = [Symbol::intern("prusti"), Symbol::intern(attr_name)];
    // Bound so that the iterator borrowing `path` is dropped before it.
    let mut attrs = tcx.get_attrs_by_path(def_id, &path);
    attrs.find_map(|attr| match attr {
        hir::Attribute::Unparsed(item) => Some(&**item),
        _ => None,
    })
}

/// Check if `prusti::<name>` is among the attributes.
/// Any arguments of the attribute are ignored.
pub fn has_prusti_attr(attrs: &[hir::Attribute], name: &str) -> bool {
    get_prusti_attr(attrs, name).is_some()
}

/// The spans of the user's Prusti annotations among `attrs`: every
/// `prusti::<name>` marker except the per-item version marker. Each marker is
/// emitted at its own annotation's span, so these point at the individual
/// attributes rather than the whole item.
pub fn prusti_annotation_spans(attrs: &[hir::Attribute]) -> impl Iterator<Item = Span> + '_ {
    get_prusti_attrs(attrs)
        .filter(|item| item.path.segments[1].as_str() != "specs_version")
        .map(|item| item.span)
}

/// Check if `prusti::spec_only` is among the attributes.
pub fn has_spec_only_attr(attrs: &[hir::Attribute]) -> bool {
    has_prusti_attr(attrs, "spec_only")
}

/// Check if `prusti::extern_spec` is among the attributes.
pub fn has_extern_spec_attr(attrs: &[hir::Attribute]) -> bool {
    has_prusti_attr(attrs, "extern_spec")
}

pub fn read_specs_version_attr(attr: &hir::Attribute) -> Option<String> {
    read_prusti_attr("specs_version", &[attr])
}

pub fn has_to_model_fn_attr(attrs: &[hir::Attribute]) -> bool {
    has_prusti_attr(attrs, "type_models_to_model_fn")
}

pub fn has_to_model_impl_attr(attrs: &[hir::Attribute]) -> bool {
    has_prusti_attr(attrs, "type_models_to_model_impl")
}

pub fn has_trait_bounds_type_cond_spec(attrs: &[hir::Attribute]) -> bool {
    has_prusti_attr(attrs, "type_cond_spec_trait_bounds_in_where_clause")
}

pub fn has_abstract_predicate_attr(attrs: &[hir::Attribute]) -> bool {
    has_prusti_attr(attrs, "abstract_predicate")
}

/// Read the value stored in a Prusti attribute (e.g. `prusti::<attr_name>="...")`.
pub fn read_prusti_attrs<'a, T: Borrow<hir::Attribute>>(
    attr_name: &'a str,
    attrs: &'a [T],
) -> impl Iterator<Item = String> + 'a {
    get_prusti_attrs(attrs)
        .filter(move |item| item.path.segments[1].as_str() == attr_name)
        .filter_map(read_prusti_attr_value)
}

/// The value of a single Prusti attribute, if it stores one.
pub fn read_prusti_attr_value(item: &hir::AttrItem) -> Option<String> {
    match &item.args {
        hir::AttrArgs::Eq { expr, .. } => {
            Some(expr.as_token_lit().symbol.as_str().replace("\\\"", "\""))
        }
        _ => None,
    }
}

/// Read the value stored in a single Prusti attribute (e.g. `prusti::<attr_name>="...")`.
pub fn read_prusti_attr<T: Borrow<hir::Attribute>>(attr_name: &str, attrs: &[T]) -> Option<String> {
    read_prusti_attrs(attr_name, attrs).next()
}
