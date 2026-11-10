//! Folding: sources added, changed, moved and taken away.

use bevy_mettle::core::registry::AttributeId;
use bevy_mettle::prelude::*;

use crate::common::{
    Damage, FLOWN, Life, MaxLife, MaxResistance, Strength, actor, app, close, get, id, own,
    set_base, value,
};

fn flat(value: f32) -> Modifiers {
    Modifiers(vec![Modifier::flat(id::<Damage>(), value)])
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
        Modifiers(vec![Modifier::additive(id::<Damage>(), 1.0)]),
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
    app.world_mut().get_mut::<Modifiers>(ring).unwrap().0[0] = Modifier::flat(id::<Damage>(), 9.0);
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
    let npc = actor(&mut app, [Modifier::flat(id::<Damage>(), 1.0)]);
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
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::flat(AttributeId::new(999), 5.0),
            Modifier::flat(id::<Damage>(), 1.0),
        ],
    );
    assert!(close(get::<Damage>(&app, npc), 11.0));
}

#[test]
fn a_source_that_is_an_actor_too_refolds_itself_as_its_modifiers_go() {
    let mut app = app();
    let owner = actor(&mut app, []);
    let pet = actor(&mut app, [Modifier::flat(id::<Damage>(), 5.0)]);
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
        [Modifier::flat(id::<Damage>(), 5.0).condition(Condition::fact_ramp(FLOWN, 5.0, 5.0))],
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
            Modifier::base(id::<Damage>(), 10.0).per(id::<Strength>(), 100.0),
            Modifier::overriding(id::<Life>(), 1.0).per(id::<Strength>(), 100.0),
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
        Modifiers(vec![Modifier::overriding(id::<Damage>(), 2.0)]),
    ));
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 2.0));
    // Its own, added after the ring was put on.
    app.entity_mut(npc).remove::<Modifiers>();
    app.tick();
    app.entity_mut(npc)
        .insert(Modifiers(vec![Modifier::overriding(id::<Damage>(), 1.0)]));
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
    let current = |app: &bevy_harness::TestApp| app.entity(npc).get::<Current<Damage>>().get();
    assert!(close(get::<Damage>(&app, npc), 42.0), "served");
    assert!(close(current(&app), 42.0));
    // A read in a context nothing of it counts by is the served value.
    let read = AttributeContext::new().tag("damage");
    assert!(
        close(value::<Damage>(&app, npc, &read), 42.0),
        "nothing counts by the context: served"
    );
    // One a modifier counts by is worked out here: the server serves values
    // with nothing in play.
    let tagged = app.spawn((
        ModifierOf(npc),
        Modifiers(vec![
            Modifier::flat(id::<Damage>(), 3.0).read_tagged("damage"),
        ]),
    ));
    app.tick();
    assert!(
        close(value::<Damage>(&app, npc, &read), 13.0),
        "a read in context is worked out here"
    );
    assert!(close(get::<Damage>(&app, npc), 42.0), "still served");
    app.entity_mut(tagged).despawn();
    app.tick();
    let guess = app.spawn((
        ModifierOf(npc),
        Modifiers(vec![Modifier::flat(id::<Damage>(), 5.0)]),
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

#[test]
fn modifiers_named_by_type_are_put_on_by_number() {
    #[derive(Attribute)]
    struct Focus;
    let mut app = app();
    let npc = actor(&mut app, []);
    app.world_mut().entity_mut(npc).modifiers([
        ModifierSpec::flat::<Damage>(5.0),
        ModifierSpec::flat::<Damage>(1.0).per::<Strength>(5.0),
        // Not added: skipped, with a warning.
        ModifierSpec::flat::<Focus>(1.0),
    ]);
    app.tick();
    // 10 + 5 + 1 per 5 of 10 strength.
    assert!(close(get::<Damage>(&app, npc), 17.0));
    let modifiers = app.entity(npc).get::<Modifiers>();
    assert_eq!(modifiers.0.len(), 2);
    assert!(
        modifiers
            .0
            .iter()
            .all(|modifier| modifier.attribute == id::<Damage>())
    );

    // As a command, too.
    app.world_mut()
        .commands()
        .entity(npc)
        .modifiers([ModifierSpec::overriding::<Damage>(3.0)]);
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 3.0));
}

#[test]
fn a_read_in_context_counts_what_its_value_is_worked_out_through() {
    let mut app = app();
    // Life over its most, a most that's more in `damage` reads; damage per
    // strength, a strength that's more in `damage` reads too.
    let npc = actor(
        &mut app,
        [
            Modifier::flat(id::<Life>(), 30.0),
            Modifier::flat(id::<MaxLife>(), 40.0).read_tagged("damage"),
            Modifier::flat(id::<Damage>(), 1.0).per(id::<Strength>(), 5.0),
            Modifier::flat(id::<Strength>(), 10.0).read_tagged("damage"),
        ],
    );
    app.tick();
    let read = AttributeContext::new().tag("damage");
    assert!(close(get::<Life>(&app, npc), 50.0), "kept to its most: 50");
    assert!(
        close(value::<Life>(&app, npc, &read), 80.0),
        "its most counts by the read: 90, so 80"
    );
    assert!(
        close(get::<Damage>(&app, npc), 12.0),
        "10, and 2 per strength"
    );
    assert!(
        close(value::<Damage>(&app, npc, &read), 14.0),
        "strength counts by the read: 20, so 4"
    );
    // What nothing counts by reads as cached.
    let other = AttributeContext::new().tag("state");
    assert!(close(value::<Life>(&app, npc, &other), 50.0));
    assert!(close(value::<Damage>(&app, npc, &other), 12.0));
}

#[test]
fn what_one_change_moves_is_worked_out_again_and_nothing_else_is_lost() {
    let mut app = app();
    // Life kept to its most; damage per strength; strength more while enraged.
    let npc = actor(
        &mut app,
        [
            Modifier::flat(id::<Life>(), 30.0),
            Modifier::flat(id::<Damage>(), 1.0).per(id::<Strength>(), 5.0),
            Modifier::flat(id::<Strength>(), 5.0).owner_has("state.enraged"),
        ],
    );
    app.tick();
    assert!(close(get::<Life>(&app, npc), 50.0), "kept to its most: 50");
    assert!(
        close(get::<Damage>(&app, npc), 12.0),
        "10, and 2 per strength"
    );
    // Its most's base alone: life, kept to it, follows.
    set_base::<MaxLife>(&mut app, npc, 70.0);
    app.tick();
    assert!(close(get::<Life>(&app, npc), 70.0), "its most moved: 70");
    // Strength's base alone: damage, per it, follows.
    set_base::<Strength>(&mut app, npc, 20.0);
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 14.0), "4 per strength now");
    // A tag alone: strength counts it, and damage, per strength, follows.
    own(&mut app, npc, "state.enraged", true);
    app.tick();
    assert!(close(get::<Strength>(&app, npc), 25.0), "enraged: 25");
    assert!(close(get::<Damage>(&app, npc), 15.0), "5 per strength");
    // What none of them moves is as it was.
    assert!(close(get::<Life>(&app, npc), 70.0));
    assert!(close(get::<MaxLife>(&app, npc), 70.0));
}
