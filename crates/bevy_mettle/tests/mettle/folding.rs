//! Folding: sources added, changed, moved and taken away.

use bevy_mettle::prelude::*;

use crate::common::{Damage, FLOWN, Life, MaxResistance, Strength, actor, app, close, get, value};

fn flat(value: f32) -> Modifiers {
    Modifiers(vec![Modifier::flat::<Damage>(value)])
}

#[test]
fn equipping_and_unequipping_a_source_changes_the_attributes() {
    let mut app = app();
    let npc = actor(&mut app, []);
    let ring = app.spawn((ModifierOf(npc), flat(5.0)));
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 15.0), "equipped");
    app.entity_mut(ring).remove::<ModifierOf>();
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 10.0), "taken off");
}

#[test]
fn despawning_a_source_takes_its_modifiers_away() {
    let mut app = app();
    let npc = actor(&mut app, []);
    let buff = app.spawn((
        ModifierOf(npc),
        Modifiers(vec![Modifier::additive::<Damage>(1.0)]),
    ));
    let ring = app.spawn((ModifierOf(npc), flat(5.0)));
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 30.0));
    app.entity_mut(buff).despawn();
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 15.0), "one gone");
    app.entity_mut(ring).despawn();
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 10.0), "the last gone");
}

#[test]
fn changing_or_removing_a_sources_modifiers_refolds() {
    let mut app = app();
    let npc = actor(&mut app, []);
    let ring = app.spawn((ModifierOf(npc), flat(5.0)));
    app.tick();
    app.world_mut().get_mut::<Modifiers>(ring).unwrap().0[0] = Modifier::flat::<Damage>(9.0);
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 19.0), "changed");
    app.entity_mut(ring).remove::<Modifiers>();
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 10.0), "removed");
}

#[test]
fn a_source_moved_to_another_actor_counts_for_it_instead() {
    let mut app = app();
    let hero = actor(&mut app, []);
    let foe = actor(&mut app, []);
    let ring = app.spawn((ModifierOf(hero), flat(5.0)));
    app.tick();
    app.entity_mut(ring).insert(ModifierOf(foe));
    app.tick();
    assert!(close(get::<Damage>(&app, hero), 10.0));
    assert!(close(get::<Damage>(&app, foe), 15.0));
}

#[test]
fn an_actors_own_modifiers_count_and_refold_as_they_change() {
    let mut app = app();
    let npc = actor(&mut app, [Modifier::flat::<Damage>(1.0)]);
    assert!(close(get::<Damage>(&app, npc), 11.0));
    app.entity_mut(npc).insert(flat(3.0));
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 13.0));
    app.entity_mut(npc).remove::<Modifiers>();
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 10.0));
}

#[test]
fn an_actor_with_nothing_has_its_bases() {
    let mut app = app();
    let npc = actor(&mut app, []);
    assert!(close(get::<Damage>(&app, npc), 10.0));
    assert!(close(get::<MaxResistance>(&app, npc), 0.75));
}

#[test]
fn a_modifier_of_an_attribute_not_added_is_skipped() {
    #[derive(Attribute)]
    struct Focus;
    let mut app = app();
    let npc = actor(
        &mut app,
        [Modifier::flat::<Focus>(5.0), Modifier::flat::<Damage>(1.0)],
    );
    assert!(close(get::<Damage>(&app, npc), 11.0));
}

#[test]
fn a_source_that_is_an_actor_too_refolds_itself_as_its_modifiers_go() {
    let mut app = app();
    let owner = actor(&mut app, []);
    let pet = actor(&mut app, [Modifier::flat::<Damage>(5.0)]);
    app.entity_mut(pet).insert(ModifierOf(owner));
    app.tick();
    assert!(close(get::<Damage>(&app, pet), 15.0));
    assert!(
        close(get::<Damage>(&app, owner), 15.0),
        "the pet buffs its owner"
    );
    app.entity_mut(pet).remove::<Modifiers>();
    app.tick();
    assert!(close(get::<Damage>(&app, owner), 10.0));
    assert!(close(get::<Damage>(&app, pet), 10.0), "and itself no more");
}

#[test]
fn a_modifier_whose_condition_cant_be_is_skipped() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [Modifier::flat::<Damage>(5.0).condition(Condition::fact_ramp(FLOWN, 5.0, 5.0))],
    );
    let read = AttributeContext::new().fact(FLOWN, 5.0);
    assert!(close(value::<Damage>(&app, npc, &read), 10.0), "not NaN");
}

#[test]
fn a_base_or_an_override_per_something_is_skipped() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::base::<Damage>(10.0).per::<Strength>(100.0),
            Modifier::overriding::<Life>(1.0).per::<Strength>(100.0),
        ],
    );
    assert!(close(get::<Damage>(&app, npc), 10.0), "not replaced by 0");
    assert!(close(get::<Life>(&app, npc), 50.0));
}

#[test]
fn of_overrides_the_one_applied_last_counts_whatever_came_first() {
    let mut app = app();
    let npc = actor(&mut app, []);
    let ring = app.spawn((
        ModifierOf(npc),
        Modifiers(vec![Modifier::overriding::<Damage>(2.0)]),
    ));
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 2.0));
    // Its own, added after the ring was put on.
    app.entity_mut(npc).remove::<Modifiers>();
    app.tick();
    app.entity_mut(npc)
        .insert(Modifiers(vec![Modifier::overriding::<Damage>(1.0)]));
    app.tick();
    assert!(
        close(get::<Damage>(&app, npc), 1.0),
        "its own, applied after the ring"
    );
    app.entity_mut(ring).insert(ModifierOf(npc));
    app.tick();
    assert!(
        close(get::<Damage>(&app, npc), 2.0),
        "the ring put on again"
    );
}

#[test]
fn a_remote_actor_shows_what_it_is_served_but_while_it_predicts() {
    let mut app = app();
    let npc = actor(&mut app, []);
    app.entity_mut(npc)
        .insert((Remote, Served::<Damage>::new(42.0)));
    app.tick();
    let current =
        |app: &bevy_harness::TestApp| app.world().get::<Current<Damage>>(npc).unwrap().get();
    assert!(close(get::<Damage>(&app, npc), 42.0), "served");
    assert!(close(current(&app), 42.0));
    let read = AttributeContext::new().tag("damage");
    assert!(
        close(value::<Damage>(&app, npc, &read), 10.0),
        "a read in context is worked out here"
    );
    let guess = app.spawn((
        ModifierOf(npc),
        Modifiers(vec![Modifier::flat::<Damage>(5.0)]),
        Provisional,
    ));
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 15.0), "the prediction");
    assert!(close(current(&app), 15.0));
    app.entity_mut(guess).despawn();
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 42.0), "served again");
    assert!(close(current(&app), 42.0));
}
