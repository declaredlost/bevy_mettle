//! `#[derive(Behaviour)]`: a behaviour's name and input.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, Error, LitStr, Type};

use crate::shared::{path, snake};

/// What `#[behaviour(..)]` says.
#[derive(Default)]
struct Settings {
    name: Option<LitStr>,
    input: Option<Type>,
}

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "a behaviour can't be generic",
        ));
    }
    let settings = settings(input)?;
    let krate = path("bevy_mettle_ability", "ability");
    let ident = &input.ident;
    let name = settings
        .name
        .unwrap_or_else(|| LitStr::new(&snake(&ident.to_string()), Span::call_site()));
    let input = settings.input.map_or_else(
        || quote!(#krate::behaviour::NoInput),
        |input| quote!(#input),
    );
    Ok(quote! {
        impl #krate::behaviour::Behaviour for #ident {
            const NAME: &'static str = #name;
            type Input = #input;
        }
    })
}

/// Reads `#[behaviour(..)]`.
fn settings(input: &DeriveInput) -> syn::Result<Settings> {
    let mut settings = Settings::default();
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("behaviour"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                if settings.name.is_some() {
                    return Err(meta.error("`name` is given twice"));
                }
                settings.name = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("input") {
                if settings.input.is_some() {
                    return Err(meta.error("`input` is given twice"));
                }
                settings.input = Some(meta.value()?.parse()?);
            } else {
                return Err(meta.error("expected `name` or `input`"));
            }
            Ok(())
        })?;
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn a_behaviour_setting_given_twice_is_an_error() {
        let input = parse_quote! {
            #[behaviour(name = "a", name = "b")]
            struct Cast;
        };
        let error = settings(&input).err().expect("an error");
        assert!(error.to_string().contains("given twice"), "{error}");
    }

    #[test]
    fn a_behaviour_setting_that_isnt_one_is_an_error() {
        let input = parse_quote! {
            #[behaviour(speed = 2)]
            struct Cast;
        };
        let error = settings(&input).err().expect("an error");
        assert!(
            error.to_string().contains("expected `name` or `input`"),
            "{error}"
        );
    }
}
