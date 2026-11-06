//! `#[derive(IdTarget)]`, re-exported by `bevy_mettle_core`, and
//! `#[derive(Attribute)]`, by `bevy_mettle_attribute`: their documentation is
//! there.

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Literal, Span};
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{
    DeriveInput, Error, Expr, ExprLit, ExprUnary, Ident, Lit, LitStr, Path, UnOp, parse_macro_input,
};

/// An attribute: a marker type, its settings on `#[attribute(..)]`.
///
/// ```ignore
/// #[derive(Attribute)]
/// #[attribute(base = 50.0, min = 0.0, max = MaxLife)]
/// pub struct Life;
/// ```
#[proc_macro_derive(Attribute, attributes(attribute))]
pub fn derive_attribute(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// Derives `IdTarget`: named as the type is in `snake_case`, unless overridden.
///
/// Override with `#[id_target(name = "..")]`.
///
/// ```ignore
/// #[derive(IdTarget)]
/// pub struct Effect { pub id: Id<Effect>, .. }
///
/// #[derive(IdTarget)]
/// #[id_target(name = "curve")]
/// pub struct LevelCurve { .. }
/// ```
#[proc_macro_derive(IdTarget, attributes(id_target))]
pub fn derive_id_target(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand_id(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn expand_id(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
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

/// What `#[attribute(..)]` says.
#[derive(Default)]
struct Settings {
    name: Option<LitStr>,
    base: Option<Literal>,
    min: Option<Limit>,
    max: Option<Limit>,
    rounding: Option<Ident>,
}

/// A limit: a number, or another attribute.
enum Limit {
    Value(Literal),
    Attribute(Path),
}

fn expand(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
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
                    quote!(#krate::registry::Limit::Attribute(<#other as #krate::Attribute>::ID))
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
    Ok(quote! {
        impl #krate::Attribute for #ident {
            const NAME: &'static str = #name;
            #base
            #min
            #max
            #rounding
        }
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
                _ => {
                    return Err(meta.error("expected `name`, `base`, `min`, `max` or `rounding`"));
                }
            }
            Ok(())
        })?;
    }
    Ok(settings)
}

/// A limit: a number, or the path of another attribute.
fn limit(expr: &Expr) -> syn::Result<Limit> {
    match expr {
        Expr::Path(path) => Ok(Limit::Attribute(path.path.clone())),
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

/// Where `krate` is, where a derive's used: itself if it's a dependency, else
/// its `module` in the `bevy_mettle` facade.
fn path(krate: &str, module: &str) -> proc_macro2::TokenStream {
    let ident = |name: &str| format_ident!("{}", name.replace('-', "_"));
    match crate_name(krate) {
        Ok(FoundCrate::Itself) => {
            let krate = ident(krate);
            quote!(::#krate)
        }
        Ok(FoundCrate::Name(found)) => {
            let found = ident(&found);
            quote!(::#found)
        }
        Err(_) => {
            let facade = match crate_name("bevy_mettle") {
                Ok(FoundCrate::Name(found)) => ident(&found),
                _ => ident("bevy_mettle"),
            };
            let module = ident(module);
            quote!(::#facade::#module)
        }
    }
}

/// `name` in `snake_case`: `MovementSpeed` is `movement_speed`.
fn snake(name: &str) -> String {
    let mut snake = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (index, &char) in chars.iter().enumerate() {
        if char.is_uppercase() {
            let after_lower = index > 0 && !chars[index - 1].is_uppercase();
            let before_lower = chars.get(index + 1).is_some_and(|next| next.is_lowercase());
            if index > 0 && (after_lower || (before_lower && chars[index - 1].is_uppercase())) {
                snake.push('_');
            }
            snake.extend(char.to_lowercase());
        } else {
            snake.push(char);
        }
    }
    snake
}

#[cfg(test)]
mod tests {
    use super::snake;

    #[test]
    fn names_go_snake_case() {
        assert_eq!(snake("Life"), "life");
        assert_eq!(snake("MovementSpeed"), "movement_speed");
        assert_eq!(snake("MaxHP"), "max_hp");
        assert_eq!(snake("HPRegen"), "hp_regen");
    }
}
