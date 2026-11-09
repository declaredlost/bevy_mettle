//! Asking the server to activate an ability: predicted, served, and resolved.
//!
//! The ask and its answer: predicted on the client, served on the server,
//! resolved as it comes back.

use std::any::TypeId;

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::registry::AbilityId;
use bevy_mettle_core::tag::Tags;
use bevy_mettle_effect::EffectWorld;
use serde::{Deserialize, Serialize};

use super::{DERIVED, SERVERS, role, running};
use crate::active::{AbilityOf, ActiveAbility, GrantedAbility};
use crate::behaviour::{Behaviour, BehaviourId, BehaviourRegistry, NoInput};
use crate::data::NetExecution;
use crate::engine;
use crate::events::{AbilityFailed, Activated, Failure};

/// A client's ask that the server activate an ability it already predicts.
///
/// It asks for `ability` on `actor`, which the client activated under
/// `activation`.
#[derive(Message, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityRequest {
    /// The actor.
    pub actor: Entity,
    /// The ability.
    pub ability: AbilityId,
    /// The activation the client predicted it under: its own, the top bit
    /// clear (the server's own have it set).
    pub activation: u64,
}

impl MapEntities for AbilityRequest {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
    }
}

/// A client's ask that the server activate an ability with its input.
///
/// One it already predicts: an [`AbilityRequest`], and the input it activated
/// it with.
#[derive(Message, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbilityRequestWith<I> {
    /// The actor.
    pub actor: Entity,
    /// The ability.
    pub ability: AbilityId,
    /// The activation the client predicted it under.
    pub activation: u64,
    /// The input it activated it with.
    pub input: I,
}

impl<I: MapEntities> MapEntities for AbilityRequestWith<I> {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
        self.input.map_entities(mapper);
    }
}

/// The server's answer to an [`AbilityRequest`]: activated, or why not.
#[derive(Message, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityReply {
    /// The actor.
    pub actor: Entity,
    /// The ability.
    pub ability: AbilityId,
    /// The activation it answers.
    pub activation: u64,
    /// Why it didn't activate, if it didn't.
    pub rejected: Option<Failure>,
}

impl MapEntities for AbilityReply {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
    }
}

/// The abilities a client predicted, by activation, not yet answered; and the
/// activations derived from each, predicted as it triggered them.
#[derive(Resource, Debug, Default)]
pub(crate) struct Pending {
    abilities: HashMap<u64, Entity>,
    derived: HashMap<u64, Vec<u64>>,
}

impl Pending {
    /// `ability`, predicted under `activation`.
    pub(crate) fn insert(&mut self, activation: u64, ability: Entity) {
        self.abilities.insert(activation, ability);
    }

    /// `ability`, predicted under `activation`, derived from `parent`'s: undone
    /// with it while it awaits its answer. Derived after it's answered, it
    /// stands alone, answered on its own.
    pub(crate) fn insert_derived(&mut self, parent: u64, activation: u64, ability: Entity) {
        self.insert(activation, ability);
        if self.abilities.contains_key(&parent) {
            self.derived.entry(parent).or_default().push(activation);
        }
    }

    /// What was predicted under `activation`, taken: answered.
    fn take(&mut self, activation: u64) -> Option<Entity> {
        self.abilities.remove(&activation)
    }

    /// The activations derived from `parent`'s, taken.
    fn take_derived(&mut self, parent: u64) -> Vec<u64> {
        self.derived.remove(&parent).unwrap_or_default()
    }
}

/// How this world runs an activation of an ability run as `net` says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// Runs it here, deciding: a server, standalone, or a client's local-only
    /// ability.
    Here,
    /// Runs it at once and asks the server: a client's predicted ability.
    Predict,
    /// Asks the server, running nothing: a client's server-run ability.
    Ask,
}

/// How `world` runs an activation of an ability run as `net` says.
fn route(world: &World, net: NetExecution) -> Route {
    match (role(world), net) {
        (NetRole::Client, NetExecution::Predicted) => Route::Predict,
        (NetRole::Client, NetExecution::ServerOnly | NetExecution::ServerInitiated) => Route::Ask,
        _ => Route::Here,
    }
}

