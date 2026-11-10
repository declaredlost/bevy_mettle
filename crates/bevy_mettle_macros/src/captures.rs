//! What executions and calculations capture: `#[execution(source(A, ..))]`.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{DeriveInput, Error, Ident, LitStr, Token, Type};

use crate::shared::snake;

/// One capture: `source(SpellPower, live, bonus, tagged("damage.fire"))`.
struct Capture {
    target: bool,
    ty: Type,
    live: bool,
    value: Option<Ident>,
    tags: Vec<LitStr>,
}

/// What's in a capture's parentheses: its attribute, then its options.
struct Options {
    ty: Type,
    live: bool,
    value: Option<Ident>,
    tags: Vec<LitStr>,
}

impl Parse for Options {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ty: Type = input.parse()?;
        let mut options = Self {
            ty,
            live: false,
            value: None,
            tags: Vec::new(),
        };
        while input.parse::<Token![,]>().is_ok() {
            if input.is_empty() {
                break;
            }
            let option: Ident = input.parse()?;
            match option.to_string().as_str() {
                "live" => options.live = true,
                "base" | "bonus" => {
                    if options.value.is_some() {
                        return Err(Error::new(option.span(), "a capture reads one value"));
                    }
                    options.value = Some(option);
                }
                "tagged" => {
                    let content;
                    syn::parenthesized!(content in input);
                    let tags = Punctuated::<LitStr, Token![,]>::parse_terminated(&content)?;
                    options.tags.extend(tags);
                }
                _ => {
                    return Err(Error::new(
                        option.span(),
                        "expected `live`, `base`, `bonus` or `tagged(..)`",
                    ));
                }
            }
        }
        Ok(options)
    }
}

/// `impl #trait_path for #ident`: its name, from `#[#attr(name = "..")]` or
/// the type's in `snake_case`, and what it captures, from
/// `#[#attr(source(..), target(..))]`.
pub(crate) fn named_capturing(
    input: &DeriveInput,
    attr: &str,
    what: &str,
    trait_path: &TokenStream,
    krate: &TokenStream,
    kind: &TokenStream,
) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            format!("{what} can't be generic"),
        ));
    }
    let mut name: Option<LitStr> = None;
    let mut captures: Vec<Capture> = Vec::new();
    for found in input
        .attrs
        .iter()
        .filter(|found| found.path().is_ident(attr))
    {
        found.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                if name.is_some() {
                    return Err(meta.error("`name` is given twice"));
                }
                name = Some(meta.value()?.parse()?);
                return Ok(());
            }
            let target = if meta.path.is_ident("source") {
                false
            } else if meta.path.is_ident("target") {
                true
            } else {
                return Err(meta.error(format!(
                    "expected `name`, `source(..)` or `target(..)`: {what} has a name and captures"
                )));
            };
            let content;
            syn::parenthesized!(content in meta.input);
            let options: Options = content.parse()?;
            let ty = &options.ty;
            let key = (target, quote!(#ty).to_string());
            if captures.iter().any(|known| {
                (known.target, {
                    let ty = &known.ty;
                    quote!(#ty).to_string()
                }) == key
            }) {
                return Err(meta.error(format!(
                    "`{}` of the {} is captured twice",
                    key.1,
                    if target { "target" } else { "source" }
                )));
            }
            captures.push(Capture {
                target,
                ty: options.ty,
                live: options.live,
                value: options.value,
                tags: options.tags,
            });
            Ok(())
        })?;
    }
    let ident = &input.ident;
    let name = name.unwrap_or_else(|| LitStr::new(&snake(&ident.to_string()), Span::call_site()));
    let reads = captures.iter().map(|capture| {
        let ty = &capture.ty;
        let mut read = if capture.target {
            quote!(#krate::data::Magnitude::target::<#ty>())
        } else {
            quote!(#krate::data::Magnitude::source::<#ty>())
        };
        if let Some(value) = &capture.value {
            read = quote!(#read.#value());
        }
        if capture.live {
            read = quote!(#read.live());
        }
        for tag in &capture.tags {
            read = quote!(#read.tagged(#tag));
        }
        read
    });
    let keyed = crate::shared::keyed(ident, krate, kind, &name);
    Ok(quote! {
        impl #trait_path for #ident {
            const NAME: &'static str = #name;

            fn captures() -> ::std::vec::Vec<#krate::data::AttributeRead> {
                ::std::vec![#(#reads),*]
            }
        }
        #keyed
    })
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
        named_capturing(
            input,
            "execution",
            "an execution",
            &quote!(Execution),
            &quote!(::bevy_mettle_effect),
            &quote!(::bevy_mettle_effect::code::AnyExecution),
        )
    }

    #[test]
    fn it_captures_as_it_says() {
        let input = parse_quote! {
            #[execution(source(SpellPower, bonus), target(Armour, live, tagged("damage.fire")))]
            struct FireDamage;
        };
        let expanded = expand(&input).unwrap().to_string();
        assert!(
            expanded.contains("source :: < SpellPower > () . bonus ()"),
            "{expanded}"
        );
        assert!(
            expanded.contains("target :: < Armour > () . live () . tagged (\"damage.fire\")"),
            "{expanded}"
        );
        assert!(expanded.contains("\"fire_damage\""), "{expanded}");
    }

    #[test]
    fn it_captures_each_once() {
        let input = parse_quote! {
            #[execution(source(SpellPower), source(SpellPower, live))]
            struct FireDamage;
        };
        let error = expand(&input).expect_err("an error");
        assert!(error.to_string().contains("captured twice"), "{error}");
    }

    #[test]
    fn it_reads_one_value() {
        let input = parse_quote! {
            #[execution(source(SpellPower, base, bonus))]
            struct FireDamage;
        };
        let error = expand(&input).expect_err("an error");
        assert!(error.to_string().contains("one value"), "{error}");
    }
}
