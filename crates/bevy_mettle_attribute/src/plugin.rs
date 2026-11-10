use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::schedule::{self, Mettle, MettleSet};
use bevy_mettle_core::tag::{OwnedTags, publish_tags};

use crate::authority::{self, Remote};
use crate::modifier::{ModifierOf, ModifierSources, Modifiers, ModifiersReplica, Provisional};
use crate::registry::{AttributeRegistry, AttributeState, state};

/// The schedule attributes are folded in, from bases synced to limits.
///
/// Bases synced, modifiers folded, values worked out, bases following their
/// limits.
///
/// Run in [`MettleSet::Attributes`], and again as what follows other actors'
/// values settles ([`MettleSet::Settle`]), so what that changed is folded
/// before values are published.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AttributeFold;

/// The steps of [`AttributeFold`], in order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Phase {
    /// Changed bases synced in.
    Sync,
    /// Changed modifiers folded.
    Fold,
    /// Owned tags told, values worked out.
    Refresh,
    /// Bases moved as their limits did, and their values worked out again.
    Follow,
}

/// Keeps actors' attributes: bases synced, modifiers folded, values computed.
///
/// Folded in [`AttributeFold`], run in [`MettleSet::Attributes`]; published
/// in [`MettleSet::Publish`]. What changed is told, and its components
/// registered for reflection. Added by
/// [`add_attribute`](crate::registry::AttributeApp::add_attribute) if it isn't
/// yet.
#[derive(Default)]
pub struct AttributePlugin;

impl Plugin for AttributePlugin {
    fn build(&self, app: &mut App) {
        schedule::ensure(app);
        app.register_type::<Modifiers>()
            .register_type::<ModifierOf>()
            .register_type::<ModifiersReplica>()
            .register_type::<ModifierSources>()
            .register_type::<OwnedTags>()
            .register_type::<AttributeState>()
            .register_type::<Provisional>()
            .register_type::<Remote>()
            .init_resource::<AttributeRegistry>()
            .init_resource::<NetRole>()
            .init_resource::<state::Dirty>()
            .init_schedule(AttributeFold)
            .configure_sets(
                AttributeFold,
                (Phase::Sync, Phase::Fold, Phase::Refresh, Phase::Follow).chain(),
            )
            .add_systems(
                AttributeFold,
                (
                    (
                        authority::sync_remote,
                        crate::modifier::sync_replicas,
                        crate::modifier::show_replicas,
                    )
                        .in_set(Phase::Sync),
                    (state::collect, state::fold).chain().in_set(Phase::Fold),
                    (publish_tags, state::refresh)
                        .chain()
                        .in_set(Phase::Refresh),
                    // After every attribute's bases followed their limits.
                    state::rework.after(Phase::Follow),
                ),
            )
            .add_systems(Mettle, fold.in_set(MettleSet::Attributes));
    }
}

/// Runs [`AttributeFold`].
fn fold(world: &mut World) {
    world.run_schedule(AttributeFold);
}
