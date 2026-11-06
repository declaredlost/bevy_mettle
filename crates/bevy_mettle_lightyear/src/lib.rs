#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::collections::HashMap;

use bevy::ecs::message::Messages;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy_mettle_ability::{AbilityOf, AbilityReplica, AbilityReply, AbilityRequest, Failure};
use bevy_mettle_attribute::{Attribute, Base};
use bevy_mettle_effect::{EffectOf, EffectReplica, NetRole};
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::*;

/// The channel mettle's messages go on: reliable and ordered, so a reply
/// never overtakes its request's effects.
#[derive(TypePath)]
pub struct MettleChannel;

/// mettle over lightyear, on the server and its clients alike: what to
/// replicate registered, and requests and replies sent. Add it after
/// lightyear's plugins and mettle's, with mettle's [`NetRole`] set; it
/// does what that role needs.
///
/// - The server replicates every active effect's replica and every granted
///   ability's to every client, with what they're of; the game replicates its
///   actors, and their bases with
///   [`replicate_attribute`](MettleNetApp::replicate_attribute).
/// - A client's [`AbilityRequest`]s are sent to the server, which takes one
///   only for an actor [`ControlledBy`] the client asking (rejecting any other
///   back to it, as `NotGranted`), and sends its [`AbilityReply`] back to that
///   client.
#[derive(Default)]
pub struct MettleNetPlugin;

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
        app.register_message::<AbilityReply>()
            .add_map_entities()
            .add_direction(NetworkDirection::ServerToClient);
        app.component::<EffectReplica>().replicate();
        app.component::<EffectOf>().replicate();
        app.component::<AbilityReplica>().replicate();
        app.component::<AbilityOf>().replicate();
        app.init_resource::<Asked>().add_systems(
            Update,
            (
                (ask, hear_replies).run_if(resource_equals(NetRole::Client)),
                (hear_requests, reply, replicate).run_if(resource_equals(NetRole::Server)),
            ),
        );
    }
}

/// Replicating attributes' bases.
pub trait MettleNetApp {
    /// Replicates `A`'s [`Base`] from the server: what clients' values are
    /// worked out from.
    fn replicate_attribute<A: Attribute>(&mut self) -> &mut Self;
}

impl MettleNetApp for App {
    fn replicate_attribute<A: Attribute>(&mut self) -> &mut Self {
        self.component::<Base<A>>().replicate();
        self
    }
}

/// On a server: which client asked for each activation, to answer.
#[derive(Resource, Default)]
struct Asked(HashMap<(Entity, u64), Entity>);

/// On a client: sends the requests mettle wrote to the server.
fn ask(
    mut requests: ResMut<Messages<AbilityRequest>>,
    mut senders: Query<&mut MessageSender<AbilityRequest>, With<Client>>,
) {
    for request in requests.drain() {
        for mut sender in &mut senders {
            sender.send::<MettleChannel>(request.clone());
        }
    }
}

/// A client's link, as the server hears it and answers on it.
type Asking = (
    Entity,
    &'static mut MessageReceiver<AbilityRequest>,
    &'static mut MessageSender<AbilityReply>,
);

/// On a server: takes each client's requests for actors it controls, and
/// rejects the rest back to it, so its prediction is undone.
fn hear_requests(
    mut links: Query<Asking, With<ClientOf>>,
    controlled: Query<&ControlledBy>,
    mut asked: ResMut<Asked>,
    mut requests: MessageWriter<AbilityRequest>,
) {
    for (link, mut receiver, mut sender) in &mut links {
        for request in receiver.receive() {
            if controlled
                .get(request.actor)
                .is_ok_and(|controlled| controlled.owner == link)
            {
                asked.0.insert((request.actor, request.activation), link);
                requests.write(request);
            } else {
                warn!(
                    "a client asked to activate `{}` for {}, which it doesn't control: rejected",
                    request.ability, request.actor
                );
                sender.send::<MettleChannel>(AbilityReply {
                    actor: request.actor,
                    ability: request.ability,
                    activation: request.activation,
                    rejected: Some(Failure::NotGranted),
                });
            }
        }
    }
}

/// On a server: sends each reply mettle wrote to the client that asked.
fn reply(
    mut replies: ResMut<Messages<AbilityReply>>,
    mut asked: ResMut<Asked>,
    mut senders: Query<&mut MessageSender<AbilityReply>>,
) {
    for reply in replies.drain() {
        let Some(link) = asked.0.remove(&(reply.actor, reply.activation)) else {
            continue;
        };
        if let Ok(mut sender) = senders.get_mut(link) {
            sender.send::<MettleChannel>(reply);
        }
    }
}

/// On a client: hands the server's replies to mettle.
fn hear_replies(
    mut receivers: Query<&mut MessageReceiver<AbilityReply>, With<Client>>,
    mut replies: MessageWriter<AbilityReply>,
) {
    for mut receiver in &mut receivers {
        replies.write_batch(receiver.receive());
    }
}

/// Effects and abilities that aren't replicated yet.
type Unreplicated = (
    Without<Replicate>,
    Or<(With<EffectReplica>, With<AbilityReplica>)>,
);

/// On a server: replicates each new effect and granted ability to every
/// client.
fn replicate(mut commands: Commands, new: Query<Entity, Unreplicated>) {
    for entity in &new {
        commands
            .entity(entity)
            .insert(Replicate::to_clients(NetworkTarget::All));
    }
}
