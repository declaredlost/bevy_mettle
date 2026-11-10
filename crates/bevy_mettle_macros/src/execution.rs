//! `#[derive(Execution)]`: an execution's name and what it captures.

use proc_macro2::TokenStream;
use syn::DeriveInput;

use crate::captures::named_capturing;
use crate::shared::path;

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let krate = path("bevy_mettle_effect", "effect");
    named_capturing(
        input,
        "execution",
        "an execution",
        &quote::quote!(#krate::code::Execution),
        &krate,
        &quote::quote!(#krate::code::AnyExecution),
    )
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn it_has_a_name_and_captures() {
        let input = parse_quote! {
            #[execution(base = 1.0)]
            struct FireDamage;
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
            struct FireDamage;
        };
        let expanded = expand(&input).unwrap().to_string();
        assert!(expanded.contains("NAME"), "{expanded}");
    }
}
