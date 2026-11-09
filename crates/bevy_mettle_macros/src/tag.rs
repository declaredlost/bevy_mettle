//! `#[derive(Tag)]`: a gameplay tag's place: its parent, and its segment.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Data, DeriveInput, Error, Fields, LitStr, Path};

use crate::shared::{path, snake};

/// What `#[tag(..)]` says.
#[derive(Default)]
struct Settings {
    name: Option<LitStr>,
    parent: Option<Path>,
}

/// Reads `#[tag(..)]`.
fn settings(input: &DeriveInput) -> syn::Result<Settings> {
    let mut settings = Settings::default();
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("tag"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                if settings.name.is_some() {
                    return Err(meta.error("`name` is given twice"));
                }
                let name: LitStr = meta.value()?.parse()?;
                if name.value().contains('.') {
                    return Err(Error::new_spanned(
                        &name,
                        "a tag's name is its own segment, without dots: its parent says what \
                         it's under",
                    ));
                }
                settings.name = Some(name);
                Ok(())
            } else if meta.path.is_ident("parent") {
                if settings.parent.is_some() {
                    return Err(meta.error("`parent` is given twice"));
                }
                settings.parent = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("expected `name` or `parent`"))
            }
        })?;
    }
    Ok(settings)
}

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "a tag can't be generic",
        ));
    }
    let unit = matches!(&input.data, Data::Struct(data) if matches!(data.fields, Fields::Unit));
    if !unit {
        return Err(Error::new_spanned(
            &input.ident,
            "a tag is a unit struct: `struct Stunned;`, named by its value",
        ));
    }
    let settings = settings(input)?;
    let krate = path("bevy_mettle_core", "core");
    let ident = &input.ident;
    let segment = settings
        .name
        .unwrap_or_else(|| LitStr::new(&snake(&ident.to_string()), Span::call_site()));
    let parent = settings
        .parent
        .map_or_else(|| quote!(#krate::tag::Root), |parent| quote!(#parent));
    Ok(quote! {
        impl #krate::tag::Tag for #ident {
            const SEGMENT: &'static str = #segment;
            type Parent = #parent;

            fn key() -> #krate::tag::TagKey {
                static KEY: ::std::sync::OnceLock<#krate::tag::TagKey> =
                    ::std::sync::OnceLock::new();
                #krate::tag::TagKey::of::<Self>(&KEY)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn a_tag_is_a_unit_struct() {
        let input = parse_quote! {
            struct Stunned { strength: f32 }
        };
        let error = expand(&input).expect_err("an error");
        assert!(error.to_string().contains("unit struct"), "{error}");
    }

    #[test]
    fn a_tags_name_is_its_own_segment() {
        let input = parse_quote! {
            #[tag(name = "state.stunned")]
            struct Stunned;
        };
        let error = expand(&input).expect_err("an error");
        assert!(error.to_string().contains("without dots"), "{error}");
        let fine = parse_quote! {
            #[tag(parent = State, name = "dazed")]
            struct Stunned;
        };
        assert!(expand(&fine).is_ok());
    }
}
