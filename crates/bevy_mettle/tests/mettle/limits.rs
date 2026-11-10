//! Bases following their limits: kept within them as they drop, or kept at
//! the same fraction of their maximum.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{Life, MaxLife, Strength, actor, app, get, id};

/// A ward kept at the same fraction of its maximum.
#[derive(Attribute)]
#[attribute(base = 50.0, min = 0.0, max = MaxWard, on_limit = Ratio)]
struct Ward;

#[derive(Attribute)]
#[attribute(base = 100.0)]
struct MaxWard;

/// `actor`'s `A`'s base.
fn base<A: Attribute>(app: &TestApp, actor: Entity) -> f32 {
    app.entity(actor).get::<Base<A>>().get()
}

/// Changes `actor`'s own modifiers to `modifiers`, and works them out.
fn modify(app: &mut TestApp, actor: Entity, modifiers: impl IntoIterator<Item = Modifier>) {
    app.entity_mut(actor)
        .insert(modifiers.into_iter().collect::<Modifiers>());
    app.tick();
}

#[test]
fn a_base_comes_down_with_its_limit_and_stays_down() {
    let mut app = app();
    let mage = actor(&mut app, []);
    modify(&mut app, mage, [Modifier::flat(id::<MaxLife>(), -20.0)]);
    assert_eq!(
        base::<Life>(&app, mage),
        30.0,
        "clamped the frame it dropped"
    );
    assert_eq!(get::<Life>(&app, mage), 30.0);
    modify(&mut app, mage, []);
    assert_eq!(get::<MaxLife>(&app, mage), 50.0);
    assert_eq!(get::<Life>(&app, mage), 30.0, "not given back");
}

#[test]
fn a_base_on_ratio_keeps_its_fraction_of_its_maximum() {
    let mut app = TestApp::builder()
        .with(|app| {
            app.add_attribute::<Ward>().add_attribute::<MaxWard>();
        })
        .build();
    let mage = app.spawn((Base::<Ward>::default(), Base::<MaxWard>::default()));
    app.tick();
    let max_ward = app.world().attribute_id::<MaxWard>().unwrap();
    modify(&mut app, mage, [Modifier::flat(max_ward, 100.0)]);
    assert_eq!(base::<Ward>(&app, mage), 100.0, "half of 200");
    assert_eq!(get::<Ward>(&app, mage), 100.0, "the frame it rose");
    modify(&mut app, mage, []);
    assert_eq!(get::<Ward>(&app, mage), 50.0, "half of 100");
}

#[test]
fn a_remote_actors_base_is_left_to_its_authority() {
    let mut app = app();
    let mage = actor(&mut app, []);
    app.entity_mut(mage).insert(Remote);
    app.tick();
    modify(&mut app, mage, [Modifier::flat(id::<MaxLife>(), -20.0)]);
    assert_eq!(
        base::<Life>(&app, mage),
        50.0,
        "what the server serves moves"
    );
}

#[test]
fn a_base_changed_is_kept_within_limits_changed_the_same_frame() {
    let mut app = app();
    // Its most gains one per strength: what keeping life reads, through it.
    let mage = actor(
        &mut app,
        [Modifier::flat(id::<MaxLife>(), 1.0).per(id::<Strength>(), 1.0)],
    );
    app.tick();
    let life = id::<Life>();
    // Its most's base set, not yet synced: a change to life reads it anyway.
    app.entity_mut(mage).insert(Base::<MaxLife>::new(80.0));
    assert!(
        app.world_mut()
            .apply_to_base(mage, life, Operation::Flat, 1000.0)
    );
    assert_eq!(base::<Life>(&app, mage), 90.0, "80, and 10 for strength");
    // What its most is worked out through, set and not synced: read too.
    app.entity_mut(mage).insert(Base::<Strength>::new(30.0));
    assert!(
        app.world_mut()
            .apply_to_base(mage, life, Operation::Flat, 1000.0)
    );
    assert_eq!(base::<Life>(&app, mage), 110.0, "80, and 30 for strength");
}