/// `actor`'s first granted ability `id`, and how it runs over a network.
pub(crate) fn granted(
    world: &World,
    actor: Entity,
    id: AbilityId,
) -> Option<(Entity, NetExecution)> {
    engine::abilities_of(world, actor)
        .into_iter()
        .find_map(|ability| {
            let granted = world.get::<GrantedAbility>(ability)?;
            (granted.def.id == id).then_some((ability, granted.def.net))
        })
}

/// That `actor` hasn't `id`: told, as activating it would.
fn not_granted(world: &mut World, actor: Entity, id: AbilityId) -> Failure {
    world.trigger(AbilityFailed {
        entity: actor,
        id,
        reason: Failure::NotGranted,
    });
    Failure::NotGranted
}

/// Activates `id` for `actor` as it runs over a network: on a client, a
/// predicted one run at once and asked of the server, a server-run one only
/// asked of it, a local-only one run here alone; elsewhere, run here.
pub(crate) fn predict(
    world: &mut World,
    actor: Entity,
    id: AbilityId,
) -> Result<Activated, Failure> {
    let Some((ability, net)) = granted(world, actor, id) else {
        return Err(not_granted(world, actor, id));
    };
    match route(world, net) {
        Route::Here => engine::activate_id(world, actor, id, None).map(Activated::Running),
        Route::Ask => {
            ask(world, actor, id);
            Ok(Activated::Asked(ability))
        }
        Route::Predict => {
            let activation = predicting(world, ability, |world, activation| {
                engine::activate_id(world, actor, id, Some(activation)).map(drop)
            })?;
            request(world, actor, id, activation);
            Ok(Activated::Running(ability))
        }
    }
}

/// Activates the granted `ability` as it runs over a network, as [`predict`]
/// does: what activating a granted ability, or abilities by tag, comes to.
pub(crate) fn predict_granted(world: &mut World, ability: Entity) -> Result<Activated, Failure> {
    let Some((actor, id, net)) = world
        .get::<GrantedAbility>(ability)
        .zip(world.get::<AbilityOf>(ability))
        .map(|(granted, of)| (of.0, granted.def.id, granted.def.net))
    else {
        return Err(Failure::NotGranted);
    };
    match route(world, net) {
        Route::Here => {
            engine::activate(world, ability, None, None).map(|()| Activated::Running(ability))
        }
        Route::Ask => {
            ask(world, actor, id);
            Ok(Activated::Asked(ability))
        }
        Route::Predict => {
            let activation = predicting(world, ability, |world, activation| {
                engine::activate(world, ability, None, Some(activation))
            })?;
            request(world, actor, id, activation);
            Ok(Activated::Running(ability))
        }
    }
}

/// Activates every one of `actor`'s abilities tagged any of `tags`, each as
/// it runs over a network: what each came to, those that didn't fail.
pub(crate) fn predict_tagged(world: &mut World, actor: Entity, tags: &Tags) -> Vec<Activated> {
    let tagged: Vec<Entity> = engine::abilities_of(world, actor)
        .into_iter()
        .filter(|ability| {
            world
                .get::<GrantedAbility>(*ability)
                .is_some_and(|granted| granted.def.tags.contains_any(tags))
        })
        .collect();
    tagged
        .into_iter()
        .filter_map(|ability| predict_granted(world, ability).ok())
        .collect()
}

/// Asks the server for `id` for `actor`, running nothing: under a new
/// activation, its number.
fn ask(world: &mut World, actor: Entity, id: AbilityId) -> u64 {
    let activation = engine::next_activation(world);
    request(world, actor, id, activation);
    activation
}

/// Asks the server for `id` for `actor` under `activation`.
fn request(world: &mut World, actor: Entity, id: AbilityId, activation: u64) {
    world.write_message(AbilityRequest {
        actor,
        ability: id,
        activation,
    });
}

/// Predicts `ability` under a new activation, its number: noted pending
/// before `activate` runs it, so what it triggers as it starts is undone with
/// it; taken back if it doesn't activate.
fn predicting(
    world: &mut World,
    ability: Entity,
    activate: impl FnOnce(&mut World, u64) -> Result<(), Failure>,
) -> Result<u64, Failure> {
    let activation = engine::next_activation(world);
    world
        .get_resource_or_init::<Pending>()
        .insert(activation, ability);
    if let Err(failure) = activate(world, activation) {
        if let Some(mut pending) = world.get_resource_mut::<Pending>() {
            pending.take(activation);
            pending.take_derived(activation);
        }
        return Err(failure);
    }
    Ok(activation)
}

