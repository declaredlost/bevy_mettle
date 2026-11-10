//! `#[derive(Route)]`: a route's name and what its router captures.

use proc_macro2::TokenStream;
use syn::DeriveInput;

use crate::captures::named_capturing;
use crate::shared::path;

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let krate = path("bevy_mettle_effect", "effect");
    named_capturing(
        input,
        "route",
        "a route",
        &quote::quote!(#krate::code::Route),
        &krate,
        &quote::quote!(#krate::code::AnyRoute),
    )
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn it_has_a_name_and_captures() {
        let input = parse_quote! {
            #[route(base = 1.0)]
            struct IncomingDamage;
        };
        let error = expand(&input).expect_err("an error");
        assert!(
            error.to_string().contains("has a name and captures"),
            "{error}"
        );
    }

    #[test]
    fn it_captures_as_it_says() {
        let input = parse_quote! {
            #[route(target(Shield, live))]
            struct IncomingDamage;
        };
        let expanded = expand(&input).unwrap().to_string();
        assert!(
            expanded.contains("target :: < Shield > () . live ()"),
            "{expanded}"
        );
    }

    #[test]
    fn it_is_named_in_snake_case() {
        let input = parse_quote! {
            struct IncomingDamage;
        };
        let expanded = expand(&input).unwrap().to_string();
        assert!(
            expanded.contains("const KEY : & 'static str = \"incoming_damage\""),
            "{expanded}"
        );
    }
}
