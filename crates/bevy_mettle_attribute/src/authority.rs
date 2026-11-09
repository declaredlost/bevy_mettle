//! Whose values an actor shows: its own, or those its authority serves.
//!
//! A server works an actor's values out and serves them as [`Served<A>`]; a
//! client marks the actor [`Remote`], and shows them as its `Current<A>` and
//! its reads with nothing in play, rather than working them out from the
//! modifiers replicated to it. A prediction counting for it
//! ([`Provisional`](crate::modifier::Provisional)) is shown as its own until
//! it's answered. Reads in context (a hit's tags) are worked out here either
//! way: on a client, they're advisory, as Unreal's are.

use std::fmt;
use std::marker::PhantomData;

use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::reflect::utility::GenericTypePathCell;

use crate::registry::{AttributeRegistry, AttributeState};
use crate::{Attribute, Current};

/// On an actor: attribute `A`'s value with nothing in play, as served.
///
/// Where the actor's values are its own (no [`Remote`]), it's kept as its
/// `Current<A>`, for the game to replicate; on a `Remote` actor, it's what
/// `Current<A>` shows. Written as its value.
#[derive(Component, Reflect)]
#[reflect(Component, Default, type_path = false)]
pub struct Served<A: Attribute> {
    value: f32,
    #[reflect(ignore)]
    attribute: PhantomData<fn() -> A>,
}

impl<A: Attribute> Default for Served<A> {
    /// Where its base starts.
    fn default() -> Self {
        Self::new(A::BASE)
    }
}

impl<A: Attribute> Served<A> {
    /// At `value`.
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self {
            value,
            attribute: PhantomData,
        }
    }

    /// Its value.
    #[must_use]
    pub fn get(&self) -> f32 {
        self.value
    }
}

impl<A: Attribute> Clone for Served<A> {
    fn clone(&self) -> Self {
        Self::new(self.value)
    }
}

impl<A: Attribute> PartialEq for Served<A> {
    fn eq(&self, other: &Self) -> bool {
        self.value.to_bits() == other.value.to_bits()
    }
}

impl<A: Attribute> fmt::Debug for Served<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Served<{}>({})", A::NAME, self.value)
    }
}

/// Named by its attribute's name: `bevy_mettle_attribute::Served<life>`.
impl<A: Attribute> TypePath for Served<A> {
    fn type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("bevy_mettle_attribute::Served<{}>", A::NAME))
    }

    fn short_type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("Served<{}>", A::NAME))
    }

    fn type_ident() -> Option<&'static str> {
        Some("Served")
    }

    fn crate_name() -> Option<&'static str> {
        Some("bevy_mettle_attribute")
    }

    fn module_path() -> Option<&'static str> {
        Some("bevy_mettle_attribute::authority")
    }
}

/// Written as its value: what a server replicates.
impl<A: Attribute> serde::Serialize for Served<A> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f32(self.value)
    }
}

impl<'de, A: Attribute> serde::Deserialize<'de> for Served<A> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        f32::deserialize(deserializer).map(Self::new)
    }
}

/// On an actor: its authority is another world, which serves its values.
///
/// Its `Current<A>`, and its reads of `A` with nothing in play, are its
/// [`Served<A>`], while no prediction counts for it.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Clone, Debug, Default, PartialEq)]
pub struct Remote;

/// Syncs each `Remote` actor's `Served<A>` into its state, as it changes or
/// the actor comes to be remote.
pub(crate) fn sync_served<A: Attribute>(
    registry: Res<AttributeRegistry>,
    mut actors: Query<
        (&Served<A>, &mut AttributeState),
        (With<Remote>, Or<(Changed<Served<A>>, Added<Remote>)>),
    >,
) {
    let Some(id) = registry.id_of::<A>() else {
        return;
    };
    for (served, mut state) in &mut actors {
        state.set_served(id, served.get());
    }
}

/// Marks each actor's state remote or not, as its `Remote` comes and goes.
pub(crate) fn sync_remote(
    mut added: Query<&mut AttributeState, (With<Remote>, Added<Remote>)>,
    mut gone: RemovedComponents<Remote>,
    mut states: Query<&mut AttributeState, Without<Remote>>,
) {
    for mut state in &mut added {
        state.set_remote(true);
    }
    for entity in gone.read() {
        if let Ok(mut state) = states.get_mut(entity) {
            state.set_remote(false);
        }
    }
}

/// Keeps each actor's own `Served<A>` as its `Current<A>`, to serve.
pub(crate) fn serve<A: Attribute>(
    mut actors: Query<
        (&Current<A>, &mut Served<A>),
        (Without<Remote>, Or<(Changed<Current<A>>, Added<Served<A>>)>),
    >,
) {
    for (current, mut served) in &mut actors {
        let value = current.get();
        if served.value.to_bits() != value.to_bits() {
            served.value = value;
        }
    }
}