/// Activates `id` for `actor` with `input`, as it runs over a network, as
/// [`predict`] does: what's asked of the server carries the input.
pub(crate) fn predict_with<B: Behaviour>(
    world: &mut World,
    actor: Entity,
    id: AbilityId,
    input: B::Input,
) -> Result<Activated, Failure> {
    let Some(behaviour) = world
        .get_resource::<BehaviourRegistry>()
        .and_then(|behaviours| behaviours.id_of::<B>())
    else {
        warn!("`{}` isn't added: nothing it runs can activate", B::NAME);
        return Err(Failure::Input);
    };
    let Some((ability, net)) = granted(world, actor, id) else {
        return Err(not_granted(world, actor, id));
    };
    if TypeId::of::<B::Input>() == TypeId::of::<NoInput>() {
        // Nothing to give: as any activation, but still of `B`.
        return predict_runs(world, actor, id, ability, net, behaviour);
    }
    match route(world, net) {
        Route::Here => {
            engine::activate_id_with(world, actor, id, None, Some(behaviour), Some(input))
                .map(Activated::Running)
        }
        Route::Ask => {
            let activation = engine::next_activation(world);
            world.write_message(AbilityRequestWith {
                actor,
                ability: id,
                activation,
                input,
            });
            Ok(Activated::Asked(ability))
        }
        Route::Predict => {
            let sent = input.clone();
            let activation = predicting(world, ability, |world, activation| {
                engine::activate_id_with(
                    world,
                    actor,
                    id,
                    Some(activation),
                    Some(behaviour),
                    Some(sent),
                )
                .map(drop)
            })?;
            world.write_message(AbilityRequestWith {
                actor,
                ability: id,
                activation,
                input,
            });
            Ok(Activated::Running(ability))
        }
    }
}

/// Activates `id`, run by `behaviour`, for `actor`, with no input: as
/// [`predict`] does.
fn predict_runs(
    world: &mut World,
    actor: Entity,
    id: AbilityId,
    ability: Entity,
    net: NetExecution,
    behaviour: BehaviourId,
) -> Result<Activated, Failure> {
    match route(world, net) {
        Route::Here => {
            engine::activate_id_with::<NoInput>(world, actor, id, None, Some(behaviour), None)
                .map(Activated::Running)
        }
        Route::Ask => {
            ask(world, actor, id);
            Ok(Activated::Asked(ability))
        }
        Route::Predict => {
            let activation = predicting(world, ability, |world, activation| {
                engine::activate_id_with::<NoInput>(
                    world,
                    actor,
                    id,
                    Some(activation),
                    Some(behaviour),
                    None,
                )
                .map(drop)
            })?;
            request(world, actor, id, activation);
            Ok(Activated::Running(ability))
        }
    }
}

/// On a server: answers a client's `activation` of `ability` for `actor`,
/// rejected for `rejected` if it is: what the client predicted, asked for or
/// not.
pub(crate) fn answer(
    world: &mut World,
    actor: Entity,
    ability: AbilityId,
    activation: u64,
    rejected: Option<Failure>,
) {
    world.write_message(AbilityReply {
        actor,
        ability,
        activation,
        rejected,
    });
}

/// On a server: rejects a client's `activation` back to it, as it's no longer
/// to be (its input didn't hold). A server's own isn't answered.
pub(crate) fn reject(
    world: &mut World,
    actor: Entity,
    ability: AbilityId,
    activation: u64,
    reason: Failure,
) {
    if role(world) == NetRole::Server && activation & SERVERS == 0 {
        world.write_message(AbilityReply {
            actor,
            ability,
            activation,
            rejected: Some(reason),
        });
    }
}

