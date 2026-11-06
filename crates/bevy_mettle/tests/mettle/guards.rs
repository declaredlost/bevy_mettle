//! Guards: attributes and owned tags, as `bevy_arbor` reads them.

use bevy::ecs::system::RunSystemOnce;
use bevy_mettle::attribute::read::{attribute_above, attribute_below};
use bevy_mettle::core::tag::owns;
use bevy_mettle::prelude::*;

use crate::common::{Damage, actor, app, own};

#[test]
fn guards_read_attributes_and_owned_tags() {
    let mut app = app();
    let npc = actor(&mut app, []);
    own(&mut app, npc, "state.low_life", true);
    let world = app.world_mut();
    assert!(
        world
            .run_system_once_with(attribute_above::<Damage>, (npc, 9.0))
            .unwrap()
    );
    assert!(
        !world
            .run_system_once_with(attribute_above::<Damage>, (npc, 10.0))
            .unwrap(),
        "10 isn't above 10"
    );
    assert!(
        !world
            .run_system_once_with(attribute_below::<Damage>, (npc, 10.0))
            .unwrap()
    );
    assert!(
        world
            .run_system_once_with(owns, (npc, Id::from_static("state")))
            .unwrap()
    );
    assert!(
        !world
            .run_system_once_with(owns, (npc, Id::from_static("state.moving")))
            .unwrap()
    );
}
