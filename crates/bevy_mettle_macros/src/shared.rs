//! What the derives share: where the crate they implement for is, names in
//! `snake_case`, and keys.

use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::LitStr;

/// `impl Keyed for #ident`: its kind, `kind`, and its key, `name`. `krate` is
/// the derive's crate, `kind` a kind in core's registry, or `krate`'s own at a
/// path.
pub(crate) fn keyed(
    ident: &syn::Ident,
    krate: &TokenStream,
    kind: &TokenStream,
    name: &LitStr,
) -> TokenStream {
    quote! {
        impl #krate::__core::registry::Keyed for #ident {
            type Kind = #kind;
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
