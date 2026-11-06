use bevy::ecs::intern::Interned;
use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use bevy_mettle_core::tag::publish_tags;

use crate::registry::{AttributeRegistry, state};

/// The system set attributes are kept in, `FixedPreUpdate` by default.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttributeSystems;

/// The steps attributes are kept in, in order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Phase {
    /// Changed bases synced in.
    Sync,
    /// Changed modifiers folded.
    Fold,
    /// Owned tags told, values worked out.
    Refresh,
    /// `Current`s written, changes told.
    Publish,
}

/// The schedule attributes are kept in.
#[derive(Resource)]
pub(crate) struct AttributeSchedule(pub(crate) Interned<dyn ScheduleLabel>);

/// Keeps actors' attributes: bases synced, modifiers folded, values computed.
///
/// What changed is told. Added by
/// [`add_attribute`](crate::AttributeApp::add_attribute) if it isn't yet; add
/// it first to choose the schedule.
pub struct AttributePlugin {
    schedule: Interned<dyn ScheduleLabel>,
}

impl AttributePlugin {
    /// Keeps attributes in `schedule`.
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        Self {
            schedule: schedule.intern(),
        }
    }
}

impl Default for AttributePlugin {
    /// Keeps attributes in `FixedPreUpdate`, so the fixed step reads them
    /// current.
    fn default() -> Self {
        Self::new(FixedPreUpdate)
    }
}

impl Plugin for AttributePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AttributeRegistry>()
            .init_resource::<state::Dirty>()
            .insert_resource(AttributeSchedule(self.schedule))
            .configure_sets(
                self.schedule,
                (Phase::Sync, Phase::Fold, Phase::Refresh, Phase::Publish)
                    .chain()
                    .in_set(AttributeSystems),
            )
            .add_systems(
                self.schedule,
                (
                    (state::collect, state::fold).chain().in_set(Phase::Fold),
                    (publish_tags, state::refresh)
                        .chain()
                        .in_set(Phase::Refresh),
                ),
            );
    }
}
