use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::schedule::{self, Mettle, MettleSet};

use crate::code::{CalculationRegistry, ExecutionRegistry, RouteRegistry};
use crate::context::ContextDataRegistry;
use crate::cue::{self, CueSystems};
use crate::{EffectRegistry, engine, net, systems};

/// Ticks active effects: their requirements, their periods, and their time.
///
/// Their requirements weighed, periods come round, time run out, in
/// [`MettleSet::Effects`]; what reads other actors' attributes live settled in
/// [`MettleSet::Settle`]; and gives them their new definitions as the
/// [`EffectRegistry`] is replaced. Starts with an empty registry; the game
/// inserts one built from its data.
#[derive(Default)]
pub struct EffectPlugin;

impl Plugin for EffectPlugin {
    fn build(&self, app: &mut App) {
        schedule::ensure(app);
        app.init_resource::<EffectRegistry>()
            .init_resource::<ExecutionRegistry>()
            .init_resource::<CalculationRegistry>()
            .init_resource::<RouteRegistry>()
            .init_resource::<ContextDataRegistry>()
            .init_resource::<NetRole>()
            .init_resource::<net::Predictions>()
            .init_resource::<systems::DueEffects>()
            .add_systems(
                Mettle,
                (
                    (
                        net::show_replicas
                            .run_if(resource_equals(NetRole::Client))
                            .run_if(net::replicas_to_show),
                        systems::relink.run_if(resource_changed::<EffectRegistry>),
                        systems::advance,
                        systems::settle.run_if(systems::due),
                        net::sync_replicas.run_if(resource_equals(NetRole::Server)),
                    )
                        .chain()
                        .in_set(MettleSet::Effects),
                    engine::settle_live.in_set(MettleSet::Settle),
                ),
            )
            .init_resource::<cue::CueHandlers>()
            .init_resource::<cue::Predicted>()
            .add_systems(
                PostUpdate,
                (
                    cue::play.run_if(resource_equals(NetRole::Client)),
                    cue::prune.run_if(resource_equals(NetRole::Server)),
                    cue::show,
                )
                    .chain()
                    .in_set(CueSystems),
            );
    }

    fn finish(&self, app: &mut App) {
        // Every type registered as context data, numbered once they all are.
        let registry = app
            .world()
            .get_resource::<AppTypeRegistry>()
            .map(|types| ContextDataRegistry::of(&types.read()))
            .unwrap_or_default();
        app.insert_resource(registry);
    }
}
