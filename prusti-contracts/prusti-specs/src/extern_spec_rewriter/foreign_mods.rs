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
            syn::ForeignItem::Fn(item_fn) => {
                // The stub is a safe function outside the `extern` block. It
                // calls the specified function in an `unsafe` block unless
                // that function is declared `safe`.
                let is_unsafe = !matches!(item_fn.sig.safety, syn::Safety::Safe(_));
                let mut item_fn = item_fn.clone();
                item_fn.sig.safety = syn::Safety::Default;
                res.extend(rewrite_fn(&item_fn, path, is_unsafe))
            }
            // eventually: handle specs for foreign variables (statics)
            _ => return Err(syn::Error::new(item.span(), "unexpected item")),
        }
    }
    Ok(res)
}
