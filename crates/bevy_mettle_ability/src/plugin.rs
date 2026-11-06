use bevy::prelude::*;
use bevy_mettle_core::tag::{TagAdded, TagRemoved};
use bevy_mettle_effect::events::{EffectApplied, EffectRemoved};
use bevy_mettle_effect::net::NetRole;

use crate::data::Behaviours;
use crate::net::{self, AbilityReply, AbilityRequest};
use crate::{AbilityRegistry, engine};

/// Activates abilities by their triggers, and grants them with their effects.
///
/// Activated as owned tags come and go, and granted and taken away with the
/// effects that grant them. Starts with an empty registry; the game inserts one
/// built from its data.
#[derive(Default)]
pub struct AbilityPlugin;

impl Plugin for AbilityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AbilityRegistry>()
            .init_resource::<Behaviours>()
            .init_resource::<engine::Activations>()
            .init_resource::<NetRole>()
            .init_resource::<net::Pending>()
            .add_message::<AbilityRequest>()
            .add_message::<AbilityReply>()
            .add_systems(
                PreUpdate,
                (
                    net::serve.run_if(resource_equals(NetRole::Server)),
                    net::resolve.run_if(resource_equals(NetRole::Client)),
                    net::show_replicas.run_if(resource_equals(NetRole::Client)),
                ),
            )
            .add_systems(
                PostUpdate,
                net::sync_replicas.run_if(resource_equals(NetRole::Server)),
            )
            .add_observer(|event: On<TagAdded>, mut commands: Commands| {
                let (actor, tag) = (event.entity, event.tag.clone());
                commands.queue(move |world: &mut World| engine::tag_added(world, actor, &tag));
            })
            .add_observer(|event: On<TagRemoved>, mut commands: Commands| {
                let (actor, tag) = (event.entity, event.tag.clone());
                commands.queue(move |world: &mut World| engine::tag_removed(world, actor, &tag));
            })
            .add_observer(|event: On<EffectApplied>, mut commands: Commands| {
                let (actor, effect) = (event.entity, event.effect);
                commands
                    .queue(move |world: &mut World| engine::effect_applied(world, actor, effect));
            })
            .add_observer(|event: On<EffectRemoved>, mut commands: Commands| {
                let (actor, effect) = (event.entity, event.effect);
                commands
                    .queue(move |world: &mut World| engine::effect_removed(world, actor, effect));
            });
    }
}
