//! Granting, activating and ending abilities: as commands, or on the world.

use bevy::prelude::*;
use bevy_mettle_core::Id;
use bevy_mettle_core::tag::Tags;

use crate::behaviour::Behaviour;
use crate::data::Ability;
use crate::engine::{self, Commitment};
use crate::events::{Failure, GameplayEvent};

/// Granting, activating, committing and ending abilities, as commands.
pub trait AbilityCommands {
    /// Grants `ability` to this actor, at `level`.
    fn grant_ability(&mut self, ability: impl Into<Id<Ability>>, level: f32) -> &mut Self;

    /// Activates this actor's `ability`: on a client, predicted, and asked of
    /// the server.
    fn activate_ability(&mut self, ability: impl Into<Id<Ability>>) -> &mut Self;

    /// Activates this actor's `ability`, run by `B`, with `input`, what `B`
    /// takes: checked, then put on the ability next to `B`. On a client,
    /// predicted, and asked of the server with the input.
    fn activate_ability_with<B: Behaviour>(
        &mut self,
        ability: impl Into<Id<Ability>>,
        input: B::Input,
    ) -> &mut Self;

    /// Gives this active ability, run by `B`, new `input`, in place of what it
    /// had: on a client, predicted and sent to the server; where it's decided,
    /// checked, cancelling the ability if it doesn't hold.
    fn set_input<B: Behaviour>(&mut self, input: B::Input) -> &mut Self;

    /// Activates every ability of this actor's tagged any of `tags`.
    fn activate_tagged(&mut self, tags: Tags) -> &mut Self;

    /// Cancels this actor's active abilities tagged any of `tags`.
    fn cancel_abilities(&mut self, tags: Tags) -> &mut Self;

    /// Cancels every one of this actor's active abilities: as it dies.
    fn cancel_all_abilities(&mut self) -> &mut Self;

    /// Sends this actor `event`'s tag and payload, as a gameplay event.
    fn send_event(&mut self, event: GameplayEvent) -> &mut Self;

    /// Activates this granted ability.
    fn activate(&mut self) -> &mut Self;

    /// Applies this active ability's cost and cooldown, cancelling it if it
    /// can't pay: for one that commits by hand. Each once an activation.
    fn commit_ability(&mut self) -> &mut Self;

    /// Pays this active ability's cost alone, cancelling it if it can't.
    fn commit_cost(&mut self) -> &mut Self;

    /// Starts this active ability's cooldown alone.
    fn commit_cooldown(&mut self) -> &mut Self;

    /// Ends this active ability. On a client, its own prediction alone: the
    /// server's copy runs its course.
    fn end_ability(&mut self) -> &mut Self;

    /// Cancels this active ability: on a client, its own prediction, and the
    /// server's copy too.
    fn cancel_ability(&mut self) -> &mut Self;

    /// Takes this granted ability away from its actor.
    fn revoke_ability(&mut self) -> &mut Self;
}

