//! `#[derive(IdTarget)]`: what an id names, and its name.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, Error, LitStr};

use crate::shared::{path, snake};

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "what an id names can't be generic",
        ));
    }
    let mut name = None;
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("id_target"))
    {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("name") {
                return Err(meta.error("expected `name = \"..\"`"));
            }
            name = Some(meta.value()?.parse::<LitStr>()?);
            Ok(())
        })?;
    }
    let ident = &input.ident;
    let name = name.unwrap_or_else(|| LitStr::new(&snake(&ident.to_string()), Span::call_site()));
    let krate = path("bevy_mettle_core", "core");
    Ok(quote! {
        impl #krate::IdTarget for #ident {
            const NAME: &'static str = #name;
        }
    })
}
