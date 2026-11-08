#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod prelude;

mod abilities;
mod attributes;
mod replicas;

use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy_mettle_ability::AbilitySystems;
use bevy_mettle_ability::active::AbilityOf;
use bevy_mettle_ability::net::end::{AbilityCancelRequest, AbilityEnd};
use bevy_mettle_ability::net::input::AbilityInput;
use bevy_mettle_ability::net::replica::AbilityReplica;
use bevy_mettle_ability::net::request::{AbilityRequest, AbilityRequestWith};
use bevy_mettle_attribute::authority::Served;
use bevy_mettle_attribute::{Attribute, Base};
use bevy_mettle_core::net::NetRole;
use bevy_mettle_effect::active::EffectOf;
use bevy_mettle_effect::cue::{CueBursts, CueSystems, ServedCues};
use bevy_mettle_effect::net::EffectReplica;
use bevy_replicon::client::ClientSystems;
use bevy_replicon::prelude::AppVisibilityExt;
use bevy_replicon::server::visibility::registry::FilterRegistry;
use lightyear::prelude::*;
use serde::Serialize;
use serde::de::DeserializeOwned;

use self::abilities::{
    Answer, Held, ask, ask_cancels, ask_with, hear_cancels, hear_ends, hear_input, hear_replies,
    hear_requests, reply, tell_ends,
};
use self::attributes::{remote, serve};
use self::replicas::{ReplicaOwner, replicate, retarget};

/// The channel mettle's messages go on: reliable and ordered.
///
/// Requests and replies keep their order. A reply isn't ordered with the
/// replication of what it did, which goes another way: a client holds each
/// reply until the server's state as of it has arrived.
#[derive(TypePath)]
pub struct MettleChannel;

/// Mettle over lightyear, on the server and its clients alike.
///
/// What to replicate registered, and requests and replies sent. Add it after
/// lightyear's plugins and mettle's, with mettle's [`NetRole`] set; it does
/// what that role needs.
///
/// - The server replicates every active effect's replica and every granted
///   ability's as their actor is replicated ([`ReplicateLike`] it), with what
///   they're of; the game replicates its actors, and their bases with
///   [`replicate_attribute`](MettleNetApp::replicate_attribute).
/// - A client's [`AbilityRequest`]s are sent to the server, which takes one
///   only for an actor [`ControlledBy`] the client asking (rejecting any other
///   back to it, as `NotGranted`), and sends its
///   [`AbilityReply`](bevy_mettle_ability::net::request::AbilityReply) back to
///   that client.
/// - Endings go both ways: a client's [`AbilityCancelRequest`]s to the server,
///   taken only for actors it controls; the server's [`AbilityEnd`]s to the
///   client controlling the actor.
/// - A client holds a reply confirming its prediction until the server's
///   replicated state of the actor as of the reply has arrived, so what it
///   predicted gives way to the server's in the same frame.
/// - Each actor's cues, lasting ([`ServedCues`]) and one-shot ([`CueBursts`]),
///   are replicated with it, to whoever sees it: what other clients see of its
///   effects and abilities, even where their replicas aren't sent.
#[derive(Default)]
pub struct MettleNetPlugin {
    /// Who an actor's effect and ability replicas are sent to: its owner
    /// alone, unless it says.
    pub replicas: Replicas,
}

/// Who an actor's effect and ability replicas are sent to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Resource)]
pub enum Replicas {
    /// The client [`ControlledBy`] the actor, alone: it predicts with them.
    /// Others see its cues, owned tags and served values; its cooldowns and
    /// buffs stay its own. An actor no client controls sends them to none.
    #[default]
    Owner,
    /// Every client that sees the actor: a party frame showing allies'
    /// buffs.
    Everyone,
}

impl Plugin for MettleNetPlugin {
    fn build(&self, app: &mut App) {
        app.add_channel::<MettleChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);
        app.register_message::<AbilityRequest>()
            .add_map_entities()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<Answer>()
            .add_map_entities()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<AbilityCancelRequest>()
            .add_map_entities()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<AbilityEnd>()
            .add_map_entities()
            .add_direction(NetworkDirection::ServerToClient);
        app.insert_resource(self.replicas);
        // Only a server filters what it sends: a client has no filters.
        if app.world().contains_resource::<FilterRegistry>() {
            app.add_visibility_filter::<ReplicaOwner>();
        }
        // Cues worked out before they're sent: served the frame they start.
        app.configure_sets(PostUpdate, CueSystems.before(ReplicationSystems::Send));
        app.component::<ServedCues>().replicate();
        app.component::<CueBursts>().replicate();
        app.component::<EffectReplica>().replicate();
        app.component::<EffectOf>().replicate();
        app.component::<AbilityReplica>().replicate();
        app.component::<AbilityOf>().replicate();
        app.init_resource::<Held>()
            .add_systems(
                PreUpdate,
                (
                    (hear_replies, hear_ends).run_if(resource_equals(NetRole::Client)),
                    (hear_requests, hear_cancels).run_if(resource_equals(NetRole::Server)),
                )
                    .after(MessageSystems::Receive)
                    .after(ReplicationSystems::Receive)
                    .after(ClientSystems::Receive)
                    .before(AbilitySystems),
            )
            .add_systems(
                Update,
                (
                    (ask.in_set(Asking), ask_cancels.after(Asking), remote)
                        .run_if(resource_equals(NetRole::Client)),
                    (reply, tell_ends, replicate, retarget)
                        .chain()
                        .run_if(resource_equals(NetRole::Server)),
                ),
            );
    }
}

/// On a client: the systems sending requests, with input and without. Cancels
/// are sent after them, so one never goes before what it cancels.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct Asking;

/// Replicating attributes, and sending abilities' input, over lightyear.
pub trait MettleNetApp {
    /// Replicates `A` from the server: its [`Base`], what predictions are
    /// worked out on, and its [`Served`] value, what clients show.
    ///
    /// On the server, every actor with `Base<A>` serves `A`.
    fn replicate_attribute<A: Attribute>(&mut self) -> &mut Self;

    /// Sends `I`, a behaviour's input, from clients to the server: activations
    /// with it, and new input to activations running, taken only for actors
    /// [`ControlledBy`] the client sending it, on [`MettleChannel`].
    fn replicate_input<I>(&mut self) -> &mut Self
    where
        I: Component + Clone + Serialize + DeserializeOwned + MapEntities;
}

impl MettleNetApp for App {
    fn replicate_attribute<A: Attribute>(&mut self) -> &mut Self {
        self.component::<Base<A>>().replicate();
        self.component::<Served<A>>().replicate();
        self.add_systems(Update, serve::<A>.run_if(resource_equals(NetRole::Server)))
    }

    fn replicate_input<I>(&mut self) -> &mut Self
    where
        I: Component + Clone + Serialize + DeserializeOwned + MapEntities,
    {
        self.register_message::<AbilityRequestWith<I>>()
            .add_map_entities()
            .add_direction(NetworkDirection::ClientToServer);
        self.register_message::<AbilityInput<I>>()
            .add_map_entities()
            .add_direction(NetworkDirection::ClientToServer);
        self.add_systems(
            PreUpdate,
            hear_input::<I>
                .run_if(resource_equals(NetRole::Server))
                .after(MessageSystems::Receive)
                .before(AbilitySystems),
        )
        .add_systems(
            Update,
            ask_with::<I>
                .in_set(Asking)
                .run_if(resource_equals(NetRole::Client)),
        )
    }
}
