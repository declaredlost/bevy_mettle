//! What's told: `Current` written, `Changed<Current<A>>`, and entity
//! events for attributes and owned tags.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{AttackSpeed, Damage, FIRE, Resistance, actor, app, own, set_base};

#[derive(Resource, Default)]
struct Told(Vec<String>);

fn watch(app: &mut TestApp) {
    let world = app.world_mut();
    world.init_resource::<Told>();
    world.add_observer(
        |event: On<AttributeChanged<Damage>>, mut told: ResMut<Told>| {
            told.0
                .push(format!("damage {} -> {}", event.old, event.new));
        },
    );
    world.add_observer(
        |event: On<AttributeChanged<AttackSpeed>>, mut told: ResMut<Told>| {
            told.0
                .push(format!("attack_speed {} -> {}", event.old, event.new));
        },
    );
    world.add_observer(
        |event: On<AttributeChanged<Resistance>>, mut told: ResMut<Told>| {
            told.0
                .push(format!("resistance {} -> {}", event.old, event.new));
        },
    );
    world.add_observer(|event: On<TagAdded>, mut told: ResMut<Told>| {
        told.0.push(format!("+{}", event.tag));
    });
    world.add_observer(|event: On<TagRemoved>, mut told: ResMut<Told>| {
        told.0.push(format!("-{}", event.tag));
    });
}

fn told(app: &mut TestApp) -> Vec<String> {
    let mut heard = std::mem::take(&mut app.world_mut().resource_mut::<Told>().0);
    heard.sort();
    heard
}

#[test]
fn a_change_is_told_once_with_what_it_was() {
    let mut app = app();
    watch(&mut app);
    let npc = actor(&mut app, [Modifier::flat::<Damage>(5.0)]);
    assert_eq!(told(&mut app), ["damage 10 -> 15"]);
    app.tick();
    assert!(told(&mut app).is_empty(), "nothing changed");
    set_base::<Damage>(&mut app, npc, 20.0);
    app.tick();
    assert_eq!(told(&mut app), ["damage 15 -> 25"]);
}

#[test]
fn a_change_with_nothing_in_play_is_told_and_one_only_some_reads_see_isnt() {
    let mut app = app();
    watch(&mut app);
    let npc = actor(&mut app, []);
    told(&mut app);
    app.spawn((
        ModifierOf(npc),
        Modifiers(vec![
            Modifier::flat::<Resistance>(0.25),
            Modifier::flat::<Resistance>(0.25).read_tagged(FIRE),
        ]),
    ));
    app.tick();
    assert_eq!(told(&mut app), ["resistance 0 -> 0.25"]);
}

#[test]
fn an_owned_tag_coming_and_going_is_told_and_changes_what_counts_while_it() {
    let mut app = app();
    watch(&mut app);
    let npc = actor(
        &mut app,
        [Modifier::multiplicative::<AttackSpeed>(1.0).owner_has("state.moving")],
    );
    told(&mut app);
    own(&mut app, npc, "state.moving", true);
    app.tick();
    assert_eq!(
        told(&mut app),
        ["+state", "+state.moving", "attack_speed 1 -> 2"]
    );
    own(&mut app, npc, "state.moving", false);
    own(&mut app, npc, "state.moving", true);
    app.tick();
    assert!(told(&mut app).is_empty(), "gone and back: nothing to tell");
}

#[test]
fn a_parent_tag_is_told_only_with_its_first_and_last_child() {
    let mut app = app();
    watch(&mut app);
    let npc = actor(&mut app, []);
    told(&mut app);
    own(&mut app, npc, "immune.stun", true);
    app.tick();
    told(&mut app);
    own(&mut app, npc, "immune.freeze", true);
    app.tick();
    assert_eq!(told(&mut app), ["+immune.freeze"]);
    own(&mut app, npc, "immune.stun", false);
    own(&mut app, npc, "immune.freeze", false);
    app.tick();
    assert_eq!(
        told(&mut app),
        ["-immune", "-immune.freeze", "-immune.stun"]
    );
}

#[test]
fn current_is_written_only_when_its_value_changes() {
    let mut app = app();
    let npc = actor(&mut app, [Modifier::flat::<Damage>(5.0)]);
    let last = |app: &TestApp| {
        app.world()
            .entity(npc)
            .get_ref::<Current<Damage>>()
            .unwrap()
            .last_changed()
    };
    let folded = last(&app);
    app.tick();
    assert_eq!(last(&app), folded, "a tick with nothing new");
    own(&mut app, npc, "state.moving", true);
    app.tick();
    assert_eq!(last(&app), folded, "a tag nothing counts while");
    set_base::<Damage>(&mut app, npc, 20.0);
    app.tick();
    assert_ne!(last(&app), folded, "a value changed");
    assert_eq!(app.world().get::<Current<Damage>>(npc).unwrap().get(), 25.0);
}
