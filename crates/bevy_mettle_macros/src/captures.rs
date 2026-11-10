//! What executions, calculations and routes capture: `#[execution(source(A))]`.
//!
//! And the routes an execution routes into: `#[execution(routes(R))]`.

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
/// `#[#attr(source(..), target(..))]`; an execution's routes, from
/// `#[execution(routes(..))]`.
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
    let mut routes: Vec<Type> = Vec::new();
    let routing = attr == "execution";
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
            if meta.path.is_ident("routes") {
                if !routing {
                    return Err(meta.error(unroutable(attr)));
                }
                return push_routes(&meta, &mut routes);
            }
            let target = if meta.path.is_ident("source") {
                false
            } else if meta.path.is_ident("target") {
                true
            } else if routing {
                return Err(meta.error(format!(
                    "expected `name`, `source(..)`, `target(..)` or `routes(..)`: {what} has a \
                     name, captures and the routes it routes into"
                )));
            } else {
                return Err(meta.error(format!(
                    "expected `name`, `source(..)` or `target(..)`: {what} has a name and captures"
                )));
            };
            push_capture(&meta, target, &mut captures)
        })?;
    }
    let ident = &input.ident;
    let name = name.unwrap_or_else(|| LitStr::new(&snake(&ident.to_string()), Span::call_site()));
    let reads = captures.iter().map(|capture| read(capture, krate));
    let keyed = crate::shared::keyed(ident, krate, kind, &name);
    let routes = routing.then(|| {
        quote! {
            fn routes() -> ::std::vec::Vec<
                #krate::__core::registry::Key<#krate::code::AnyRoute>,
            > {
                ::std::vec![#(
                    #krate::__core::registry::Key::<#krate::code::AnyRoute>::of::<#routes>()
                ),*]
            }
        }
    });
    Ok(quote! {
        impl #trait_path for #ident {
            fn captures() -> ::std::vec::Vec<#krate::data::AttributeRead> {
                ::std::vec![#(#reads),*]
            }
            #routes
        }
        #keyed
    })
}

/// The capture in `meta`, `source(..)` or `target(..)`, pushed onto
/// `captures`: an error if it's there already.
fn push_capture(
    meta: &syn::meta::ParseNestedMeta,
    target: bool,
    captures: &mut Vec<Capture>,
) -> syn::Result<()> {
    let content;
    syn::parenthesized!(content in meta.input);
    let options: Options = content.parse()?;
    let ty = &options.ty;
    let key = quote!(#ty).to_string();
    if captures.iter().any(|known| {
        let ty = &known.ty;
        known.target == target && quote!(#ty).to_string() == key
    }) {
        return Err(meta.error(format!(
            "`{key}` of the {} is captured twice",
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
}

/// The routes in `meta`, `routes(..)`, pushed onto `routes`: an error for
/// one there already.
fn push_routes(meta: &syn::meta::ParseNestedMeta, routes: &mut Vec<Type>) -> syn::Result<()> {
    let content;
    syn::parenthesized!(content in meta.input);
    for ty in Punctuated::<Type, Token![,]>::parse_terminated(&content)? {
        let key = quote!(#ty).to_string();
        if routes.iter().any(|known| quote!(#known).to_string() == key) {
            return Err(Error::new_spanned(
                &ty,
                format!("`{key}` is routed into twice"),
            ));
        }
        routes.push(ty);
    }
    Ok(())
}

/// `capture` as the `AttributeRead` it makes: `Magnitude::source::<A>()..`.
fn read(capture: &Capture, krate: &TokenStream) -> TokenStream {
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
}

/// Why `#[#attr(routes(..))]` isn't accepted: only an execution routes.
fn unroutable(attr: &str) -> String {
    match attr {
        "calculation" => "`routes(..)` is an execution's: a calculation works out a magnitude, \
                          not changes, so it routes into nothing"
            .to_owned(),
        "route" => "`routes(..)` is an execution's: a router's changes go to attributes, as \
                    routing is single-pass"
            .to_owned(),
        _ => format!("`routes(..)` is an execution's, not a {attr}'s"),
    }
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
    fn it_routes_into_what_it_declares() {
        let input = parse_quote! {
            #[execution(source(SpellPower), routes(IncomingDamage, path::Healing), routes(Mana))]
            struct FireDamage;
        };
        let expanded = expand(&input).unwrap().to_string();
        for route in ["IncomingDamage", "path :: Healing", "Mana"] {
            assert!(
                expanded.contains(&format!(":: of :: < {route} > ()")),
                "{expanded}"
            );
        }
        assert!(expanded.contains("fn routes ()"), "{expanded}");
    }

    #[test]
    fn it_routes_into_each_once() {
        let input = parse_quote! {
            #[execution(routes(IncomingDamage), routes(IncomingDamage))]
            struct FireDamage;
        };
        let error = expand(&input).expect_err("an error");
        assert!(error.to_string().contains("routed into twice"), "{error}");
    }

    #[test]
    fn a_calculation_routes_into_nothing() {
        let input = parse_quote! {
            #[calculation(source(SpellPower), routes(IncomingDamage))]
            struct BurnTime;
        };
        let error = named_capturing(
            &input,
            "calculation",
            "a calculation",
            &quote!(Calculation),
            &quote!(::bevy_mettle_effect),
            &quote!(::bevy_mettle_effect::code::AnyCalculation),
        )
        .expect_err("an error");
        assert!(
            error.to_string().contains("a magnitude, not changes"),
            "{error}"
        );
    }

    #[test]
    fn a_calculation_has_no_routes() {
        let input = parse_quote! {
            #[calculation(source(SpellPower))]
            struct BurnTime;
        };
        let expanded = named_capturing(
            &input,
            "calculation",
            "a calculation",
            &quote!(Calculation),
            &quote!(::bevy_mettle_effect),
            &quote!(::bevy_mettle_effect::code::AnyCalculation),
        )
        .unwrap()
        .to_string();
        assert!(!expanded.contains("fn routes"), "{expanded}");
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
