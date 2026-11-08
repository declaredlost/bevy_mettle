//! Attributes over lightyear: served on the server, shown on the client.

use bevy::prelude::*;
use bevy_mettle_attribute::authority::{self, Served};
use bevy_mettle_attribute::registry::AttributeState;
use bevy_mettle_attribute::{Attribute, Base, Current};
use lightyear::prelude::*;

/// On a server: has each actor with `A` serve it, from its value now.
pub(super) fn serve<A: Attribute>(
    mut commands: Commands,
    actors: Query<(Entity, &Current<A>), (With<Base<A>>, Without<Served<A>>)>,
) {
    for (actor, current) in &actors {
        commands
            .entity(actor)
            .insert(Served::<A>::new(current.get()));
    }
}

/// On a client: marks each actor replicated from the server
/// [`Remote`](authority::Remote), so it shows the values served.
pub(super) fn remote(
    mut commands: Commands,
    actors: Query<
        Entity,
        (
            With<Replicated>,
            With<AttributeState>,
            Without<authority::Remote>,
        ),
    >,
) {
    for actor in &actors {
        commands.entity(actor).insert(authority::Remote);
    }
}
