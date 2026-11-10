//! `#[derive(Attribute)]`: an attribute's settings, read and written out as
//! its constants.

use proc_macro2::{Literal, Span, TokenStream};
use quote::{format_ident, quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{DeriveInput, Error, Expr, ExprLit, ExprUnary, Ident, Lit, LitStr, Path, UnOp};

use crate::shared::{path, snake};

/// What `#[attribute(..)]` says.
#[derive(Default)]
struct Settings {
    name: Option<LitStr>,
    base: Option<Literal>,
    min: Option<Limit>,
    max: Option<Limit>,
    rounding: Option<Ident>,
    on_limit: Option<Ident>,
}

/// A limit: a number, or another attribute.
enum Limit {
    Value(Literal),
    Attribute(Path),
}

pub(crate) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "an attribute can't be generic",
        ));
    }
    let settings = settings(input)?;
    let krate = path("bevy_mettle_attribute", "attribute");
    let ident = &input.ident;
    let name = settings
        .name
        .unwrap_or_else(|| LitStr::new(&snake(&ident.to_string()), Span::call_site()));
    let base = settings.base.map(|base| quote!(const BASE: f32 = #base;));
    let limit = |which: &str, limit: Option<Limit>| {
        let which = format_ident!("{which}");
        limit.map(|limit| {
            let limit = match limit {
                Limit::Value(value) => quote!(#krate::registry::Limit::Value(#value)),
                Limit::Attribute(other) => {
                    // Spanned at the path, so a type that isn't an attribute
                    // is pointed out where it's written.
                    quote_spanned!(other.span()=> #krate::registry::Limit::attribute::<#other>())
                }
            };
            quote! {
                const #which: ::core::option::Option<#krate::registry::Limit> =
                    ::core::option::Option::Some(#limit);
            }
        })
    };
    let min = limit("MIN", settings.min);
    let max = limit("MAX", settings.max);
    let rounding = settings.rounding.map(|rounding| {
        quote! {
            const ROUNDING: ::core::option::Option<#krate::registry::Rounding> =
                ::core::option::Option::Some(#krate::registry::Rounding::#rounding);
        }
    });
    let on_limit = settings.on_limit.map(|on_limit| {
        quote! {
            const ON_LIMIT: #krate::registry::OnLimit = #krate::registry::OnLimit::#on_limit;
        }
    });
    let keyed = crate::shared::keyed(
        ident,
        &krate,
        &quote!(#krate::__core::registry::AnyAttribute),
        &name,
    );
    Ok(quote! {
        impl #krate::Attribute for #ident {
            const NAME: &'static str = #name;
            #base
            #min
            #max
            #rounding
            #on_limit
        }
        #keyed
    })
}

/// Reads `#[attribute(..)]`.
fn settings(input: &DeriveInput) -> syn::Result<Settings> {
    let mut settings = Settings::default();
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("attribute"))
    {
        attr.parse_nested_meta(|meta| {
            let key = meta
                .path
                .get_ident()
                .map(ToString::to_string)
                .unwrap_or_default();
            let value = meta.value()?;
            let given = match key.as_str() {
                "name" => settings.name.is_some(),
                "base" => settings.base.is_some(),
                "min" => settings.min.is_some(),
                "max" => settings.max.is_some(),
                "rounding" => settings.rounding.is_some(),
                "on_limit" => settings.on_limit.is_some(),
                _ => false,
            };
            if given {
                return Err(meta.error(format!("`{key}` is given twice")));
            }
            match key.as_str() {
                "name" => settings.name = Some(value.parse()?),
                "base" => settings.base = Some(number(&value.parse()?)?),
                "min" => settings.min = Some(limit(&value.parse()?)?),
                "max" => settings.max = Some(limit(&value.parse()?)?),
                "rounding" => {
                    let rounding: Ident = value.parse()?;
                    if !["Down", "Up", "Nearest"].contains(&rounding.to_string().as_str()) {
                        return Err(Error::new_spanned(
                            rounding,
                            "rounding is `Down`, `Up` or `Nearest`",
                        ));
                    }
                    settings.rounding = Some(rounding);
                }
                "on_limit" => {
                    let on_limit: Ident = value.parse()?;
                    if !["Clamp", "Ratio"].contains(&on_limit.to_string().as_str()) {
                        return Err(Error::new_spanned(
                            on_limit,
                            "on_limit is `Clamp` or `Ratio`",
                        ));
                    }
                    settings.on_limit = Some(on_limit);
                }
                _ => {
                    return Err(meta
                        .error("expected `name`, `base`, `min`, `max`, `rounding` or `on_limit`"));
                }
            }
            Ok(())
        })?;
    }
    let follows = |limit: &Option<Limit>| matches!(limit, Some(Limit::Attribute(_)));
    if let Some(on_limit) = &settings.on_limit {
        if on_limit == "Ratio" && !follows(&settings.max) {
            return Err(Error::new_spanned(
                on_limit,
                "`on_limit = Ratio` keeps a fraction of a maximum that's another attribute: \
                 give `max` one",
            ));
        }
        if !follows(&settings.min) && !follows(&settings.max) {
            return Err(Error::new_spanned(
                on_limit,
                "`on_limit` says what the base does as a limit that's another attribute \
                 moves: give `min` or `max` one",
            ));
        }
    }
    Ok(settings)
}

/// A limit: a number, or the path of another attribute.
fn limit(expr: &Expr) -> syn::Result<Limit> {
    match expr {
        Expr::Path(path) => {
            let path = &path.path;
            let primitive = path.segments.len() > 1
                && path.segments.first().is_some_and(|first| {
                    [
                        "f32", "f64", "i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64",
                    ]
                    .iter()
                    .any(|primitive| first.ident == primitive)
                });
            if primitive {
                return Err(Error::new_spanned(
                    path,
                    "a limit is a number written out, or an attribute's type",
                ));
            }
            Ok(Limit::Attribute(path.clone()))
        }
        _ => number(expr).map(Limit::Value),
    }
}

/// A number literal, maybe negative, as an `f32` literal.
fn number(expr: &Expr) -> syn::Result<Literal> {
    let (negative, lit) = match expr {
        Expr::Lit(ExprLit { lit, .. }) => (false, lit),
        Expr::Unary(ExprUnary {
            op: UnOp::Neg(_),
            expr,
            ..
        }) => match &**expr {
            Expr::Lit(ExprLit { lit, .. }) => (true, lit),
            other => return Err(Error::new(other.span(), "expected a number")),
        },
        other => return Err(Error::new(other.span(), "expected a number")),
    };
    let value: f32 = match lit {
        Lit::Float(float) => float.base10_parse()?,
        Lit::Int(int) => int.base10_parse()?,
        other => return Err(Error::new(other.span(), "expected a number")),
    };
    Ok(Literal::f32_suffixed(if negative { -value } else { value }))
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn a_setting_given_twice_is_an_error() {
        let input = parse_quote! {
            #[attribute(base = 1.0, base = 2.0)]
            struct Life;
        };
        let error = settings(&input).err().expect("an error");
        assert!(error.to_string().contains("given twice"), "{error}");
    }

    #[test]
    fn on_limit_ratio_needs_a_maximum_thats_an_attribute() {
        let input = parse_quote! {
            #[attribute(min = MinLife, max = 100.0, on_limit = Ratio)]
            struct Life;
        };
        let error = settings(&input).err().expect("an error");
        assert!(error.to_string().contains("give `max` one"), "{error}");
    }

    #[test]
    fn on_limit_needs_a_limit_thats_an_attribute() {
        let input = parse_quote! {
            #[attribute(max = 100.0, on_limit = Clamp)]
            struct Life;
        };
        let error = settings(&input).err().expect("an error");
        assert!(
            error.to_string().contains("give `min` or `max` one"),
            "{error}"
        );
        let fine = parse_quote! {
            #[attribute(max = MaxLife, on_limit = Ratio)]
            struct Life;
        };
        assert!(settings(&fine).is_ok());
    }

    #[test]
    fn a_limit_that_names_a_constant_is_an_error() {
        let input = parse_quote! {
            #[attribute(min = f32::MIN)]
            struct Life;
        };
        let error = settings(&input).err().expect("an error");
        assert!(
            error.to_string().contains("a number written out"),
            "{error}"
        );
    }
}
