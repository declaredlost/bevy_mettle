use bevy::ecs::schedule::{InternedScheduleLabel, ScheduleLabel};
use bevy::prelude::*;
use bevy_mettle_core::tag::{OwnedTags, publish_tags};

use crate::authority::{self, Remote};
use crate::modifier::{ModifierOf, ModifierSources, Modifiers, Provisional};
use crate::registry::{AttributeRegistry, AttributeState, state};

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
pub(crate) struct AttributeSchedule(pub(crate) InternedScheduleLabel);

/// Keeps actors' attributes: bases synced, modifiers folded, values computed.
///
/// What changed is told, and its components registered for reflection. Added by
/// [`add_attribute`](crate::registry::AttributeApp::add_attribute) if it isn't
/// yet; add it first to choose the schedule.
pub struct AttributePlugin {
    schedule: InternedScheduleLabel,
}

impl AttributePlugin {
    /// Keeps attributes in `schedule`.
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        Self {
            schedule: schedule.intern(),
        }
    }

    /// The schedule it keeps attributes in.
    #[must_use]
    pub fn schedule(&self) -> InternedScheduleLabel {
        self.schedule
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
        app.register_type::<Modifiers>()
            .register_type::<ModifierOf>()
            .register_type::<ModifierSources>()
            .register_type::<OwnedTags>()
            .register_type::<AttributeState>()
            .register_type::<Provisional>()
            .register_type::<Remote>()
            .init_resource::<AttributeRegistry>()
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
                    authority::sync_remote.in_set(Phase::Sync),
                    (state::collect, state::fold).chain().in_set(Phase::Fold),
                    (publish_tags, state::refresh)
                        .chain()
                        .in_set(Phase::Refresh),
                ),
            );
    }
}
