use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::tag::{TagAdded, TagRemoved};
use bevy_mettle_effect::events::EffectApplied;

use crate::active::GrantedBy;
use crate::behaviour::BehaviourRegistry;
use crate::net::end::{AbilityCancelRequest, AbilityEnd};
use crate::net::request::{AbilityReply, AbilityRequest};
use crate::net::{self};
use crate::{AbilityRegistry, engine};

/// The system set granted abilities are kept in, in `PreUpdate`.
///
/// New definitions given, requests served, replies resolved, replicas shown: a
/// transport hands its messages over before it.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AbilitySystems;

/// Activates abilities by their triggers, and grants them with their effects.
///
/// Activated as owned tags come and go, and granted and taken away with the
/// effects that grant them; given their new definitions as the
/// [`AbilityRegistry`] is replaced. Starts with an empty registry; the game
/// inserts one built from its data.
#[derive(Default)]
pub struct AbilityPlugin;

impl Plugin for AbilityPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AbilityRegistry>()
            .init_resource::<BehaviourRegistry>()
            .init_resource::<engine::Activations>()
            .init_resource::<NetRole>()
            .init_resource::<net::Pending>()
            .init_resource::<net::EarlyCancels>()
            .init_resource::<engine::Interrupting>()
            .add_message::<AbilityRequest>()
            .add_message::<AbilityReply>()
            .add_message::<AbilityCancelRequest>()
            .add_message::<AbilityEnd>()
            .add_systems(
                PreUpdate,
                (
                    engine::relink.run_if(resource_changed::<AbilityRegistry>),
                    net::serve
                        .in_set(net::Serving)
                        .run_if(resource_equals(NetRole::Server)),
                    net::resolve.run_if(resource_equals(NetRole::Client)),
                    net::hear_cancels
                        .after(net::Serving)
                        .run_if(resource_equals(NetRole::Server)),
                    net::hear_ends.run_if(resource_equals(NetRole::Client)),
                    net::show_replicas.run_if(resource_equals(NetRole::Client)),
                )
                    .chain()
                    .in_set(AbilitySystems),
            )
            .add_systems(
                PostUpdate,
                net::sync_replicas.run_if(resource_equals(NetRole::Server)),
            )
            .add_observer(
                |event: On<TagAdded>,
                 role: Res<NetRole>,
                 mut interrupting: ResMut<engine::Interrupting>,
                 mut commands: Commands| {
                    let (actor, tag) = (event.entity, event.tag);
                    engine::note_interrupt(actor, *role, &mut interrupting, &mut commands);
                    commands.queue(move |world: &mut World| engine::tag_added(world, actor, tag));
                },
            )
            .add_observer(
                |event: On<TagRemoved>,
                 role: Res<NetRole>,
                 mut interrupting: ResMut<engine::Interrupting>,
                 mut commands: Commands| {
                    let (actor, tag) = (event.entity, event.tag);
                    engine::note_interrupt(actor, *role, &mut interrupting, &mut commands);
                    commands.queue(move |world: &mut World| engine::tag_removed(world, actor, tag));
                },
            )
            .add_observer(|event: On<EffectApplied>, mut commands: Commands| {
                let (actor, effect) = (event.entity, event.effect);
                commands
                    .queue(move |world: &mut World| engine::effect_applied(world, actor, effect));
            })
            .add_observer(|event: On<Remove<GrantedBy>>, mut commands: Commands| {
                let ability = event.entity;
                commands.queue(move |world: &mut World| engine::ungranted(world, ability));
            });
    }
}
