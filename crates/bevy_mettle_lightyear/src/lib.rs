#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod prelude;

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::ecs::message::Messages;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy_mettle_ability::AbilitySystems;
use bevy_mettle_ability::active::AbilityOf;
use bevy_mettle_ability::events::Failure;
use bevy_mettle_ability::net::{
    AbilityInput, AbilityReplica, AbilityReply, AbilityRequest, AbilityRequestWith,
};
use bevy_mettle_attribute::authority::{self, Served};
use bevy_mettle_attribute::registry::AttributeState;
use bevy_mettle_attribute::{Attribute, Base, Current};
use bevy_mettle_effect::active::EffectOf;
use bevy_mettle_effect::net::{EffectReplica, NetRole};
use bevy_replicon::client::ClientSystems;
use bevy_replicon::prelude::RepliconTick;
use bevy_replicon::server::server_tick::ServerTick;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The channel mettle's messages go on: reliable and ordered.
///
/// Requests and replies keep their order. A reply isn't ordered with the
/// replication of what it did, which goes another way: a client holds each
/// reply until the server's state as of it has arrived.
#[derive(TypePath)]
pub struct MettleChannel;

/// How many frames a client holds a confirming reply at most, waiting for the
/// state as of it: one whose actor didn't change has nothing to wait for.
const HOLD: u32 = 30;

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
///   back to it, as `NotGranted`), and sends its [`AbilityReply`] back to that
///   client.
/// - A client holds a reply confirming its prediction until the server's
///   replicated state of the actor as of the reply has arrived, so what it
///   predicted gives way to the server's in the same frame.
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
        app.register_message::<Answer>()
            .add_map_entities()
            .add_direction(NetworkDirection::ServerToClient);
        app.component::<EffectReplica>().replicate();
        app.component::<EffectOf>().replicate();
        app.component::<AbilityReplica>().replicate();
        app.component::<AbilityOf>().replicate();
        app.init_resource::<Held>()
            .add_systems(
                PreUpdate,
                (
                    hear_replies.run_if(resource_equals(NetRole::Client)),
                    hear_requests.run_if(resource_equals(NetRole::Server)),
                )
                    .after(MessageSystems::Receive)
                    .after(ReplicationSystems::Receive)
                    .after(ClientSystems::Receive)
                    .before(AbilitySystems),
            )
            .add_systems(
                Update,
                (
                    (ask, remote).run_if(resource_equals(NetRole::Client)),
                    (reply, replicate).run_if(resource_equals(NetRole::Server)),
                ),
            );
    }
}

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
            ask_with::<I>.run_if(resource_equals(NetRole::Client)),
        )
    }
}

/// On a server: has each actor with `A` serve it, from its value now.
fn serve<A: Attribute>(
    mut commands: Commands,
    actors: Query<(Entity, &Current<A>), (With<Base<A>>, Without<Served<A>>)>,
) {
    for (actor, current) in &actors {
        commands
            .entity(actor)
            .insert(Served::<A>::new(current.get()));
    }
}

/// On a client: marks each actor replicated from the server
/// [`Remote`](authority::Remote), so it shows the values served.
fn remote(
    mut commands: Commands,
    actors: Query<
        Entity,
        (
            With<Replicated>,
            With<AttributeState>,
            Without<authority::Remote>,
        ),
    >,
) {
    for actor in &actors {
        commands.entity(actor).insert(authority::Remote);
    }
}

/// A reply as it's sent: with the replication tick the server's state as of
/// it goes out at.
#[derive(Message, Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Answer {
    reply: AbilityReply,
    tick: RepliconTick,
}

impl MapEntities for Answer {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.reply.map_entities(mapper);
    }
}

/// On a client: replies heard, held until the state as of them arrives, and
/// how many frames each has been.
#[derive(Resource, Default)]
struct Held(Vec<(Answer, u32)>);

/// On a client: sends the requests with input, and the new input, mettle
/// wrote to the server.
fn ask_with<I: Component + Clone + Serialize + DeserializeOwned + MapEntities>(
    mut requests: ResMut<Messages<AbilityRequestWith<I>>>,
    mut inputs: ResMut<Messages<AbilityInput<I>>>,
    mut senders: Query<
        (
            &mut MessageSender<AbilityRequestWith<I>>,
            &mut MessageSender<AbilityInput<I>>,
        ),
        With<Client>,
    >,
) {
    for request in requests.drain() {
        for (mut sender, _) in &mut senders {
            sender.send::<MettleChannel>(request.clone());
        }
    }
    for input in inputs.drain() {
        for (_, mut sender) in &mut senders {
            sender.send::<MettleChannel>(input.clone());
        }
    }
}

/// A client's link, as the server hears its input on it. Lightyear gives a
/// link a receiver as its first message of that kind arrives, so either may
/// not be there yet.
type Giving<I> = (
    Entity,
    Option<&'static mut MessageReceiver<AbilityRequestWith<I>>>,
    Option<&'static mut MessageReceiver<AbilityInput<I>>>,
    &'static mut MessageSender<Answer>,
);

