//! What's told: `Current` written, `Changed<Current<A>>`, and entity events for
//! attributes and owned tags.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{AttackSpeed, Damage, FIRE, Life, Resistance, actor, app, id, own, set_base};

/// Records everything these tests ask about.
fn watch(app: &mut TestApp) {
    app.watch_triggered::<AttributeChanged<Damage>>();
    app.watch_triggered::<AttributeChanged<AttackSpeed>>();
    app.watch_triggered::<AttributeChanged<Resistance>>();
    app.watch_triggered::<TagAdded>();
    app.watch_triggered::<TagRemoved>();
}

/// What was told since last asked, each as it reads, sorted.
fn told(app: &mut TestApp) -> Vec<String> {
    let changed = |name: &str, old: f32, new: f32| format!("{name} {old} -> {new}");
    let mut heard: Vec<String> = app
        .read_triggered::<AttributeChanged<Damage>>()
        .into_iter()
        .map(|event| changed("damage", event.old, event.new))
        .chain(
            app.read_triggered::<AttributeChanged<AttackSpeed>>()
                .into_iter()
                .map(|event| changed("attack_speed", event.old, event.new)),
        )
        .chain(
            app.read_triggered::<AttributeChanged<Resistance>>()
                .into_iter()
                .map(|event| changed("resistance", event.old, event.new)),
        )
        .chain(
            app.read_triggered::<TagAdded>()
                .into_iter()
                .map(|event| format!("+{}", event.tag)),
        )
        .chain(
            app.read_triggered::<TagRemoved>()
                .into_iter()
                .map(|event| format!("-{}", event.tag)),
        )
        .collect();
    heard.sort();
    heard
}

#[test]
fn a_change_is_told_once_with_what_it_was() {
    let mut app = app();
    watch(&mut app);
    let npc = actor(&mut app, [Modifier::flat(id::<Damage>(), 5.0)]);
    assert!(told(&mut app).is_empty(), "its first value isn't a change");
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
            Modifier::flat(id::<Resistance>(), 0.25),
            Modifier::flat(id::<Resistance>(), 0.25).read_tagged(FIRE),
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
        [Modifier::multiplicative(id::<AttackSpeed>(), 1.0).owner_has("state.moving")],
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
    let npc = actor(&mut app, [Modifier::flat(id::<Damage>(), 5.0)]);
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
    assert_eq!(app.entity(npc).get::<Current<Damage>>().get(), 25.0);
}

#[test]
fn current_starts_at_the_base_spawned_with() {
    let mut app = app();
    watch(&mut app);
    let npc = app.spawn(Base::<Life>::new(30.0));
    assert_eq!(
        app.entity(npc).get::<Current<Life>>().get(),
        30.0,
        "before it's worked out"
    );
    app.tick();
    assert_eq!(app.entity(npc).get::<Current<Life>>().get(), 30.0);
    assert!(told(&mut app).is_empty(), "nothing changed");
}

#[test]
fn removing_a_tag_not_owned_tells_nothing_and_keeps_the_rest() {
    let mut app = app();
    watch(&mut app);
    let npc = actor(&mut app, []);
    own(&mut app, npc, "immune.stun", true);
    app.tick();
    told(&mut app);
    own(&mut app, npc, "immune.freeze", false);
    own(&mut app, npc, "state.moving", false);
    app.tick();
    assert!(told(&mut app).is_empty(), "nothing went");
    let owned = app.entity(npc).get::<OwnedTags>();
    assert!(owned.contains("immune"));
    assert!(owned.contains("immune.stun"));
}
