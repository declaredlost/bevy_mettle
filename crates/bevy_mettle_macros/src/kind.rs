//! `#[derive(Kind)]`: a kind of thing content names, and what it's called.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, Error, LitStr};

use crate::shared::{path, snake};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "a kind can't be generic",
        ));
    }
    let mut name = None;
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("kind"))
    {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("name") {
                return Err(meta.error("expected `name = \"..\"`"));
            }
            if name.is_some() {
                return Err(meta.error("`name` is given twice"));
            }
            name = Some(meta.value()?.parse::<LitStr>()?);
            Ok(())
        })?;
    }
    let ident = &input.ident;
    let name = name.unwrap_or_else(|| LitStr::new(&called(&ident.to_string()), Span::call_site()));
    let krate = path("bevy_mettle_core", "core");
    Ok(quote! {
        impl #krate::registry::Kind for #ident {
            const NAME: &'static str = #name;
        }
    })
}

/// What a kind's called by default: its type's name in `snake_case`, without a
/// leading `Any`. `AnyEffect` is `effect`.
fn called(ident: &str) -> String {
    let bare = ident
        .strip_prefix("Any")
        .filter(|rest| rest.starts_with(char::is_uppercase))
        .unwrap_or(ident);
    snake(bare)
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn a_kind_is_called_without_any() {
        assert_eq!(called("AnyEffect"), "effect");
        assert_eq!(called("AnyCurveData"), "curve_data");
        assert_eq!(called("Anything"), "anything");
        assert_eq!(called("Item"), "item");
    }

    #[test]
    fn a_kind_cant_be_generic() {
        let input = parse_quote! {
            enum AnyThing<T> { Of(T) }
        };
        let error = expand(&input).expect_err("an error");
        assert!(error.to_string().contains("generic"), "{error}");
    }
}
