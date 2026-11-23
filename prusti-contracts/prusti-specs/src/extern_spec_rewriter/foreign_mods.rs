//! Process external specifications in Rust foreign modules marked with
//! the #[extern_spec] attribute.

use super::functions::rewrite_fn;
use proc_macro2::TokenStream;
use syn::spanned::Spanned;

pub fn rewrite_extern_spec(
    item_foreign_mod: &syn::ItemForeignMod,
    path: &syn::Path,
) -> syn::Result<TokenStream> {
    let mut res = TokenStream::new();
    for item in item_foreign_mod.items.iter() {
        match item {
            syn::ForeignItem::Fn(item_fn) => res.extend(rewrite_fn(item_fn, path, true)),
            // eventually: handle specs for foreign variables (statics)
            _ => return Err(syn::Error::new(item.span(), "unexpected item")),
        }
    }
    Ok(res)
}
