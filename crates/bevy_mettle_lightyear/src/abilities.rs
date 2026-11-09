//! Abilities over lightyear: requests and their input to the server, replies
//! back, and endings either way.

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::ecs::message::Messages;
use bevy::prelude::*;
use bevy_mettle_ability::events::Failure;
use bevy_mettle_ability::net::{
    AbilityCancelRequest, AbilityEnd, AbilityInput, AbilityReply, AbilityRequest,
    AbilityRequestWith, AbilityStarted, AbilityStartedWith,
};
use bevy_replicon::prelude::RepliconTick;
use bevy_replicon::server::server_tick::ServerTick;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::MettleChannel;

/// How many frames a client holds a confirming reply at most, waiting for the
/// state as of it: one whose actor didn't change has nothing to wait for.
const HOLD: u32 = 30;

/// A reply as it's sent: with the replication tick the server's state as of
/// it goes out at.
#[derive(Message, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(super) struct Answer {
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
pub(super) struct Held(Vec<(Answer, u32)>);

/// On a client: sends the requests with input, and the new input, mettle
/// wrote to the server.
pub(super) fn ask_with<I: Component + Clone + Serialize + DeserializeOwned + MapEntities>(
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
pub(super) fn hear_input<I: Component + Clone + Serialize + DeserializeOwned + MapEntities>(
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
                    "a client asked to activate {:?} for {}, which it doesn't control: rejected",
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

/// On a client: asks the server to cancel what it cancelled of its own
/// activations, after every request is sent: a cancel doesn't overtake what
/// it cancels on the ordered channel. One that does anyway, written between
/// the two, the server holds for its request.
pub(super) fn ask_cancels(
    mut cancels: ResMut<Messages<AbilityCancelRequest>>,
    mut senders: Query<&mut MessageSender<AbilityCancelRequest>, With<Client>>,
) {
    for cancel in cancels.drain() {
        for mut sender in &mut senders {
            sender.send::<MettleChannel>(cancel.clone());
        }
    }
}

/// On a server: takes each client's cancels of its own activations, for
/// actors it controls; any other is dropped. Lightyear gives a link a receiver
/// as its first one arrives.
pub(super) fn hear_cancels(
    mut links: Query<(Entity, Option<&mut MessageReceiver<AbilityCancelRequest>>), With<ClientOf>>,
    controlled: Query<&ControlledBy>,
    mut cancels: MessageWriter<AbilityCancelRequest>,
) {
    for (link, mut receiver) in &mut links {
        for cancel in receiver.iter_mut().flat_map(|receiver| receiver.receive()) {
            if controlled
                .get(cancel.actor)
                .is_ok_and(|controlled| controlled.owner == link)
            {
                cancels.write(cancel);
            } else {
                warn!(
                    "a client asked to cancel an ability of {}, which it doesn't control: dropped",
                    cancel.actor
                );
            }
        }
    }
}

/// A message from the server for the client controlling its actor: an end
/// of what that client runs, a start of what the server initiated.
pub(super) trait ForController: bevy::ecs::message::Message + Clone {
    /// The actor it's about.
    fn actor(&self) -> Entity;
}

impl ForController for AbilityEnd {
    fn actor(&self) -> Entity {
        self.actor
    }
}

impl ForController for AbilityStarted {
    fn actor(&self) -> Entity {
        self.actor
    }
}

impl<I: Send + Sync + Clone + 'static> ForController for AbilityStartedWith<I> {
    fn actor(&self) -> Entity {
        self.actor
    }
}

/// On a server: sends each `M` mettle wrote to the client controlling its
/// actor, if one does.
pub(super) fn tell<M: ForController>(
    mut messages: ResMut<Messages<M>>,
    controlled: Query<&ControlledBy>,
    mut senders: Query<&mut MessageSender<M>>,
) {
    for message in messages.drain() {
        let Ok(link) = controlled
            .get(message.actor())
            .map(|controlled| controlled.owner)
        else {
            continue;
        };
        if let Ok(mut sender) = senders.get_mut(link) {
            sender.send::<MettleChannel>(message);
        }
    }
}

/// On a client: hands each `M` from the server to mettle.
pub(super) fn hear<M: ForController>(
    mut receivers: Query<&mut MessageReceiver<M>, With<Client>>,
    mut messages: MessageWriter<M>,
) {
    for mut receiver in &mut receivers {
        for message in receiver.receive() {
            messages.write(message);
        }
    }
}

/// On a client: sends the requests mettle wrote to the server.
pub(super) fn ask(
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
pub(super) fn hear_requests(
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
                    "a client asked to activate {:?} for {}, which it doesn't control: rejected",
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
pub(super) fn reply(
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
pub(super) fn hear_replies(
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
