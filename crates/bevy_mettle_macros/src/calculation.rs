//! `#[derive(Calculation)]`: a calculation's name and what it captures.

use proc_macro2::TokenStream;
use syn::DeriveInput;

use crate::captures::named_capturing;
use crate::shared::path;

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let krate = path("bevy_mettle_effect", "effect");
    named_capturing(
        input,
        "calculation",
        "a calculation",
        &quote::quote!(#krate::code::Calculation),
        &krate,
    )
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn it_has_a_name_and_captures() {
        let input = parse_quote! {
            #[calculation(base = 1.0)]
            struct BurnTime;
        };
        let error = expand(&input).expect_err("an error");
        assert!(
            error.to_string().contains("has a name and captures"),
            "{error}"
        );
    }

    #[test]
    fn it_is_named_in_snake_case() {
        let input = parse_quote! {
            struct BurnTime;
        };
        let expanded = expand(&input).unwrap().to_string();
        assert!(expanded.contains("NAME"), "{expanded}");
    }
}
