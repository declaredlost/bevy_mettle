//! The facade's plugins: `MettlePlugins` adds every part, run in the
//! `Mettle` schedule, run where `MettleCorePlugin` says.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{Strength, attributes, close, mettle};

/// The buff, as code names it.
#[derive(Effect)]
struct Buff;

/// A buff to `Strength` for a few ticks.
fn buff() -> EffectData {
    EffectData::lasting(Key::of::<Buff>(), 0.05)
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
    assert!(app.is_plugin_added::<MettleCorePlugin>());
}

#[test]
fn effects_tick_before_attributes_fold_in_the_same_step() {
    let mut app = TestApp::builder()
        .plugins(mettle())
        .with(attributes)
        .with(|app| {
            app.add_effect::<Buff>();
            let registry = registry(app);
            app.insert_resource(registry)
                .init_resource::<Seen>()
                .add_systems(FixedPostUpdate, see.after(MettleSystems));
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

/// Each tick, before mettle: the actor's strength.
#[derive(Resource, Default)]
struct Before(Vec<f32>);

fn before(actors: Query<&Current<Strength>>, mut seen: ResMut<Before>) {
    seen.0.extend(actors.iter().map(Current::get));
}

#[test]
fn mettle_runs_where_its_core_plugin_says() {
    let mut app = TestApp::builder()
        .plugins(mettle().set(MettleCorePlugin::new(Update)))
        .with(attributes)
        .with(|app| {
            app.add_effect::<Buff>();
            let registry = registry(app);
            app.insert_resource(registry)
                .init_resource::<Seen>()
                .init_resource::<Before>()
                .add_systems(
                    Update,
                    (before.before(MettleSystems), see.after(MettleSystems)),
                );
        })
        .build();
    let actor = app.spawn(Base::<Strength>::default());
    app.tick();
    app.world_mut().apply_effect(actor, Buff);
    app.tick();
    // In `Update`, between them: before it, not yet; after it, folded.
    let before = &app.resource::<Before>().0;
    assert!(
        before.last().is_some_and(|&strength| close(strength, 10.0)),
        "not folded before it: {before:?}"
    );
    let seen = &app.resource::<Seen>().0;
    assert!(
        seen.last()
            .is_some_and(|&(buffed, strength)| buffed && close(strength, 15.0)),
        "folded after it: {seen:?}"
    );
}

#[test]
fn mettle_core_plugin_added_again_for_the_same_schedule_is_nothing() {
    let app = TestApp::builder()
        .plugins((MettleCorePlugin::default(), mettle()))
        .with(attributes)
        .build();
    assert!(app.app().is_plugin_added::<MettleCorePlugin>());
}

#[test]
#[should_panic(expected = "mettle already runs in")]
fn mettle_core_plugin_added_late_for_another_schedule_says_why() {
    let mut app = App::new();
    app.add_plugins(EffectPlugin)
        .add_plugins(MettleCorePlugin::new(Update));
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