impl AbilityCommands for EntityCommands<'_> {
    fn grant_ability(&mut self, ability: impl Into<Id<Ability>>, level: f32) -> &mut Self {
        let ability = ability.into();
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            engine::grant(entity.into_world_mut(), actor, &ability, level, None);
        })
    }

    fn activate_ability(&mut self, ability: impl Into<Id<Ability>>) -> &mut Self {
        let ability = ability.into();
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            let _ = crate::net::predict(entity.into_world_mut(), actor, &ability);
        })
    }

    fn activate_ability_with<B: Behaviour>(
        &mut self,
        ability: impl Into<Id<Ability>>,
        input: B::Input,
    ) -> &mut Self {
        let ability = ability.into();
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            let _ = crate::net::predict_with::<B>(entity.into_world_mut(), actor, &ability, input);
        })
    }

    fn set_input<B: Behaviour>(&mut self, input: B::Input) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let ability = entity.id();
            let _ = engine::set_input(entity.into_world_mut(), ability, Some(&B::ID), input);
        })
    }

    fn activate_tagged(&mut self, tags: Tags) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            engine::activate_tagged(entity.into_world_mut(), actor, &tags);
        })
    }

    fn cancel_abilities(&mut self, tags: Tags) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            engine::cancel_tagged(entity.into_world_mut(), actor, Some(&tags));
        })
    }

    fn cancel_all_abilities(&mut self) -> &mut Self {
        self.queue(|entity: EntityWorldMut| {
            let actor = entity.id();
            engine::cancel_tagged(entity.into_world_mut(), actor, None);
        })
    }

    fn send_event(&mut self, event: GameplayEvent) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            engine::send(
                entity.into_world_mut(),
                GameplayEvent {
                    entity: actor,
                    ..event
                },
            );
        })
    }

    fn activate(&mut self) -> &mut Self {
        self.queue(|entity: EntityWorldMut| {
            let ability = entity.id();
            let _ = engine::activate(entity.into_world_mut(), ability, None, None);
        })
    }

    fn commit_ability(&mut self) -> &mut Self {
        self.queue(|entity: EntityWorldMut| {
            let ability = entity.id();
            engine::commit(entity.into_world_mut(), ability, Commitment::Both);
        })
    }

    fn commit_cost(&mut self) -> &mut Self {
        self.queue(|entity: EntityWorldMut| {
            let ability = entity.id();
            engine::commit(entity.into_world_mut(), ability, Commitment::Cost);
        })
    }

    fn commit_cooldown(&mut self) -> &mut Self {
        self.queue(|entity: EntityWorldMut| {
            let ability = entity.id();
            engine::commit(entity.into_world_mut(), ability, Commitment::Cooldown);
        })
    }

    fn end_ability(&mut self) -> &mut Self {
        self.queue(|entity: EntityWorldMut| {
            let ability = entity.id();
            engine::end(entity.into_world_mut(), ability, false);
        })
    }

    fn cancel_ability(&mut self) -> &mut Self {
        self.queue(|entity: EntityWorldMut| {
            let ability = entity.id();
            engine::cancel(entity.into_world_mut(), ability);
        })
    }

    fn revoke_ability(&mut self) -> &mut Self {
        self.queue(|entity: EntityWorldMut| {
            let ability = entity.id();
            engine::revoke(entity.into_world_mut(), ability);
        })
    }
}

/// Granting, activating and ending abilities on the world, at once.
///
/// In an exclusive system, or a test.
pub trait AbilityWorld {
    /// Grants `ability` to `actor` at `level`: the granted ability, if there's
    /// such an ability.
    fn grant_ability(
        &mut self,
        actor: Entity,
        ability: impl Into<Id<Ability>>,
        level: f32,
    ) -> Option<Entity>;

    /// Activates `actor`'s `ability` (on a client, predicted, and asked of the
    /// server): the granted ability, or why it didn't.
    ///
    /// # Errors
    ///
    /// Why it didn't activate.
    fn activate_ability(
        &mut self,
        actor: Entity,
        ability: impl Into<Id<Ability>>,
    ) -> Result<Entity, Failure>;

    /// Activates `actor`'s `ability`, run by `B`, with `input`, what `B` takes
    /// (on a client, predicted, and asked of the server with it): the granted
    /// ability, or why it didn't.
    ///
    /// # Errors
    ///
    /// Why it didn't activate: [`Failure::Input`] if `ability` isn't run by
    /// `B`, or `input` doesn't pass its check.
    fn activate_ability_with<B: Behaviour>(
        &mut self,
        actor: Entity,
        ability: impl Into<Id<Ability>>,
        input: B::Input,
    ) -> Result<Entity, Failure>;

    /// Gives the active `ability` new `input`, in place of what it had (on a
    /// client, predicted and sent to the server).
    ///
    /// # Errors
    ///
    /// [`Failure::Input`] if it isn't active, or isn't run by `B`, or (where
    /// it's decided) `input` doesn't pass its check, which cancels it.
    fn set_input<B: Behaviour>(&mut self, ability: Entity, input: B::Input) -> Result<(), Failure>;

    /// Cancels `actor`'s active abilities tagged any of `tags`: those it
    /// cancelled.
    fn cancel_abilities(&mut self, actor: Entity, tags: &Tags) -> Vec<Entity>;

    /// Cancels every one of `actor`'s active abilities: those it cancelled.
    fn cancel_all_abilities(&mut self, actor: Entity) -> Vec<Entity>;

