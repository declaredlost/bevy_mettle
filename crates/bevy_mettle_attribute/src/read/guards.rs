//! Guards on attributes: read-only systems taking an agent and a parameter, as
//! `bevy_arbor` takes them. The guard on owned tags is core's:
//! `bevy_mettle_core::tag::owns`.
//!
//! ```ignore
//! behaviour_tree! {
//!     Select => {
//!         when(attribute_below::<Life>, 20.0) => task("Flee", Flee),
//!         when(owns, TagKey::named("state.enraged")) => task("Charge", Charge),
//!     }
//! }
//! ```

use bevy::prelude::*;

use crate::{Attribute, Current};

/// A guard: whether the agent's `A` is above `value`, with nothing in play.
///
/// No for an agent without it.
#[must_use]
pub fn attribute_above<A: Attribute>(
    In((agent, value)): In<(Entity, f32)>,
    actors: Query<&Current<A>>,
) -> bool {
    actors.get(agent).is_ok_and(|current| current.get() > value)
}

/// A guard: whether the agent's `A` is below `value`, with nothing in play.
///
/// No for an agent without it.
#[must_use]
pub fn attribute_below<A: Attribute>(
    In((agent, value)): In<(Entity, f32)>,
    actors: Query<&Current<A>>,
) -> bool {
    actors.get(agent).is_ok_and(|current| current.get() < value)
}
