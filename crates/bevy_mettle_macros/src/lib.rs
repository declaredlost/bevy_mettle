//! `#[derive(Kind)]` and `#[derive(Tag)]`, re-exported by
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
mod kind;
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

/// A route effects deal: a marker type, its name on `#[route(..)]`.
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

/// A kind of thing content names, named in messages after its type.
///
/// Named as the type is in `snake_case`, without a leading `Any`, unless
/// `#[kind(name = "..")]` says otherwise.
///
/// ```ignore
/// #[derive(Kind)]
/// pub enum AnyEffect {}
/// ```
#[proc_macro_derive(Kind, attributes(kind))]
pub fn derive_kind(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    kind::expand(&input)
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
