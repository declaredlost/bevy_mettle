//! `#[derive(Ability)]`: an ability, defined by its keys.

use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

use crate::defined::{self, Defining};
use crate::shared::path;

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let krate = path("bevy_mettle_ability", "ability");
    defined::expand(
        input,
        &Defining {
            attr: "ability",
            what: "an ability",
            trait_path: quote!(#krate::Ability),
            data: quote!(#krate::data::AbilityData),
            kind: quote!(#krate::__core::registry::AnyAbility),
            krate,
        },
    )
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn it_is_keyed_as_an_ability() {
        let input = parse_quote! {
            #[ability(behaviour::<CastFireball>, cooldown = 2.0)]
            struct Fireball;
        };
        let expanded = expand(&input).unwrap().to_string();
        assert!(expanded.contains("Ability for Fireball"), "{expanded}");
        assert!(
            expanded.contains("const KEY : & 'static str = \"fireball\""),
            "{expanded}"
        );
        assert!(
            expanded.contains(". behaviour :: < CastFireball > () . cooldown (2.0)"),
            "{expanded}"
        );
    }
}
