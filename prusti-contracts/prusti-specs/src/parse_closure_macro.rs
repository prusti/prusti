use crate::{prusti_attribute_kind, require_argument, unwrap_argument, SpecAttributeKind};
use proc_macro2::TokenStream;
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

pub(crate) struct ClosureWithSpec {
    pub pres: Vec<TokenStream>,
    pub posts: Vec<TokenStream>,
    pub cl: syn::ExprClosure,
}

impl Parse for ClosureWithSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;
        let mut cl: syn::ExprClosure = input.parse()?;

        let mut pres: Vec<syn::Result<TokenStream>> = vec![];
        let mut posts: Vec<syn::Result<TokenStream>> = vec![];

        // collect and remove any specification attributes
        // leave other attributes intact
        attrs.retain(|attr| {
            match prusti_attribute_kind(attr) {
                Some(SpecAttributeKind::Requires) => pres.push(spec_argument(attr)),
                Some(SpecAttributeKind::Ensures) => posts.push(spec_argument(attr)),
                _ => return true,
            }
            false
        });
        cl.attrs = attrs;

        Ok(Self {
            pres: pres.into_iter().collect::<syn::Result<Vec<_>>>()?,
            posts: posts.into_iter().collect::<syn::Result<Vec<_>>>()?,
            cl,
        })
    }
}

/// The argument of the specification attribute `attr`, which is parsed later
/// as a Prusti assertion.
fn spec_argument(attr: &syn::Attribute) -> syn::Result<TokenStream> {
    let argument = unwrap_argument(attr)?;
    require_argument(attr.span(), &argument)?;
    Ok(argument)
}
