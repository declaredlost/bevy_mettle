//! `#[derive(IdTarget)]`, re-exported by `bevy_mettle_core`;
//! `#[derive(Attribute)]`, by `bevy_mettle_attribute`;
//! `#[derive(MetaAttribute)]`, by `bevy_mettle_effect`; and
//! `#[derive(Behaviour)]`, by `bevy_mettle_ability`: their documentation is
//! there.

mod attribute;
mod behaviour;
mod id_target;
mod meta_attribute;
mod shared;

use proc_macro::TokenStream;
use syn::{DeriveInput, Error, parse_macro_input};

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
    attribute::expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// A meta attribute: a marker type, its name on `#[meta_attribute(..)]`.
///
/// ```ignore
/// #[derive(MetaAttribute)]
/// #[meta_attribute(name = "incoming_damage")]
/// pub struct IncomingDamage;
/// ```
#[proc_macro_derive(MetaAttribute, attributes(meta_attribute))]
pub fn derive_meta_attribute(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    meta_attribute::expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// A behaviour: what runs an ability, its settings on `#[behaviour(..)]`.
///
/// ```ignore
/// #[derive(Component, Default, Behaviour)]
/// #[behaviour(name = "cast_fireball", input = Aim)]
/// pub struct CastFireball { timer: f32 }
/// ```
#[proc_macro_derive(Behaviour, attributes(behaviour))]
pub fn derive_behaviour(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    behaviour::expand(&input)
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
    id_target::expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
