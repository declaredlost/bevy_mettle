//! Folding: sources added, changed, moved and taken away.

use bevy_mettle::prelude::*;

use crate::common::{Damage, MaxResistance, actor, app, close, get};

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
