//! An ability's input over a network: given as it activates, and while it runs.
//!
//! Activated with it, and new input sent while it runs.

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy_mettle_core::Activation;
use bevy_mettle_core::net::NetRole;
use serde::{Deserialize, Serialize};

use super::request::{AbilityRequest, serve};
use super::start::{AbilityStarted, hear_started};
use super::{Serving, running};
use crate::behaviour::NoInput;
use crate::engine;

/// A client's new input to an ability it's running, for the server to check.
///
/// Input it already predicts with: for the server to check and take.
#[derive(Message, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbilityInput<I> {
    /// The actor.
    pub actor: Entity,
    /// The activation it's running under.
    pub activation: Activation,
    /// The input.
    pub input: I,
}

impl<I: MapEntities> MapEntities for AbilityInput<I> {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
        self.input.map_entities(mapper);
    }
}

/// On a client: sends the server new input to the activation it's running.
pub(crate) fn send_input<I: Component + Clone>(
    world: &mut World,
    actor: Entity,
    activation: Activation,
    input: I,
) {
    world.write_message(AbilityInput {
        actor,
        activation,
        input,
    });
}

/// Adds what carries `I`, a behaviour's input, between a client and its
/// server: once, however many behaviours take it.
pub(crate) fn add_input<I: Component + Clone>(app: &mut App) {
    if app.world().contains_resource::<Messages<AbilityInput<I>>>() {
        return;
    }
    app.init_resource::<NetRole>()
        .add_message::<AbilityRequest<I>>()
        .add_message::<AbilityInput<I>>()
        .add_message::<AbilityStarted<I>>()
        .add_systems(
            PreUpdate,
            hear_started::<I>
                .after(hear_started::<NoInput>)
                .before(super::hear_ends)
                .in_set(crate::AbilitySystems)
                .run_if(resource_equals(NetRole::Client)),
        )
        .add_systems(
            PreUpdate,
            (serve::<I>.in_set(Serving), hear_input::<I>)
                .chain()
                .after(serve::<NoInput>)
                .in_set(crate::AbilitySystems)
                .run_if(resource_equals(NetRole::Server)),
        );
}

/// On a server: gives clients' new input to the activations they're running,
/// checked; one no longer running is past, and dropped.
fn hear_input<I: Component + Clone>(
    mut inputs: MessageReader<AbilityInput<I>>,
    mut commands: Commands,
) {
    for given in inputs.read().cloned() {
        commands.queue(move |world: &mut World| {
            if let Some(ability) = running(world, given.actor, given.activation) {
                let _ = engine::set_input(world, ability, None, given.input);
            }
        });
    }
}
