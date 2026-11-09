//! Bases following their limits: kept within them as they drop, or kept at
//! the same fraction of their maximum.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{Life, MaxLife, actor, app, get};

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
    modify(&mut app, mage, [Modifier::flat::<MaxLife>(-20.0)]);
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
    modify(&mut app, mage, [Modifier::flat::<MaxWard>(100.0)]);
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
    modify(&mut app, mage, [Modifier::flat::<MaxLife>(-20.0)]);
    assert_eq!(
        base::<Life>(&app, mage),
        50.0,
        "what the server serves moves"
    );
}
