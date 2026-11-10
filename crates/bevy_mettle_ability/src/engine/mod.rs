//! Granting, activating, committing and ending abilities, on the world.

mod activate;
mod check;
mod commit;
mod end;
mod react;

use bevy::prelude::*;
use bevy_mettle_core::Activation;
use bevy_mettle_core::condition::Context;
use bevy_mettle_core::registry::AbilityId;
use bevy_mettle_core::tag::{OwnedTags, TagCounts, TagKey, Tags};

pub(crate) use self::activate::{activate, activate_id, activate_id_with, mirror, set_input};
pub(crate) use self::check::{can_activate, can_afford, cooldown_remaining};
pub(crate) use self::commit::{Commitment, commit};
pub(crate) use self::end::{
    Ended, Interrupting, cancel, cancel_tagged, end, end_quietly, ended_unasked, interrupt,
    note_interrupt,
};
pub(crate) use self::react::{effect_applied, event_sent, tag_added, tag_removed, ungranted};
use crate::AbilityRegistry;
use crate::active::{Abilities, AbilityOf, ActiveAbility, GrantedAbility, GrantedBy};

/// How many activations this world has made: the next one's count.
#[derive(Resource, Default)]
pub(crate) struct Activations(u64);

/// What an ability's requirement is weighed in: its own tags, and its owner's
/// (as `OwnerHas`, `TargetHas` and `SourceHas`: it's its own source).
struct Check<'a> {
    tags: &'a Tags,
    owner: &'a TagCounts,
}

impl<'a> Check<'a> {
    /// What `granted`'s conditions are weighed in, its owner owning `owner`.
    fn of(granted: &'a GrantedAbility, owner: &'a OwnedTags) -> Self {
        Self {
            tags: &granted.def.tags,
            owner: owner.counts(),
        }
    }
}

/// Whether what interrupts `granted` holds, its owner owning `owner`.
fn interrupts(granted: &GrantedAbility, owner: &OwnedTags) -> bool {
    granted
        .def
        .end_if
        .as_ref()
        .is_some_and(|end_if| end_if.holds(&Check::of(granted, owner)))
}

impl Context for Check<'_> {
    fn tag(&self, tag: TagKey) -> bool {
        self.tags.contains(tag)
    }

    fn has(&self, tag: TagKey) -> bool {
        self.owner.contains(tag)
    }

    fn target(&self, tag: TagKey) -> bool {
        self.owner.contains(tag)
    }

    fn source(&self, tag: TagKey) -> bool {
        self.owner.contains(tag)
    }

    fn fact(&self, _fact: TagKey) -> Option<f32> {
        None
    }
}

/// The abilities granted to `actor`.
pub(crate) fn abilities_of(world: &World, actor: Entity) -> Vec<Entity> {
    world
        .get::<Abilities>(actor)
        .map(|abilities| abilities.iter().collect())
        .unwrap_or_default()
}

/// Grants `id` to `actor` at `level`, by the effect `by` if one grants it:
/// the granted ability's entity, if there's such an ability.
pub(crate) fn grant(
    world: &mut World,
    actor: Entity,
    id: AbilityId,
    level: f32,
    by: Option<Entity>,
) -> Option<Entity> {
    let Some(def) = world
        .get_resource::<AbilityRegistry>()
        .and_then(|registry| registry.get(id).cloned())
    else {
        warn!("there's no ability {id:?}: content reloaded without it");
        return None;
    };
    let mut granted = world.spawn((GrantedAbility::new(def, level), AbilityOf(actor)));
    if let Some(effect) = by {
        granted.insert(GrantedBy(effect));
    }
    Some(granted.id())
}

/// Takes `ability` away from its actor: despawned, cancelled if it's
/// active, as [`GrantedAbility`]'s hook does.
pub(crate) fn revoke(world: &mut World, ability: Entity) {
    if world.get_entity(ability).is_ok() {
        world.despawn(ability);
    }
}

/// The next activation: a server's own, apart from those its clients
/// predict under, or this world's own.
pub(crate) fn next_activation(world: &mut World) -> Activation {
    let server = crate::net::role(world) == bevy_mettle_core::net::NetRole::Server;
    let mut activations = world.get_resource_or_init::<Activations>();
    activations.0 += 1;
    if server {
        Activation::server(activations.0)
    } else {
        Activation::client(activations.0)
    }
}

/// Gives every granted ability its definition from the registry, as it's
/// replaced: one whose ability is gone is taken away, and an active one the
/// new definition interrupts is cancelled.
pub(crate) fn relink(world: &mut World, granted: &mut QueryState<Entity, With<GrantedAbility>>) {
    let registry = world.resource::<AbilityRegistry>().clone();
    let all: Vec<Entity> = granted.iter(world).collect();
    let mut relinked = Vec::new();
    for ability in all {
        let Some(granted) = world.get::<GrantedAbility>(ability) else {
            continue;
        };
        match registry.get(granted.def.id).cloned() {
            None => revoke(world, ability),
            Some(def) if !std::sync::Arc::ptr_eq(&def, &granted.def) => {
                if let Some(mut granted) = world.get_mut::<GrantedAbility>(ability) {
                    granted.def = def;
                }
                if let Some(active) = world.get::<ActiveAbility>(ability)
                    && !relinked.contains(&active.actor)
                {
                    relinked.push(active.actor);
                }
            }
            Some(_) => {}
        }
    }
    for actor in relinked {
        interrupt(world, actor);
    }
}
