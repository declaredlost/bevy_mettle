use bevy::ecs::schedule::InternedScheduleLabel;
use bevy::prelude::*;

/// Every part's systems in each schedule: abilities, effects, then attributes.
///
/// In each schedule they run in: abilities, then effects, then attributes,
/// where they run in the same one.
///
/// By default each runs in its own: abilities in `PreUpdate`, attributes in
/// `FixedPreUpdate`, effects in `FixedUpdate`. Put what reads an actor's
/// attributes or effects as they stand after it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MettleSystems;

/// Puts every part's systems in [`MettleSystems`], in order, in each schedule.
///
/// In each schedule they were given. In
/// [`MettlePlugins`](crate::MettlePlugins); add it yourself if you add the
/// parts' plugins one by one.
///
/// It looks at the parts' plugins once all plugins are added, so they can be
/// added in any order.
#[derive(Default)]
pub struct MettleSystemsPlugin;

impl Plugin for MettleSystemsPlugin {
    fn build(&self, _: &mut App) {}

    fn finish(&self, app: &mut App) {
        let abilities = abilities(app);
        let effects = effects(app, &abilities);
        attributes(app, &abilities, &effects);
    }
}

/// Puts abilities in [`MettleSystems`]: the schedules they run in.
fn abilities(app: &mut App) -> Vec<InternedScheduleLabel> {
    #[cfg(feature = "ability")]
    {
        use bevy::ecs::schedule::ScheduleLabel;
        use bevy_mettle_ability::{AbilityPlugin, AbilitySystems};

        if app.is_plugin_added::<AbilityPlugin>() {
            app.configure_sets(PreUpdate, AbilitySystems.in_set(MettleSystems));
            return vec![PreUpdate.intern()];
        }
    }
    let _ = app;
    Vec::new()
}

/// Puts effects in [`MettleSystems`], after abilities where they run in the
/// same schedule, one of `abilities`: what an ability applies ticks the same
/// tick.
fn effects(app: &mut App, abilities: &[InternedScheduleLabel]) -> Vec<InternedScheduleLabel> {
    #[cfg(feature = "effect")]
    {
        use bevy_mettle_effect::{EffectPlugin, EffectSystems};

        let schedules: Vec<_> = app
            .get_added_plugins::<EffectPlugin>()
            .iter()
            .map(|plugin| plugin.schedule())
            .collect();
        let _ = abilities;
        for &schedule in &schedules {
            app.configure_sets(schedule, EffectSystems.in_set(MettleSystems));
            #[cfg(feature = "ability")]
            if abilities.contains(&schedule) {
                app.configure_sets(
                    schedule,
                    bevy_mettle_ability::AbilitySystems.before(EffectSystems),
                );
            }
        }
        schedules
    }
    #[cfg(not(feature = "effect"))]
    {
        let _ = (app, abilities);
        Vec::new()
    }
}

/// Puts attributes in [`MettleSystems`], after abilities and effects where
/// they run in the same schedule: what they changed folds the same tick.
fn attributes(
    app: &mut App,
    abilities: &[InternedScheduleLabel],
    effects: &[InternedScheduleLabel],
) {
    #[cfg(feature = "attribute")]
    {
        use bevy_mettle_attribute::{AttributePlugin, AttributeSystems};

        let schedules: Vec<_> = app
            .get_added_plugins::<AttributePlugin>()
            .iter()
            .map(|plugin| plugin.schedule())
            .collect();
        for schedule in schedules {
            app.configure_sets(schedule, AttributeSystems.in_set(MettleSystems));
            #[cfg(feature = "ability")]
            if abilities.contains(&schedule) {
                app.configure_sets(
                    schedule,
                    bevy_mettle_ability::AbilitySystems.before(AttributeSystems),
                );
            }
            #[cfg(feature = "effect")]
            if effects.contains(&schedule) {
                app.configure_sets(
                    schedule,
                    bevy_mettle_effect::EffectSystems.before(AttributeSystems),
                );
            }
        }
    }
    let _ = (app, abilities, effects);
}
