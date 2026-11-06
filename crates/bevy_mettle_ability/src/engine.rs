//! Granting, activating, committing and ending abilities, on the world.

use bevy::prelude::*;
use bevy_mettle_attribute::{AttributeContext, AttributeWorld, Operation};
use bevy_mettle_core::Id;
use bevy_mettle_core::condition::Context;
use bevy_mettle_core::tag::{OwnedTags, Tag, TagCounts, Tags};
use bevy_mettle_effect::{EffectRegistry, EffectSpec, EffectWorld};

use crate::{
    Abilities, Ability, AbilityActivated, AbilityEnded, AbilityFailed, AbilityOf, AbilityRegistry,
    ActiveAbility, Behaviours, Commit, Failure, GameplayEvent, GrantedAbility, Trigger,
};

/// The next activation's number.
#[derive(Resource, Default)]
pub(crate) struct Activations(u64);

/// What an ability's requirement is weighed in: its own tags, and its
/// owner's (as `OwnerHas`, `TargetHas` and `SourceHas`: it's its own
/// source).
struct Check<'a> {
    tags: &'a Tags,
    owner: &'a TagCounts,
}

impl Context for Check<'_> {
    fn tag(&self, tag: &Id<Tag>) -> bool {
        self.tags.contains(tag)
    }

    fn has(&self, tag: &Id<Tag>) -> bool {
        self.owner.contains(tag)
    }

    fn target(&self, tag: &Id<Tag>) -> bool {
        self.owner.contains(tag)
    }

    fn source(&self, tag: &Id<Tag>) -> bool {
        self.owner.contains(tag)
    }

    fn fact(&self, _fact: &Id<Tag>) -> Option<f32> {
        None
    }
}

/// The abilities granted to `actor`.
fn abilities_of(world: &World, actor: Entity) -> Vec<Entity> {
    world
        .get::<Abilities>(actor)
        .map(|abilities| abilities.iter().collect())
        .unwrap_or_default()
}

/// Grants `id` to `actor` at `level`, from `source` if an effect grants
/// it: the granted ability's entity, if there's such an ability.
pub(crate) fn grant(
    world: &mut World,
    actor: Entity,
    id: &Id<Ability>,
    level: f32,
    source: Option<Entity>,
) -> Option<Entity> {
    let Some(def) = world
        .get_resource::<AbilityRegistry>()
        .and_then(|registry| registry.get(id).cloned())
    else {
        warn!("there's no ability called `{id}`");
        return None;
    };
    Some(
        world
            .spawn((GrantedAbility::new(def, level, source), AbilityOf(actor)))
            .id(),
    )
}

/// Takes `ability` away from its actor, cancelling it if it's active.
pub(crate) fn revoke(world: &mut World, ability: Entity) {
    end(world, ability, true);
    if world.get_entity(ability).is_ok() {
        world.despawn(ability);
    }
}

/// The next activation's number: a server's own with the top bit set, so
/// they're apart from those its clients predict under.
pub(crate) fn next_activation(world: &mut World) -> u64 {
    let server = crate::net::role(world) == bevy_mettle_effect::NetRole::Server;
    let mut activations = world.get_resource_or_init::<Activations>();
    activations.0 += 1;
    if server {
        activations.0 | 1 << 63
    } else {
        activations.0
    }
}

/// Activates `id` for `actor`, under `activation` if it's given: its
/// first granted ability of that id.
pub(crate) fn activate_id(
    world: &mut World,
    actor: Entity,
    id: &Id<Ability>,
    activation: Option<u64>,
) -> Result<Entity, Failure> {
    let granted = abilities_of(world, actor).into_iter().find(|ability| {
        world
            .get::<GrantedAbility>(*ability)
            .is_some_and(|granted| granted.def.id == *id)
    });
    let Some(ability) = granted else {
        world.trigger(AbilityFailed {
            entity: actor,
            id: id.clone(),
            reason: Failure::NotGranted,
        });
        return Err(Failure::NotGranted);
    };
    activate(world, ability, None, activation).map(|()| ability)
}

/// Activates every ability granted to `actor` tagged any of `tags`: those
/// that did.
pub(crate) fn activate_tagged(world: &mut World, actor: Entity, tags: &Tags) -> Vec<Entity> {
    let tagged: Vec<Entity> = abilities_of(world, actor)
        .into_iter()
        .filter(|ability| {
            world
                .get::<GrantedAbility>(*ability)
                .is_some_and(|granted| granted.def.tags.contains_any(tags))
        })
        .collect();
    tagged
        .into_iter()
        .filter(|ability| activate(world, *ability, None, None).is_ok())
        .collect()
}

