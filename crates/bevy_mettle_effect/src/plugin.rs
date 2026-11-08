use bevy::ecs::schedule::{InternedScheduleLabel, ScheduleLabel};
use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;

use crate::code::EffectCode;
use crate::cue::{self, CueSystems};
use crate::{EffectRegistry, net, systems};

/// The system set active effects tick in, in the plugin's schedule.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectSystems;

/// Ticks active effects: their requirements, their periods, and their time.
///
/// Their requirements weighed, periods come round, time run out; and gives them
/// their new definitions as the [`EffectRegistry`] is replaced. Starts with an
/// empty registry; the game inserts one built from its data.
pub struct EffectPlugin {
    schedule: InternedScheduleLabel,
}

impl EffectPlugin {
    /// Ticks effects in `schedule`.
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        Self {
            schedule: schedule.intern(),
        }
    }

    /// The schedule it ticks effects in.
    #[must_use]
    pub fn schedule(&self) -> InternedScheduleLabel {
        self.schedule
    }
}

impl Default for EffectPlugin {
    /// Ticks effects in `FixedUpdate`, by the fixed step.
    fn default() -> Self {
        Self::new(FixedUpdate)
    }
}

impl Plugin for EffectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<EffectRegistry>()
            .init_resource::<EffectCode>()
            .init_resource::<NetRole>()
            .init_resource::<net::Predictions>()
            .init_resource::<systems::DueEffects>()
            .add_systems(
                self.schedule,
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
                    .in_set(EffectSystems),
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
}
