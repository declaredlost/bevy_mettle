//! The facade's plugins: `MettlePlugins` adds every part, and
//! `MettleSystems` orders them where they share a schedule.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{Strength, attributes, close, mettle};

/// The buff, as code names it.
#[derive(Effect)]
struct Buff;

/// A buff to `Strength` for a few ticks.
fn buff() -> EffectData {
    EffectData::lasting("buff", 0.05)
        .modifier(EffectModifier::new::<Strength>(Operation::Flat, 5.0))
}

/// `app`'s effect registry, of `buff`: checked against what `app` has.
fn registry(app: &App) -> EffectRegistry {
    let world = app.world();
    world
        .resource::<EffectRegistry>()
        .recheck(
            &[buff()],
            EffectChecks {
                tags: &TagRegistry::default(),
                curves: &CurveRegistry::default(),
                attributes: world.resource(),
                executions: world.resource(),
                calculations: world.resource(),
                routes: world.resource(),
            },
        )
        .unwrap()
}

/// Each tick, after mettle: whether the actor's buffed, and its strength.
#[derive(Resource, Default)]
struct Seen(Vec<(bool, f32)>);

fn see(actors: Query<(&Current<Strength>, Option<&ActiveEffects>)>, mut seen: ResMut<Seen>) {
    for (strength, effects) in &actors {
        let buffed = effects.is_some_and(|effects| effects.iter().next().is_some());
        seen.0.push((buffed, strength.get()));
    }
}

#[test]
fn mettle_plugins_adds_every_part() {
    let app = TestApp::builder()
        .plugins(mettle())
        .with(attributes)
        .build();
    let app = app.app();
    assert!(app.is_plugin_added::<AttributePlugin>());
    assert!(app.is_plugin_added::<EffectPlugin>());
    assert!(app.is_plugin_added::<AbilityPlugin>());
    assert!(app.is_plugin_added::<bevy_mettle::MettleSystemsPlugin>());
}

#[test]
fn effects_tick_before_attributes_fold_in_a_shared_schedule() {
    let mut app = TestApp::builder()
        .plugins(mettle().set(EffectPlugin::new(FixedPreUpdate)))
        .with(attributes)
        .with(|app| {
            app.add_effect::<Buff>();
            let registry = registry(app);
            app.insert_resource(registry)
                .init_resource::<Seen>()
                .add_systems(FixedPreUpdate, see.after(MettleSystems));
        })
        .build();
    let actor = app.spawn(Base::<Strength>::default());
    app.tick();
    app.world_mut().apply_effect(actor, Buff);
    app.secs(0.2);
    let seen = &app.resource::<Seen>().0;
    assert!(seen.iter().any(|&(buffed, _)| buffed), "buffed for a while");
    assert!(seen.last().is_some_and(|&(buffed, _)| !buffed), "then not");
    for &(buffed, strength) in seen {
        let expected = if buffed { 15.0 } else { 10.0 };
        assert!(
            close(strength, expected),
            "folded the tick it changed: {seen:?}"
        );
    }
}

#[cfg(feature = "lightyear")]
#[test]
fn mettle_plugins_carries_mettle_over_lightyear_after_its_plugins() {
    use core::time::Duration;

    use bevy::state::app::StatesPlugin;
    use lightyear::prelude::server::ServerPlugins;

    let app = TestApp::builder()
        .plugins((
            StatesPlugin,
            ServerPlugins {
                tick_duration: Duration::from_millis(10),
            },
        ))
        .plugins(MettlePlugins)
        .with(|app| {
            app.insert_resource(NetRole::Server);
        })
        .build();
    assert!(app.app().is_plugin_added::<MettleNetPlugin>());
}