/// Whether `ability` can activate now, and if not, why.
fn can_activate(world: &World, ability: Entity) -> Result<(), Failure> {
    let (Some(granted), Some(of)) = (
        world.get::<GrantedAbility>(ability),
        world.get::<AbilityOf>(ability),
    ) else {
        return Err(Failure::NotGranted);
    };
    let actor = of.0;
    if world.get::<ActiveAbility>(ability).is_some() {
        return Err(Failure::Active);
    }
    let Some(owner) = world.get::<OwnedTags>(actor) else {
        return Err(Failure::NotGranted);
    };
    let def = &granted.def;
    if let Some(cooldown) = &def.cooldown
        && world
            .get_resource::<EffectRegistry>()
            .and_then(|effects| effects.get(cooldown))
            .is_some_and(|effect| owner.contains_any(&effect.grants))
    {
        return Err(Failure::Cooldown);
    }
    if !affords(world, actor, granted) {
        return Err(Failure::Cost);
    }
    let blocked_by_owner = def.blocked_by.written().any(|tag| owner.contains(tag));
    let blocked_by_active = abilities_of(world, actor).into_iter().any(|other| {
        other != ability
            && world.get::<ActiveAbility>(other).is_some()
            && world
                .get::<GrantedAbility>(other)
                .is_some_and(|other| def.tags.contains_any(&other.def.blocks))
    });
    if blocked_by_owner || blocked_by_active {
        return Err(Failure::Blocked);
    }
    let check = Check {
        tags: &def.tags,
        owner: owner.counts(),
    };
    if def
        .require
        .as_ref()
        .is_some_and(|require| !require.holds(&check))
    {
        return Err(Failure::Requirement);
    }
    Ok(())
}

/// Whether `actor` can pay `granted`'s cost: applying it wouldn't take an
/// attribute below 0.
fn affords(world: &World, actor: Entity, granted: &GrantedAbility) -> bool {
    let Some(cost) = &granted.def.cost else {
        return true;
    };
    let spec = EffectSpec::new(cost.clone())
        .level(granted.level)
        .source(actor);
    let Some(changes) = world.preview_effect(actor, &spec) else {
        return true;
    };
    changes.iter().all(|change| {
        let now = world
            .value_at(actor, &change.attribute, &AttributeContext::new())
            .unwrap_or_default();
        let after = match change.op {
            Operation::Flat | Operation::Unscaled => now + change.value,
            Operation::Base | Operation::Override => change.value,
            Operation::Additive | Operation::Multiplicative => now * (1.0 + change.value),
            Operation::Divide => now / (1.0 + change.value).max(f32::EPSILON),
        };
        after >= 0.0
    })
}

/// Activates `ability`, for `event` if one triggered it, under
/// `activation` if it's given (a client's, or the server answering it).
pub(crate) fn activate(
    world: &mut World,
    ability: Entity,
    event: Option<GameplayEvent>,
    activation: Option<u64>,
) -> Result<(), Failure> {
    if let Err(reason) = can_activate(world, ability) {
        if let (Some(granted), Some(of)) = (
            world.get::<GrantedAbility>(ability),
            world.get::<AbilityOf>(ability),
        ) {
            let id = granted.def.id.clone();
            let actor = of.0;
            world.trigger(AbilityFailed {
                entity: actor,
                id,
                reason,
            });
        }
        return Err(reason);
    }
    let Some(actor) = world.get::<AbilityOf>(ability).map(|of| of.0) else {
        return Err(Failure::NotGranted);
    };
    let Some(granted) = world.get::<GrantedAbility>(ability) else {
        return Err(Failure::NotGranted);
    };
    let def = granted.def.clone();
    let level = granted.level;
    let cancelled: Vec<Entity> = abilities_of(world, actor)
        .into_iter()
        .filter(|other| {
            *other != ability
                && world.get::<ActiveAbility>(*other).is_some()
                && world
                    .get::<GrantedAbility>(*other)
                    .is_some_and(|other| other.def.tags.contains_any(&def.cancels))
        })
        .collect();
    for other in cancelled {
        end(world, other, true);
    }
    let activation = match activation {
        Some(activation) => activation,
        None => next_activation(world),
    };
    if let Some(mut owned) = world.get_mut::<OwnedTags>(actor) {
        owned.add_all(&def.grants);
    }
    if let Some(mut granted) = world.get_mut::<GrantedAbility>(ability) {
        granted.granted = def.grants.clone();
        granted.committed = false;
    }
    let behaviour = world
        .get_resource::<Behaviours>()
        .and_then(|behaviours| behaviours.0.get(&def.behaviour).copied());
    let mut entity = world.entity_mut(ability);
    entity.insert(ActiveAbility {
        actor,
        activation,
        level,
        event,
    });
    if let Some(behaviour) = behaviour {
        (behaviour.insert)(&mut entity);
    }
    if def.commit == Commit::OnActivate {
        commit(world, ability);
    }
    world.trigger(AbilityActivated {
        entity: actor,
        ability,
        id: def.id.clone(),
        activation,
    });
    Ok(())
}

