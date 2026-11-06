//! What replication needs: modifiers that serialize by name, and a
//! source's actor mapped into another world.

use bevy::ecs::entity::{EntityHashMap, MapEntities};
use bevy::prelude::*;
use bevy_mettle::prelude::*;

use crate::common::{BEHIND, Damage, MaxLife, Resistance, Strength};

#[test]
fn modifiers_round_trip_by_name() {
    let modifiers = Modifiers(vec![
        Modifier::additive::<Resistance>(0.2)
            .read_tagged("damage.elemental")
            .read_tagged("delivery.spell")
            .condition(Condition::fact_set(BEHIND)),
        Modifier::flat::<Damage>(3.0).local(),
        Modifier::flat::<MaxLife>(2.0).per::<Strength>(10.0),
    ]);
    let text = ron::to_string(&modifiers).unwrap();
    assert!(
        text.contains(r#""resistance""#) && text.contains(r#""damage.elemental""#),
        "{text}"
    );
    assert_eq!(ron::from_str::<Modifiers>(&text).unwrap(), modifiers);
}

#[test]
fn a_sources_actor_maps_into_another_world() {
    let mut world = World::new();
    let theirs = world.spawn_empty().id();
    let ours = world.spawn_empty().id();
    let mut mapping = EntityHashMap::default();
    mapping.insert(theirs, ours);
    let mut of = ModifierOf(theirs);
    of.map_entities(&mut mapping);
    assert_eq!(of, ModifierOf(ours));
}
