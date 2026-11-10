//! Granting, activating and ending abilities: as commands, or on the world.

use bevy::prelude::*;
use bevy_mettle_core::registry::{AbilityId, AnyAbility, Ref};
use bevy_mettle_core::tag::Tags;

use crate::AbilityRegistry;
use crate::ability::{Ability, AbilitySpec};
use crate::behaviour::{Behaviour, BehaviourId, BehaviourRegistry};
use crate::engine::{self, Commitment};
use crate::events::{Activated, Failure};

/// Granting, activating, committing and ending abilities, as commands.
pub trait AbilityCommands {
    /// Grants `spec`'s ability to this actor, at its level: an [`Ability`]
    /// type (`Fireball`, `Fireball.level(2.0)`).
    fn grant_ability(&mut self, spec: impl Into<AbilitySpec>) -> &mut Self;

    /// Activates this actor's `ability`: on a client, predicted, and asked of
    /// the server.
    fn activate_ability(&mut self, ability: impl Ability) -> &mut Self;

    /// Activates this actor's `ability`, run by `B`, with `input`, what `B`
    /// takes: checked, then put on the ability next to `B`. On a client,
    /// predicted, and asked of the server with the input.
    fn activate_ability_with<B: Behaviour>(
        &mut self,
        ability: impl Ability,
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
}

impl AbilityCommands for EntityCommands<'_> {
    fn grant_ability(&mut self, spec: impl Into<AbilitySpec>) -> &mut Self {
        let spec = spec.into();
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            let world = entity.into_world_mut();
            if let Some(ability) = resolve(world, spec.named()) {
                engine::grant(world, actor, ability, spec.level, None);
            }
        })
    }

    fn activate_ability(&mut self, ability: impl Ability) -> &mut Self {
        let named = named(&ability);
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            let world = entity.into_world_mut();
            if let Some(ability) = resolve(world, named) {
                let _ = crate::net::predict(world, actor, ability);
            }
        })
    }

    fn activate_ability_with<B: Behaviour>(
        &mut self,
        ability: impl Ability,
        input: B::Input,
    ) -> &mut Self {
        let named = named(&ability);
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            let world = entity.into_world_mut();
            if let Some(ability) = resolve(world, named) {
                let _ = crate::net::predict_with::<B>(world, actor, ability, input);
            }
        })
    }

    fn set_input<B: Behaviour>(&mut self, input: B::Input) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let ability = entity.id();
            let world = entity.into_world_mut();
            let behaviour = behaviour_id::<B>(world);
            let _ = engine::set_input(world, ability, behaviour, input);
        })
    }

    fn activate_tagged(&mut self, tags: Tags) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            crate::net::predict_tagged(entity.into_world_mut(), actor, &tags);
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

    fn activate(&mut self) -> &mut Self {
        self.queue(|entity: EntityWorldMut| {
            let ability = entity.id();
            let _ = crate::net::predict_granted(entity.into_world_mut(), ability);
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
}

/// Granting, activating and ending abilities on the world, at once.
///
/// In an exclusive system, or a test. Asking (`can_activate`, `can_afford`,
/// `cooldown_remaining`) takes `&self`, so a system taking `&World` asks too,
/// for a UI greying out what can't be cast, or an AI choosing what to cast:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mettle_ability::prelude::*;
/// fn buttons(abilities: Query<Entity, With<GrantedAbility>>, world: &World) {
///     for ability in &abilities {
///         let usable = world.can_activate(ability).is_ok();
///         let cooling = world.cooldown_remaining(ability);
///         let _ = (usable, cooling);
///     }
/// }
/// # App::new().add_systems(Update, buttons);
/// ```
pub trait AbilityWorld {
    /// Grants `spec`'s ability to `actor` at its level: the granted ability,
    /// if there's such an ability.
    fn grant_ability(&mut self, actor: Entity, spec: impl Into<AbilitySpec>) -> Option<Entity>;

    /// Activates `actor`'s `ability`, as it runs over a network: what it came
    /// to, running here or asked of the server, or why it didn't.
    ///
    /// # Errors
    ///
    /// Why it didn't activate.
    fn activate_ability(
        &mut self,
        actor: Entity,
        ability: impl Ability,
    ) -> Result<Activated, Failure>;

    /// Activates `actor`'s `ability`, run by `B`, with `input`, what `B` takes,
    /// as it runs over a network: what it came to, running here or asked of
    /// the server with the input, or why it didn't.
    ///
    /// # Errors
    ///
    /// Why it didn't activate: [`Failure::Input`] if `ability` isn't run by
    /// `B`, or `input` doesn't pass its check.
    fn activate_ability_with<B: Behaviour>(
        &mut self,
        actor: Entity,
        ability: impl Ability,
        input: B::Input,
    ) -> Result<Activated, Failure>;

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

    /// Activates the granted ability `ability`, as it runs over a network:
    /// what it came to, running here or asked of the server.
    ///
    /// # Errors
    ///
    /// Why it didn't activate.
    fn activate(&mut self, ability: Entity) -> Result<Activated, Failure>;

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
    /// down: the longest of the effects giving its owner what its cooldown
    /// does; forever if none of them runs out.
    fn cooldown_remaining(&self, ability: Entity) -> Option<f32>;

    /// Ends the active `ability`. On a client, its own prediction alone.
    fn end_ability(&mut self, ability: Entity);

    /// Cancels the active `ability`: on a client, its own prediction, and the
    /// server's copy too.
    fn cancel_ability(&mut self, ability: Entity);
}

impl AbilityWorld for World {
    fn grant_ability(&mut self, actor: Entity, spec: impl Into<AbilitySpec>) -> Option<Entity> {
        let spec = spec.into();
        let ability = resolve(self, spec.named())?;
        engine::grant(self, actor, ability, spec.level, None)
    }

    fn activate_ability(
        &mut self,
        actor: Entity,
        ability: impl Ability,
    ) -> Result<Activated, Failure> {
        let ability = resolve(self, named(&ability)).ok_or(Failure::NotGranted)?;
        crate::net::predict(self, actor, ability)
    }

    fn activate_ability_with<B: Behaviour>(
        &mut self,
        actor: Entity,
        ability: impl Ability,
        input: B::Input,
    ) -> Result<Activated, Failure> {
        let ability = resolve(self, named(&ability)).ok_or(Failure::NotGranted)?;
        crate::net::predict_with::<B>(self, actor, ability, input)
    }

    fn set_input<B: Behaviour>(&mut self, ability: Entity, input: B::Input) -> Result<(), Failure> {
        let behaviour = behaviour_id::<B>(self).ok_or(Failure::Input)?;
        engine::set_input(self, ability, Some(behaviour), input)
    }

    fn cancel_abilities(&mut self, actor: Entity, tags: &Tags) -> Vec<Entity> {
        engine::cancel_tagged(self, actor, Some(tags))
    }

    fn cancel_all_abilities(&mut self, actor: Entity) -> Vec<Entity> {
        engine::cancel_tagged(self, actor, None)
    }

    fn activate(&mut self, ability: Entity) -> Result<Activated, Failure> {
        crate::net::predict_granted(self, ability)
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
}

/// `B`'s number in `world`'s app, if it's added.
fn behaviour_id<B: Behaviour>(world: &World) -> Option<BehaviourId> {
    world.get_resource::<BehaviourRegistry>()?.id_of::<B>()
}

/// What `ability`'s type names.
fn named<A: Ability>(_: &A) -> Ref<AnyAbility> {
    Ref::of::<A>()
}

/// What `ability` names' number in `world`'s registry; `None`, warned, if
/// there's no such ability.
fn resolve(world: &World, ability: Ref<AnyAbility>) -> Option<AbilityId> {
    let id = world
        .get_resource::<AbilityRegistry>()
        .and_then(|registry| registry.find(ability));
    if id.is_none() {
        warn!(
            "ability `{}` isn't added, or content hasn't it",
            ability.label()
        );
    }
    id
}
