//! `#[derive(IdTarget)]` and `#[derive(Tag)]`, re-exported by
//! `bevy_mettle_core`;
//! `#[derive(Attribute)]`, by `bevy_mettle_attribute`;
//! `#[derive(Effect)]`, `#[derive(Route)]`, `#[derive(Execution)]` and
//! `#[derive(Calculation)]`, by `bevy_mettle_effect`; and
//! `#[derive(Ability)]` and `#[derive(Behaviour)]`, by `bevy_mettle_ability`:
//! their documentation is there.

mod ability;
mod attribute;
mod behaviour;
mod calculation;
mod effect;
mod execution;
mod id_target;
mod route;
mod shared;
mod tag;

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

/// A route: a marker type, its name on `#[route(..)]`.
///
/// ```ignore
/// #[derive(Route)]
/// #[route(name = "incoming_damage")]
/// pub struct IncomingDamage;
/// ```
#[proc_macro_derive(Route, attributes(route))]
pub fn derive_route(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    route::expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// An effect code names: a marker type, its name on `#[effect(..)]`.
///
/// ```ignore
/// #[derive(Effect)]
/// #[effect(name = "burning")]
/// pub struct Burning;
/// ```
#[proc_macro_derive(Effect, attributes(effect))]
pub fn derive_effect(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    effect::expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// An execution: a marker type, its name on `#[execution(..)]`.
///
/// ```ignore
/// #[derive(Execution)]
/// pub struct FireDamage;
/// ```
#[proc_macro_derive(Execution, attributes(execution))]
pub fn derive_execution(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    execution::expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// A calculation: a marker type, its name on `#[calculation(..)]`.
///
/// ```ignore
/// #[derive(Calculation)]
/// pub struct BurnTime;
/// ```
#[proc_macro_derive(Calculation, attributes(calculation))]
pub fn derive_calculation(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    calculation::expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

/// An ability code names: a marker type, its name on `#[ability(..)]`.
///
/// ```ignore
/// #[derive(Ability)]
/// pub struct Fireball;
/// ```
#[proc_macro_derive(Ability, attributes(ability))]
pub fn derive_ability(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ability::expand(&input)
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
/// pub struct EffectData { pub id: Id<AnyEffect>, .. }
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

/// A gameplay tag: a unit struct, its place on `#[tag(..)]`.
///
/// ```ignore
/// #[derive(Tag)]
/// #[tag(parent = State)]
/// pub struct Stunned;
/// ```
#[proc_macro_derive(Tag, attributes(tag))]
pub fn derive_tag(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    tag::expand(&input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
