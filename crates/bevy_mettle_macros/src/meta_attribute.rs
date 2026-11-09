//! `#[derive(MetaAttribute)]`: a meta attribute's name.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, Error, LitStr};

use crate::shared::{path, snake};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "a meta attribute can't be generic",
        ));
    }
    let mut name: Option<LitStr> = None;
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("meta_attribute"))
    {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("name") {
                return Err(
                    meta.error("expected `name`: a meta attribute has no base, limits or rounding")
                );
            }
            if name.is_some() {
                return Err(meta.error("`name` is given twice"));
            }
            name = Some(meta.value()?.parse()?);
            Ok(())
        })?;
    }
    let krate = path("bevy_mettle_effect", "effect");
    let ident = &input.ident;
    let name = name.unwrap_or_else(|| LitStr::new(&snake(&ident.to_string()), Span::call_site()));
    Ok(quote! {
        impl #krate::code::MetaAttribute for #ident {
            const NAME: &'static str = #name;
        }
    })
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn a_meta_attribute_has_only_a_name() {
        let input = parse_quote! {
            #[meta_attribute(base = 1.0)]
            struct IncomingDamage;
        };
        let error = expand(&input).expect_err("an error");
        assert!(error.to_string().contains("no base"), "{error}");
    }
}
