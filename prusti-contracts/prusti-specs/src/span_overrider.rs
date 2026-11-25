use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::ToTokens;
use syn::parse::Parse;

/// Returns `node` with the span of each of its tokens, punctuation and
/// delimiters included, set to `span`, by printing and parsing it again. What
/// `node` does not print is lost: [syn::Generics], for example, does not print
/// its `where` clause, so the result has none.
pub fn override_spans<T: Parse + ToTokens>(node: &T, span: Span) -> syn::Result<T> {
    syn::parse2(override_token_spans(node.to_token_stream(), span))
}

fn override_token_spans(tokens: TokenStream, span: Span) -> TokenStream {
    tokens
        .into_iter()
        .map(|token| match token {
            TokenTree::Group(group) => {
                let stream = override_token_spans(group.stream(), span);
                let mut group = Group::new(group.delimiter(), stream);
                group.set_span(span);
                TokenTree::Group(group)
            }
            mut token => {
                token.set_span(span);
                token
            }
        })
        .collect()
}
