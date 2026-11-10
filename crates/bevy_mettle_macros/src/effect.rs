//! `#[derive(Effect)]`: an effect's name.

use proc_macro2::TokenStream;
use syn::DeriveInput;

use crate::shared::{named, path};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let krate = path("bevy_mettle_effect", "effect");
    named(
        input,
        "effect",
        "an effect",
        &quote::quote!(#krate::Effect),
        &krate,
        &quote::quote!(#krate::__core::registry::AnyEffect),
    )
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn it_has_only_a_name() {
        let input = parse_quote! {
            #[effect(base = 1.0)]
            struct Burning;
        };
        let error = expand(&input).expect_err("an error");
        assert!(error.to_string().contains("only a name"), "{error}");
    }

    #[test]
    fn it_is_named_in_snake_case() {
        let input = parse_quote! {
            struct Burning;
        };
        let expanded = expand(&input).unwrap().to_string();
        assert!(expanded.contains("NAME"), "{expanded}");
    }

    #[test]
    fn it_is_keyed_as_an_effect() {
        let input = parse_quote! {
            #[effect(name = "fire.burning")]
            struct Burning;
        };
        let expanded = expand(&input).unwrap().to_string();
        assert!(expanded.contains("Keyed < "), "{expanded}");
        assert!(expanded.contains("AnyEffect"), "{expanded}");
        assert!(
            expanded.contains("const KEY : & 'static str = \"fire.burning\""),
            "{expanded}"
        );
    }
}