/// On a server: activates what clients ask for, under their activations, and
/// answers. One under an activation with the server's bit set is rejected: a
/// client's can't look like the server's own.
pub(crate) fn serve(mut requests: MessageReader<AbilityRequest>, mut commands: Commands) {
    for request in requests.read().cloned() {
        commands.queue(move |world: &mut World| {
            let rejected = askable(world, request.actor, request.ability, request.activation)
                .and_then(|()| {
                    engine::activate_id(
                        world,
                        request.actor,
                        request.ability,
                        Some(request.activation),
                    )
                })
                .err();
            world.write_message(AbilityReply {
                actor: request.actor,
                ability: request.ability,
                activation: request.activation,
                rejected,
            });
        });
    }
}

/// Whether a client can ask for `id` for `actor` under `activation`: its own
/// activation, not the server's or one derived; an ability that isn't local
/// only. Warns, and gives why not, if not.
pub(crate) fn askable(
    world: &World,
    actor: Entity,
    id: AbilityId,
    activation: u64,
) -> Result<(), Failure> {
    if activation & (SERVERS | DERIVED) != 0 {
        warn!(
            "a client asked to activate {id:?} under activation {activation}, not its own: rejected"
        );
        return Err(Failure::NotGranted);
    }
    if granted(world, actor, id).is_some_and(|(_, net)| net == NetExecution::LocalOnly) {
        warn!("a client asked the server to activate {id:?}, which runs locally only: rejected");
        return Err(Failure::NotGranted);
    }
    Ok(())
}

/// On a client: keeps what the server confirmed, and undoes what it rejected,
/// with what was predicted as it triggered.
pub(crate) fn resolve(mut replies: MessageReader<AbilityReply>, mut commands: Commands) {
    for reply in replies.read().cloned() {
        commands.queue(move |world: &mut World| {
            // Not pending, it was answered before: a running activation whose
            // new input was rejected.
            let ability = world
                .get_resource_mut::<Pending>()
                .and_then(|mut pending| pending.take(reply.activation))
                .or_else(|| running(world, reply.actor, reply.activation));
            let Some(reason) = reply.rejected else {
                world.confirm_prediction(reply.activation);
                // What it triggered is answered on its own.
                if let Some(mut pending) = world.get_resource_mut::<Pending>() {
                    pending.take_derived(reply.activation);
                }
                return;
            };
            undo(world, reply.actor, reply.activation, ability);
            world.trigger(AbilityFailed {
                entity: reply.actor,
                id: reply.ability,
                reason,
            });
        });
    }
}

/// Undoes what `actor` predicted under `activation` (its `ability`, if it
/// still runs it), and what that predicted as it triggered: cancelled.
fn undo(world: &mut World, actor: Entity, activation: u64, ability: Option<Entity>) {
    if let Some(ability) = ability
        && world
            .get::<ActiveAbility>(ability)
            .is_some_and(|active| active.activation == activation)
    {
        engine::end_quietly(world, ability, true);
    }
    world.reject_prediction(activation);
    let derived = world
        .get_resource_mut::<Pending>()
        .map(|mut pending| pending.take_derived(activation))
        .unwrap_or_default();
    for child in derived {
        let ability = world
            .get_resource_mut::<Pending>()
            .and_then(|mut pending| pending.take(child))
            .or_else(|| running(world, actor, child));
        undo(world, actor, child, ability);
    }
}

#[cfg(test)]
mod tests {
    use bevy::prelude::*;

    use super::Pending;

    #[test]
    fn a_child_links_to_its_parent_only_while_the_parent_awaits_its_answer() {
        let mut world = World::new();
        let (parent, first, second) = (
            world.spawn_empty().id(),
            world.spawn_empty().id(),
            world.spawn_empty().id(),
        );
        let mut pending = Pending::default();
        pending.insert(1, parent);
        pending.insert_derived(1, 2, first);
        assert_eq!(pending.derived.get(&1), Some(&vec![2]), "linked");

        // Answered: confirmed, what it triggered answered on its own.
        assert_eq!(pending.take(1), Some(parent));
        assert_eq!(pending.take_derived(1), [2]);

        pending.insert_derived(1, 3, second);
        assert_eq!(pending.take_derived(1), [], "not linked: answered already");
        assert_eq!(pending.take(3), Some(second), "but pending itself");
        assert_eq!(pending.take(2), Some(first));
    }
}
