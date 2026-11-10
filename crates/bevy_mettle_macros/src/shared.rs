//! What the derives share: where the crate they implement for is, names in
//! `snake_case`, and marker types known by name.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{DeriveInput, Error, LitStr};

/// A marker type known by name: `impl #trait for #ident { const NAME }`, its
/// name on `#[#attr(name = "..")]`, else the type's in `snake_case`. `what` is
/// what it is, for errors: "a route".
pub(crate) fn named(
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
    for attr in input
        .attrs
        .iter()
        .filter(|found| found.path().is_ident(attr))
    {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("name") {
                return Err(meta.error(format!("expected `name`: {what} has only a name")));
            }
            if name.is_some() {
                return Err(meta.error("`name` is given twice"));
            }
            name = Some(meta.value()?.parse()?);
            Ok(())
        })?;
    }
    let ident = &input.ident;
    let name = name.unwrap_or_else(|| LitStr::new(&snake(&ident.to_string()), Span::call_site()));
    let keyed = keyed(ident, krate, kind, &name);
    Ok(quote! {
        impl #trait_path for #ident {
            const NAME: &'static str = #name;
        }
        #keyed
    })
}

/// `impl Keyed<#kind> for #ident`: its key, `name`, as `kind`. `krate` is the
/// derive's crate, `kind` a kind in core's registry, or `krate`'s own at a
/// path.
pub(crate) fn keyed(
    ident: &syn::Ident,
    krate: &TokenStream,
    kind: &TokenStream,
    name: &LitStr,
) -> TokenStream {
    quote! {
        impl #krate::__core::registry::Keyed<#kind> for #ident {
            const KEY: &'static str = #name;
        }
    }
}

/// Where `krate` is, where a derive's used: itself if it's a dependency, else
/// its `module` in the `bevy_mettle` facade.
pub(crate) fn path(krate: &str, module: &str) -> TokenStream {
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
pub(crate) fn snake(name: &str) -> String {
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

    use super::*;

    #[test]
    fn names_go_snake_case() {
        assert_eq!(snake("Life"), "life");
        assert_eq!(snake("MovementSpeed"), "movement_speed");
        assert_eq!(snake("MaxHP"), "max_hp");
        assert_eq!(snake("HPRegen"), "hp_regen");
    }
}
