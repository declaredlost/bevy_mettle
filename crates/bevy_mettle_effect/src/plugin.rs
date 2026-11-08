use bevy::ecs::schedule::{InternedScheduleLabel, ScheduleLabel};
use bevy::prelude::*;

use crate::code::EffectCode;
use crate::net::{self, NetRole};
use crate::{EffectRegistry, systems};

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
            .add_systems(
                self.schedule,
                (
                    net::show_replicas.run_if(resource_equals(NetRole::Client)),
                    systems::relink.run_if(resource_changed::<EffectRegistry>),
                    systems::tick,
                    net::sync_replicas.run_if(resource_equals(NetRole::Server)),
                )
                    .chain()
                    .in_set(EffectSystems),
            );
    }
}
