use crate::{prusti_attribute_kind, unwrap_argument, SpecAttributeKind};
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

pub(crate) struct ClosureWithSpec {
    pub pres: Vec<syn::Expr>,
    pub posts: Vec<syn::Expr>,
    pub cl: syn::ExprClosure,
}

impl Parse for ClosureWithSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut attrs = input.call(syn::Attribute::parse_outer)?;
        let mut cl: syn::ExprClosure = input.parse()?;

        let mut pres: Vec<syn::Result<syn::Expr>> = vec![];
        let mut posts: Vec<syn::Result<syn::Expr>> = vec![];

        // collect and remove any specification attributes
        // leave other attributes intact
        attrs.retain(|attr| {
            match prusti_attribute_kind(attr) {
                Some(SpecAttributeKind::Requires) => pres.push(parse_argument(attr)),
                Some(SpecAttributeKind::Ensures) => posts.push(parse_argument(attr)),
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

/// The expression that is the argument of the specification attribute `attr`.
fn parse_argument(attr: &syn::Attribute) -> syn::Result<syn::Expr> {
    let argument = unwrap_argument(attr)?;
    if argument.is_empty() {
        return Err(syn::Error::new(
            attr.span(),
            "this attribute expects an argument",
        ));
    }
    syn::parse2(argument)
}
