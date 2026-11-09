//! `#[derive(Route)]`: a route's name.

use proc_macro2::TokenStream;
use syn::DeriveInput;

use crate::shared::{named, path};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let krate = path("bevy_mettle_effect", "effect");
    named(
        input,
        "route",
        "a route",
        &quote::quote!(#krate::code::Route),
    )
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn it_has_only_a_name() {
        let input = parse_quote! {
            #[route(base = 1.0)]
            struct IncomingDamage;
        };
        let error = expand(&input).expect_err("an error");
        assert!(error.to_string().contains("only a name"), "{error}");
    }

    #[test]
    fn it_is_named_in_snake_case() {
        let input = parse_quote! {
            struct IncomingDamage;
        };
        let expanded = expand(&input).unwrap().to_string();
        assert!(expanded.contains("NAME"), "{expanded}");
    }
}