/// On a server: takes each client's requests with input, and new input, for
/// actors it controls; rejects requests for the rest back to it, and drops
/// input for them.
fn hear_input<I: Component + Clone + Serialize + DeserializeOwned + MapEntities>(
    mut links: Query<Giving<I>, With<ClientOf>>,
    controlled: Query<&ControlledBy>,
    tick: Res<ServerTick>,
    mut requests: MessageWriter<AbilityRequestWith<I>>,
    mut inputs: MessageWriter<AbilityInput<I>>,
) {
    let controls = |actor: Entity, link: Entity| {
        controlled
            .get(actor)
            .is_ok_and(|controlled| controlled.owner == link)
    };
    for (link, mut asked, mut given, mut sender) in &mut links {
        for request in asked.iter_mut().flat_map(|asked| asked.receive()) {
            if controls(request.actor, link) {
                requests.write(request);
            } else {
                warn!(
                    "a client asked to activate `{}` for {}, which it doesn't control: rejected",
                    request.ability, request.actor
                );
                sender.send::<MettleChannel>(Answer {
                    reply: AbilityReply {
                        actor: request.actor,
                        ability: request.ability,
                        activation: request.activation,
                        rejected: Some(Failure::NotGranted),
                    },
                    tick: **tick,
                });
            }
        }
        for input in given.iter_mut().flat_map(|given| given.receive()) {
            if controls(input.actor, link) {
                inputs.write(input);
            } else {
                warn!(
                    "a client gave input for {}, which it doesn't control: dropped",
                    input.actor
                );
            }
        }
    }
}

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
    &'static mut MessageSender<Answer>,
);

/// On a server: takes each client's requests for actors it controls, and
/// rejects the rest back to it, so its prediction is undone.
fn hear_requests(
    mut links: Query<Asking, With<ClientOf>>,
    controlled: Query<&ControlledBy>,
    tick: Res<ServerTick>,
    mut requests: MessageWriter<AbilityRequest>,
) {
    for (link, mut receiver, mut sender) in &mut links {
        for request in receiver.receive() {
            if controlled
                .get(request.actor)
                .is_ok_and(|controlled| controlled.owner == link)
            {
                requests.write(request);
            } else {
                warn!(
                    "a client asked to activate `{}` for {}, which it doesn't control: rejected",
                    request.ability, request.actor
                );
                sender.send::<MettleChannel>(Answer {
                    reply: AbilityReply {
                        actor: request.actor,
                        ability: request.ability,
                        activation: request.activation,
                        rejected: Some(Failure::NotGranted),
                    },
                    tick: **tick,
                });
            }
        }
    }
}

/// On a server: sends each reply mettle wrote to the client controlling its
/// actor, the only one a request is taken from, with the tick this frame's
/// state goes out at: this one's, if replication's tick came round this
/// frame, else the next. Later replies too: a running activation's input
/// rejected.
fn reply(
    mut replies: ResMut<Messages<AbilityReply>>,
    controlled: Query<&ControlledBy>,
    mut senders: Query<&mut MessageSender<Answer>>,
    tick: Res<ServerTick>,
) {
    let at = if tick.is_changed() {
        **tick
    } else {
        **tick + 1
    };
    for reply in replies.drain() {
        let Ok(link) = controlled
            .get(reply.actor)
            .map(|controlled| controlled.owner)
        else {
            continue;
        };
        if let Ok(mut sender) = senders.get_mut(link) {
            sender.send::<MettleChannel>(Answer { reply, tick: at });
        }
    }
}

/// On a client: hands the server's replies to mettle, a confirming one once
/// its actor's state as of it has arrived (or it's waited long enough); a
/// rejecting one at once, nothing having changed there.
fn hear_replies(
    mut receivers: Query<&mut MessageReceiver<Answer>, With<Client>>,
    histories: Query<&ConfirmHistory>,
    mut held: ResMut<Held>,
    mut replies: MessageWriter<AbilityReply>,
) {
    for mut receiver in &mut receivers {
        held.0.extend(receiver.receive().map(|answer| (answer, 0)));
    }
    held.0.retain_mut(|(answer, waited)| {
        let arrived = answer.reply.rejected.is_some()
            || *waited >= HOLD
            || histories
                .get(answer.reply.actor)
                .is_ok_and(|history| history.last_tick().is_newer_or_eq(answer.tick));
        if arrived {
            replies.write(answer.reply.clone());
        } else {
            *waited += 1;
        }
        !arrived
    });
}

/// Effects and abilities that aren't replicated yet.
type Unreplicated = (
    Without<Replicate>,
    Without<ReplicateLike>,
    Or<(With<EffectReplica>, With<AbilityReplica>)>,
);

/// On a server: replicates each new effect and granted ability as its actor is.
fn replicate(
    mut commands: Commands,
    new: Query<(Entity, Option<&EffectOf>, Option<&AbilityOf>), Unreplicated>,
) {
    for (entity, effect, ability) in &new {
        let Some(actor) = effect.map(|of| of.0).or(ability.map(|of| of.0)) else {
            continue;
        };
        commands
            .entity(entity)
            .insert(ReplicateLike { root: actor });
    }
}
