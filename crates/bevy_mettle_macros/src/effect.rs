//! `#[derive(Effect)]`: an effect, defined by its keys.

use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

use crate::defined::{self, Defining};
use crate::shared::path;

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let krate = path("bevy_mettle_effect", "effect");
    defined::expand(
        input,
        &Defining {
            attr: "effect",
            what: "an effect",
            trait_path: quote!(#krate::Effect),
            data: quote!(#krate::data::EffectData),
            kind: quote!(#krate::__core::registry::AnyEffect),
            krate,
        },
    )
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn it_is_keyed_as_an_effect() {
        let input = parse_quote! {
            #[effect(name = "fire.burning", lasting = 4.0)]
            struct Burning;
        };
        let expanded = expand(&input).unwrap().to_string();
        assert!(expanded.contains("Effect for Burning"), "{expanded}");
        assert!(expanded.contains("Keyed for Burning"), "{expanded}");
        assert!(
            expanded.contains("__core :: registry :: AnyEffect ;"),
            "{expanded}"
        );
        assert!(expanded.contains(". lasting (4.0)"), "{expanded}");
    }
}
