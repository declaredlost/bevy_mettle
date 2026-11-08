//! Effect and ability replicas over lightyear: sent with their actor, to its
//! owner alone or to everyone who sees it.

use bevy::prelude::*;
use bevy_mettle_ability::active::{Abilities, AbilityOf};
use bevy_mettle_ability::net::replica::AbilityReplica;
use bevy_mettle_effect::active::{ActiveEffects, EffectOf};
use bevy_mettle_effect::net::EffectReplica;
use bevy_replicon::prelude::VisibilityFilter;
use lightyear::prelude::*;

use super::Replicas;

/// Effects and abilities that aren't replicated yet.
type Unreplicated = (
    Without<Replicate>,
    Without<ReplicateLike>,
    Or<(With<EffectReplica>, With<AbilityReplica>)>,
);

/// On a server: replicates each new effect and granted ability as its actor
/// is, to its owner alone if [`Replicas`] says.
pub(super) fn replicate(
    mut commands: Commands,
    new: Query<(Entity, Option<&EffectOf>, Option<&AbilityOf>), Unreplicated>,
    controlled: Query<&ControlledBy>,
    replicas: Res<Replicas>,
) {
    for (entity, effect, ability) in &new {
        let Some(actor) = effect.map(|of| of.0).or(ability.map(|of| of.0)) else {
            continue;
        };
        let mut entity = commands.entity(entity);
        entity.insert(ReplicateLike { root: actor });
        if *replicas == Replicas::Owner {
            entity.insert(ReplicaOwner(
                controlled
                    .get(actor)
                    .ok()
                    .map(|controlled| controlled.owner),
            ));
        }
    }
}

/// On an effect or ability replica, owner-only: the link of the client that
/// may see it, if one controls its actor.
///
/// A visibility filter, so it's only ever narrower than its actor's: the
/// replica still comes and goes with its actor ([`ReplicateLike`]), and the
/// owner alone of those who see the actor sees it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[component(immutable)]
pub(super) struct ReplicaOwner(Option<Entity>);

impl VisibilityFilter for ReplicaOwner {
    type ClientComponent = Self;
    type Scope = Entity;

    fn is_visible(&self, client: Entity, _: Option<&Self>) -> bool {
        self.0 == Some(client)
    }
}

/// An actor whose control came, changed or went, and its replicas.
type Handed = (
    Option<&'static ControlledBy>,
    Option<&'static ActiveEffects>,
    Option<&'static Abilities>,
);

/// On a server, owner-only: points each actor's replicas at its owner again
/// as control of it comes, changes hands, or goes.
pub(super) fn retarget(
    mut commands: Commands,
    handed: Query<Handed>,
    changed: Query<Entity, Changed<ControlledBy>>,
    mut released: RemovedComponents<ControlledBy>,
    replicas: Res<Replicas>,
) {
    if *replicas != Replicas::Owner {
        return;
    }
    for actor in changed.iter().chain(released.read()) {
        let Ok((controlled, effects, abilities)) = handed.get(actor) else {
            continue;
        };
        let owner = ReplicaOwner(controlled.map(|controlled| controlled.owner));
        let replicas = effects
            .into_iter()
            .flat_map(ActiveEffects::iter)
            .chain(abilities.into_iter().flat_map(Abilities::iter));
        for replica in replicas {
            commands.entity(replica).insert(owner);
        }
    }
}
