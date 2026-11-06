//! Granting, activating and ending abilities: as commands, or on the
//! world.

use bevy::prelude::*;
use bevy_mettle_core::Id;
use bevy_mettle_core::tag::Tags;

use crate::{Ability, Failure, GameplayEvent, engine};

/// Granting and activating abilities on an actor, and committing and
/// ending a granted ability, as commands.
pub trait AbilityCommands {
    /// Grants `ability` to this actor, at `level`.
    fn grant_ability(&mut self, ability: impl Into<Id<Ability>>, level: f32) -> &mut Self;

    /// Activates this actor's `ability`: on a client, predicted, and asked
    /// of the server.
    fn activate_ability(&mut self, ability: impl Into<Id<Ability>>) -> &mut Self;

    /// Activates every ability of this actor's tagged any of `tags`.
    fn activate_tagged(&mut self, tags: Tags) -> &mut Self;

    /// Sends this actor `event`'s tag and payload, as a gameplay event.
    fn send_event(&mut self, event: GameplayEvent) -> &mut Self;

    /// Activates this granted ability.
    fn activate(&mut self) -> &mut Self;

    /// Applies this active ability's cost and cooldown, cancelling it if it
    /// can't pay: for one that commits by hand.
    fn commit_ability(&mut self) -> &mut Self;

    /// Ends this active ability.
    fn end_ability(&mut self) -> &mut Self;

    /// Cancels this active ability.
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

    fn activate_tagged(&mut self, tags: Tags) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            engine::activate_tagged(entity.into_world_mut(), actor, &tags);
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
            engine::commit(entity.into_world_mut(), ability);
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
            engine::end(entity.into_world_mut(), ability, true);
        })
    }

    fn revoke_ability(&mut self) -> &mut Self {
        self.queue(|entity: EntityWorldMut| {
            let ability = entity.id();
            engine::revoke(entity.into_world_mut(), ability);
        })
    }
}

/// Granting, activating and ending abilities on the world, at once: in an
/// exclusive system, or a test.
pub trait AbilityWorld {
    /// Grants `ability` to `actor` at `level`: the granted ability, if
    /// there's such an ability.
    fn grant_ability(
        &mut self,
        actor: Entity,
        ability: impl Into<Id<Ability>>,
        level: f32,
    ) -> Option<Entity>;

    /// Activates `actor`'s `ability` (on a client, predicted, and asked of
    /// the server): the granted ability, or why it didn't.
    ///
    /// # Errors
    ///
    /// Why it didn't activate.
    fn activate_ability(
        &mut self,
        actor: Entity,
        ability: impl Into<Id<Ability>>,
    ) -> Result<Entity, Failure>;

    /// Activates the granted ability `ability`.
    ///
    /// # Errors
    ///
    /// Why it didn't activate.
    fn activate(&mut self, ability: Entity) -> Result<(), Failure>;

    /// Applies the active `ability`'s cost and cooldown: whether it could
    /// pay. One that can't is cancelled.
    fn commit_ability(&mut self, ability: Entity) -> bool;

    /// Ends the active `ability`; cancelled if `cancelled`.
    fn end_ability(&mut self, ability: Entity, cancelled: bool);

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

    fn activate(&mut self, ability: Entity) -> Result<(), Failure> {
        engine::activate(self, ability, None, None)
    }

    fn commit_ability(&mut self, ability: Entity) -> bool {
        engine::commit(self, ability)
    }

    fn end_ability(&mut self, ability: Entity, cancelled: bool) {
        engine::end(self, ability, cancelled);
    }

    fn send_event(&mut self, event: GameplayEvent) {
        engine::send(self, event);
    }
}