    /// Activates the granted ability `ability`.
    ///
    /// # Errors
    ///
    /// Why it didn't activate.
    fn activate(&mut self, ability: Entity) -> Result<(), Failure>;

    /// Applies the active `ability`'s cost and cooldown, each once an
    /// activation: whether it could pay. One that can't is cancelled.
    fn commit_ability(&mut self, ability: Entity) -> bool;

    /// Pays the active `ability`'s cost alone: whether it could. One that
    /// can't is cancelled.
    fn commit_cost(&mut self, ability: Entity) -> bool;

    /// Starts the active `ability`'s cooldown alone: whether it's active.
    fn commit_cooldown(&mut self, ability: Entity) -> bool;

    /// Whether the granted `ability` could activate now, and if not, why:
    /// every check activating it makes, with nothing done.
    ///
    /// # Errors
    ///
    /// Why it couldn't activate.
    fn can_activate(&self, ability: Entity) -> Result<(), Failure>;

    /// Whether the granted `ability`'s owner could pay its cost now.
    fn can_afford(&self, ability: Entity) -> bool;

    /// The seconds left on the granted `ability`'s cooldown, if it's cooling
    /// down: the longest of the effects granting what its cooldown grants;
    /// forever if none of them runs out.
    fn cooldown_remaining(&self, ability: Entity) -> Option<f32>;

    /// Ends the active `ability`. On a client, its own prediction alone.
    fn end_ability(&mut self, ability: Entity);

    /// Cancels the active `ability`: on a client, its own prediction, and the
    /// server's copy too.
    fn cancel_ability(&mut self, ability: Entity);

    /// Sends `event` to its actor.
    fn send_event(&mut self, event: GameplayEvent);
}

impl AbilityWorld for World {
    fn grant_ability(
        &mut self,
        actor: Entity,
        ability: impl Into<Id<Ability>>,
        level: f32,
    ) -> Option<Entity> {
        engine::grant(self, actor, &ability.into(), level, None)
    }

    fn activate_ability(
        &mut self,
        actor: Entity,
        ability: impl Into<Id<Ability>>,
    ) -> Result<Entity, Failure> {
        crate::net::predict(self, actor, &ability.into())
    }

    fn activate_ability_with<B: Behaviour>(
        &mut self,
        actor: Entity,
        ability: impl Into<Id<Ability>>,
        input: B::Input,
    ) -> Result<Entity, Failure> {
        crate::net::predict_with::<B>(self, actor, &ability.into(), input)
    }

    fn set_input<B: Behaviour>(&mut self, ability: Entity, input: B::Input) -> Result<(), Failure> {
        engine::set_input(self, ability, Some(&B::ID), input)
    }

    fn cancel_abilities(&mut self, actor: Entity, tags: &Tags) -> Vec<Entity> {
        engine::cancel_tagged(self, actor, Some(tags))
    }

    fn cancel_all_abilities(&mut self, actor: Entity) -> Vec<Entity> {
        engine::cancel_tagged(self, actor, None)
    }

    fn activate(&mut self, ability: Entity) -> Result<(), Failure> {
        engine::activate(self, ability, None, None)
    }

    fn commit_ability(&mut self, ability: Entity) -> bool {
        engine::commit(self, ability, Commitment::Both)
    }

    fn commit_cost(&mut self, ability: Entity) -> bool {
        engine::commit(self, ability, Commitment::Cost)
    }

    fn commit_cooldown(&mut self, ability: Entity) -> bool {
        engine::commit(self, ability, Commitment::Cooldown)
    }

    fn can_activate(&self, ability: Entity) -> Result<(), Failure> {
        engine::can_activate(self, ability)
    }

    fn can_afford(&self, ability: Entity) -> bool {
        engine::can_afford(self, ability)
    }

    fn cooldown_remaining(&self, ability: Entity) -> Option<f32> {
        engine::cooldown_remaining(self, ability)
    }

    fn end_ability(&mut self, ability: Entity) {
        engine::end(self, ability, false);
    }

    fn cancel_ability(&mut self, ability: Entity) {
        engine::cancel(self, ability);
    }

    fn send_event(&mut self, event: GameplayEvent) {
        engine::send(self, event);
    }
}
