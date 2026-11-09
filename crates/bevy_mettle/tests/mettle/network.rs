//! What replication needs: modifiers written by number, a source's actor
//! mapped into another world, and a source's modifiers sent by number.

use bevy::ecs::entity::{EntityHashMap, MapEntities};
use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{Damage, attributes, id};

#[test]
fn a_modifier_is_written_by_number() {
    use serde_test::{Token, assert_tokens};

    let damage = id::<Damage>();
    let index = u32::try_from(damage.index()).unwrap();
    assert_tokens(
        &Modifier::flat(damage, 3.0).local(),
        &[
            Token::Struct {
                name: "Modifier",
                len: 6,
            },
            Token::Str("attribute"),
            Token::U32(index),
            Token::Str("op"),
            Token::UnitVariant {
                name: "Operation",
                variant: "Flat",
            },
            Token::Str("value"),
            Token::F32(3.0),
            Token::Str("condition"),
            Token::None,
            Token::Str("per"),
            Token::None,
            Token::Str("local"),
            Token::Bool(true),
            Token::StructEnd,
        ],
    );
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

/// A world playing as `role`, with the tests' attributes and the tags
/// `state` and `state.stunned`.
fn world(role: NetRole) -> TestApp {
    TestApp::builder()
        .with(attributes)
        .with(move |app| {
            let tags = ["state", "state.stunned"].into_iter().map(TagKey::named);
            app.insert_resource(role)
                .insert_resource(TagRegistry::build(tags).unwrap());
        })
        .build()
}

#[test]
fn a_sources_modifiers_go_by_number_and_come_back_the_same() {
    let damage = id::<Damage>();
    let kept = vec![
        Modifier::flat(damage, 3.0),
        Modifier::additive(damage, 0.5).owner_has("state.stunned"),
    ];
    let unknown = Modifier::flat(damage, 9.0).owner_has("state.unknown");
    let mut server = world(NetRole::Server);
    let wearer = server.spawn(Base::<Damage>::default());
    let ring = server.spawn((
        ModifierOf(wearer),
        kept.iter().cloned().chain([unknown]).collect::<Modifiers>(),
        ModifiersReplica::default(),
    ));
    server.tick();
    let stunned = server
        .resource::<TagRegistry>()
        .id("state.stunned")
        .expect("a registered tag");
    let replica = server.entity(ring).get::<ModifiersReplica>().clone();
    assert_eq!(
        replica,
        ModifiersReplica {
            of: Some(wearer),
            modifiers: vec![
                Modifier {
                    attribute: damage,
                    op: Operation::Flat,
                    value: 3.0,
                    condition: None,
                    per: None,
                    local: false,
                },
                Modifier {
                    attribute: damage,
                    op: Operation::Additive,
                    value: 0.5,
                    condition: Some(ConditionOver::OwnerHas(stunned)),
                    per: None,
                    local: false,
                },
            ],
        },
        "by number, what it's of with it, the one naming an unregistered tag left out"
    );

    // The client's own wearer, as replication would map it.
    let mut client = world(NetRole::Client);
    let worn = client.spawn(Base::<Damage>::default());
    let ring = client.spawn(ModifiersReplica {
        of: Some(worn),
        ..replica
    });
    client.tick();
    client.entity(ring).assert_eq(Modifiers(kept));
    client.entity(ring).assert_eq(ModifierOf(worn));
}

#[test]
fn a_replica_put_on_a_source_that_has_its_modifiers_fills_at_once() {
    let damage = id::<Damage>();
    let mut server = world(NetRole::Server);
    let wearer = server.spawn(Base::<Damage>::default());
    let ring = server.spawn((
        ModifierOf(wearer),
        Modifiers(vec![Modifier::flat(damage, 3.0)]),
    ));
    server.tick();
    server.entity_mut(ring).insert(ModifiersReplica::default());
    server.tick();
    let replica = server.entity(ring).get::<ModifiersReplica>().clone();
    assert_eq!(replica.of, Some(wearer));
    assert_eq!(
        replica.modifiers.len(),
        1,
        "though its modifiers didn't change"
    );
}

#[test]
fn a_source_taken_off_its_actor_is_sent_of_no_one() {
    let mut server = world(NetRole::Server);
    let wearer = server.spawn(Base::<Damage>::default());
    let ring = server.spawn((
        ModifierOf(wearer),
        Modifiers(vec![Modifier::flat(id::<Damage>(), 3.0)]),
        ModifiersReplica::default(),
    ));
    server.tick();
    server.entity_mut(ring).remove::<ModifierOf>();
    server.tick();
    assert_eq!(server.entity(ring).get::<ModifiersReplica>().of, None);

    let mut client = world(NetRole::Client);
    let worn = client.spawn(Base::<Damage>::default());
    let shown = client.spawn((
        ModifierOf(worn),
        ModifiersReplica {
            of: None,
            modifiers: Vec::new(),
        },
    ));
    client.tick();
    assert!(
        client.world().get::<ModifierOf>(shown).is_none(),
        "taken off on the client too"
    );
}