/// Applies `ability`'s cost and cooldown, under its activation: whether it
/// could pay. One that can't is cancelled.
pub(crate) fn commit(world: &mut World, ability: Entity) -> bool {
    let (Some(granted), Some(active)) = (
        world.get::<GrantedAbility>(ability),
        world.get::<ActiveAbility>(ability),
    ) else {
        return false;
    };
    if granted.committed {
        return true;
    }
    let actor = active.actor;
    let activation = active.activation;
    if !affords(world, actor, granted) {
        end(world, ability, true);
        return false;
    }
    let level = granted.level;
    let effects: Vec<Id<_>> = granted
        .def
        .cost
        .iter()
        .chain(&granted.def.cooldown)
        .cloned()
        .collect();
    for effect in effects {
        world.apply_effect(
            actor,
            EffectSpec::new(effect)
                .level(level)
                .source(actor)
                .activation(activation),
        );
    }
    if let Some(mut granted) = world.get_mut::<GrantedAbility>(ability) {
        granted.committed = true;
    }
    true
}

/// Ends `ability` if it's active: its behaviour taken off, the tags it
/// granted taken back; cancelled if `cancelled`.
pub(crate) fn end(world: &mut World, ability: Entity, cancelled: bool) {
    let (Some(granted), Some(active)) = (
        world.get::<GrantedAbility>(ability),
        world.get::<ActiveAbility>(ability),
    ) else {
        return;
    };
    let actor = active.actor;
    let id = granted.def.id.clone();
    let behaviour = world
        .get_resource::<Behaviours>()
        .and_then(|behaviours| behaviours.0.get(&granted.def.behaviour).copied());
    let granted_tags = world
        .get_mut::<GrantedAbility>(ability)
        .map(|mut granted| std::mem::take(&mut granted.granted))
        .unwrap_or_default();
    if let Some(mut owned) = world.get_mut::<OwnedTags>(actor) {
        owned.remove_all(&granted_tags);
    }
    let mut entity = world.entity_mut(ability);
    entity.remove::<ActiveAbility>();
    if let Some(behaviour) = behaviour {
        (behaviour.remove)(&mut entity);
    }
    world.trigger(AbilityEnded {
        entity: actor,
        ability,
        id,
        cancelled,
    });
}

/// Sends `event` to its actor: told, and the abilities it triggers
/// activated with it.
pub(crate) fn send(world: &mut World, event: GameplayEvent) {
    let actor = event.entity;
    let triggered: Vec<Entity> = abilities_of(world, actor)
        .into_iter()
        .filter(|ability| {
            world
                .get::<GrantedAbility>(*ability)
                .is_some_and(|granted| {
                    granted.def.triggers.iter().any(|trigger| {
                    matches!(trigger, Trigger::Event(tag) if event.tag.is_within(tag))
                })
                })
        })
        .collect();
    world.trigger(event.clone());
    if crate::net::defers(world) {
        return;
    }
    for ability in triggered {
        let _ = activate(world, ability, Some(event.clone()), None);
    }
}

/// `actor` came to own `tag`: abilities it triggers activated.
pub(crate) fn tag_added(world: &mut World, actor: Entity, tag: &Id<Tag>) {
    if crate::net::defers(world) {
        return;
    }
    let triggered: Vec<Entity> = abilities_of(world, actor)
        .into_iter()
        .filter(|ability| {
            world
                .get::<GrantedAbility>(*ability)
                .is_some_and(|granted| {
                    granted.def.triggers.iter().any(|trigger| match trigger {
                        Trigger::TagAdded(on) | Trigger::TagPresent(on) => on == tag,
                        Trigger::Event(_) => false,
                    })
                })
        })
        .collect();
    for ability in triggered {
        let _ = activate(world, ability, None, None);
    }
}

/// `actor` no longer owns `tag`: abilities present on it cancelled.
pub(crate) fn tag_removed(world: &mut World, actor: Entity, tag: &Id<Tag>) {
    if crate::net::defers(world) {
        return;
    }
    let triggered: Vec<Entity> = abilities_of(world, actor)
        .into_iter()
        .filter(|ability| {
            world
                .get::<GrantedAbility>(*ability)
                .is_some_and(|granted| {
                    granted
                        .def
                        .triggers
                        .iter()
                        .any(|trigger| matches!(trigger, Trigger::TagPresent(on) if on == tag))
                })
        })
        .collect();
    for ability in triggered {
        end(world, ability, true);
    }
}

/// An effect applied to `actor`: the abilities it grants, granted.
pub(crate) fn effect_applied(world: &mut World, actor: Entity, effect: Entity) {
    if crate::net::defers(world) {
        return;
    }
    let Some(active) = world.get::<bevy_mettle_effect::ActiveEffect>(effect) else {
        return;
    };
    let level = active.spec().level;
    let Some(registry) = world.get_resource::<AbilityRegistry>() else {
        return;
    };
    let abilities = registry.granted_by(active.id()).to_vec();
    for ability in abilities {
        grant(world, actor, &ability, level, Some(effect));
    }
}

/// An effect taken off `actor`: the abilities it granted, taken away.
pub(crate) fn effect_removed(world: &mut World, actor: Entity, effect: Entity) {
    if crate::net::defers(world) {
        return;
    }
    let granted: Vec<Entity> = abilities_of(world, actor)
        .into_iter()
        .filter(|ability| {
            world
                .get::<GrantedAbility>(*ability)
                .is_some_and(|granted| granted.source == Some(effect))
        })
        .collect();
    for ability in granted {
        revoke(world, ability);
    }
}
