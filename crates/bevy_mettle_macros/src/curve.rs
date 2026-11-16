//! `#[derive(ScalingCurve)]`: a curve, defined by its keys.

use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

use crate::defined::{self, Defining};
use crate::shared::path;

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let krate = path("bevy_mettle_core", "core");
    defined::expand(
        input,
        &Defining {
            attr: "curve",
            what: "a curve",
            trait_path: quote!(#krate::curve::ScalingCurve),
            data: quote!(#krate::curve::CurveData),
            kind: quote!(#krate::registry::AnyCurve),
            krate,
        },
    )
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn it_is_keyed_as_a_curve() {
        let input = parse_quote! {
            #[curve(keys = [(1.0, 10.0), (10.0, 85.0)], constant)]
            struct FireballDamage;
        };
        let expanded = expand(&input).unwrap().to_string();
        assert!(
            expanded.contains("ScalingCurve for FireballDamage"),
            "{expanded}"
        );
        assert!(
            expanded.contains("const KEY : & 'static str = \"fireball_damage\""),
            "{expanded}"
        );
        assert!(expanded.contains(". constant ()"), "{expanded}");
    }
}
