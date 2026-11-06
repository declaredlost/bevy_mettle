//! Applying, executing, stacking, inhibiting and removing effects: what
//! commands and the tick run, on the world.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_attribute::{AttributeContext, Modifier, ModifierOf, Modifiers};
use bevy_mettle_core::Id;
use bevy_mettle_core::condition::Context;
use bevy_mettle_core::tag::{OwnedTags, Tag, TagCounts, Tags};

use crate::code::{BaseChange, EffectContext};
use crate::net::{self, EffectReplica};
use crate::{
    ActiveEffect, ActiveEffects, Blocked, Capture, Effect, EffectApplied, EffectBlocked,
    EffectDuration, EffectExecuted, EffectOf, EffectRegistry, EffectRemoved, EffectSpec,
    EffectStacked, Expire, Magnitude, Reapply, StackBy, Stacking,
};

/// How deep effects applying effects go before it's taken for a loop.
const DEPTH: u32 = 16;

/// What an effect's requirements and executing modifiers are weighed in:
/// its own tags, its target's (as `OwnerHas` and `TargetHas`), and its
/// source's
/// as it applied.
struct Check<'a> {
    tags: &'a Tags,
    owner: &'a TagCounts,
    source: &'a TagCounts,
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
        self.source.contains(tag)
    }

    fn fact(&self, _fact: &Id<Tag>) -> Option<f32> {
        None
    }
}

/// `stacks` as a number to multiply by: past 65535, as many.
fn count(stacks: u32) -> f32 {
    f32::from(u16::try_from(stacks).unwrap_or(u16::MAX))
}

/// Applies `spec` to `target`.
pub(crate) fn apply(world: &mut World, target: Entity, spec: EffectSpec) {
    apply_at(world, target, spec, 0);
}

/// Applies `spec` to `target`, `depth` effects deep in a chain.
fn apply_at(world: &mut World, target: Entity, spec: EffectSpec, depth: u32) {
    if depth > DEPTH {
        warn!(
            "effect `{}` applies effects more than {DEPTH} deep: taken for a loop, and stopped",
            spec.effect
        );
        return;
    }
    let Some(registry) = world.get_resource::<EffectRegistry>().cloned() else {
        warn!(
            "effect `{}` isn't applied: there's no effect registry",
            spec.effect
        );
        return;
    };
    let Some(def) = registry.get(&spec.effect).cloned() else {
        warn!("there's no effect called `{}`", spec.effect);
        return;
    };
    let source_tags = source_tags(world, spec.source);
    if world.get::<OwnedTags>(target).is_none() {
        warn!(
            "effect `{}` isn't applied to {target}: it owns no tags",
            def.id
        );
        return;
    }
    if let Some(reason) = blocked(world, target, &def, &source_tags) {
        world.trigger(EffectBlocked {
            entity: target,
            id: def.id.clone(),
            reason,
        });
        return;
    }
    if !def.removes.is_empty() {
        remove_tagged(world, target, &def.removes, depth);
    }
    if !def.lasts() {
        execute(world, &registry, target, &spec, &def, 1, &source_tags);
        chain(world, target, &spec, &def.on_apply, depth);
        return;
    }
    if let Some(stacking) = &def.stacking
        && let Some(existing) = stack_of(world, target, &def.id, stacking.by, spec.source)
    {
        stack(world, &registry, existing, stacking, &spec, depth);
        return;
    }
    let effect = spawn(world, &registry, target, &spec, &def, source_tags);
    if let Some(activation) = net::predicting(world, &spec) {
        net::spawned(world, activation, effect);
    }
    world.trigger(EffectApplied {
        entity: target,
        effect,
        id: def.id.clone(),
    });
    chain(world, target, &spec, &def.on_apply, depth);
}

/// Why `def` can't apply to `target`, if it can't.
fn blocked(
    world: &World,
    target: Entity,
    def: &Effect,
    source_tags: &TagCounts,
) -> Option<Blocked> {
    let owner = world.get::<OwnedTags>(target)?;
    let check = Check {
        tags: &def.tags,
        owner: owner.counts(),
        source: source_tags,
    };
    if def
        .require
        .as_ref()
        .is_some_and(|require| !require.holds(&check))
    {
        Some(Blocked::Requirement)
    } else if immune(world, target, &def.tags) {
        Some(Blocked::Immunity)
    } else {
        None
    }
}

/// Spawns `def`, applied as `spec`, on `target`: counting at once if its
/// ongoing requirement holds, and executing at once if it says so.
fn spawn(
    world: &mut World,
    registry: &EffectRegistry,
    target: Entity,
    spec: &EffectSpec,
    def: &Arc<Effect>,
    source_tags: TagCounts,
) -> Entity {
    let (duration, period) = {
        let context = EffectContext {
            world: &*world,
            target,
            spec,
            stacks: 1,
        };
        let duration = match &def.duration {
            EffectDuration::For(seconds) => Some(resolve(&context, registry, seconds)),
            EffectDuration::Instant | EffectDuration::Infinite => None,
        };
        let period = def
            .period
            .as_ref()
            .map(|period| resolve(&context, registry, &period.every))
            .filter(|every| {
                let fine = *every > 0.0;
                if !fine {
                    warn!(
                        "effect `{}` has a period of {every}: it executes on apply alone",
                        def.id
                    );
                }
                fine
            });
        (duration, period)
    };
    let effect = world
        .spawn((
            ActiveEffect {
                spec: spec.clone(),
                def: def.clone(),
                stacks: 1,
                duration,
                remaining: duration,
                period,
                until_period: period.unwrap_or_default(),
                inhibited: true,
                granted: Tags::new(),
                source_tags: source_tags.clone(),
            },
            EffectOf(target),
        ))
        .id();
    if holds_ongoing(world, effect) {
        activate(world, registry, effect);
        if def.period.as_ref().is_some_and(|period| period.on_apply) {
            execute(world, registry, target, spec, def, 1, &source_tags);
        }
    }
    effect
}

/// What `source` owns, if it's there.
fn source_tags(world: &World, source: Option<Entity>) -> TagCounts {
    source
        .and_then(|source| world.get::<OwnedTags>(source))
        .map(|owned| owned.counts().clone())
        .unwrap_or_default()
}

/// The effects on `target`.
fn effects_on(world: &World, target: Entity) -> Vec<Entity> {
    world
        .get::<ActiveEffects>(target)
        .map(|effects| effects.iter().collect())
        .unwrap_or_default()
}

/// Whether an effect on `target` keeps effects tagged `tags` off it.
fn immune(world: &World, target: Entity, tags: &Tags) -> bool {
    effects_on(world, target).into_iter().any(|effect| {
        world
            .get::<ActiveEffect>(effect)
            .is_some_and(|active| !active.inhibited && tags.contains_any(&active.def.immunity))
    })
}

/// The active effect on `target` that applying `id` from `source` stacks
/// onto, if there is one.
fn stack_of(
    world: &World,
    target: Entity,
    id: &Id<Effect>,
    by: StackBy,
    source: Option<Entity>,
) -> Option<Entity> {
    effects_on(world, target).into_iter().find(|effect| {
        world.get::<ActiveEffect>(*effect).is_some_and(|active| {
            active.def.id == *id && (by == StackBy::Target || active.spec.source == source)
        })
    })
}

/// Stacks `spec` onto `existing`, or, at the limit, applies its overflow
/// instead.
fn stack(
    world: &mut World,
    registry: &EffectRegistry,
    existing: Entity,
    stacking: &Stacking,
    spec: &EffectSpec,
    depth: u32,
) {
    let Some(target) = world.get::<EffectOf>(existing).map(|of| of.0) else {
        return;
    };
    let Some(mut active) = world.get_mut::<ActiveEffect>(existing) else {
        return;
    };
    let id = active.def.id.clone();
    if active.stacks >= stacking.limit {
        for overflow in &stacking.overflow {
            apply_at(world, target, spec.for_effect(overflow.clone()), depth + 1);
        }
        world.trigger(EffectBlocked {
            entity: target,
            id,
            reason: Blocked::StackLimit,
        });
        return;
    }
    active.stacks += 1;
    if stacking.reapply == Reapply::Refresh {
        active.remaining = active.duration;
    }
    if stacking.reset_period {
        active.until_period = active.period.unwrap_or_default();
    }
    let stacks = active.stacks;
    let on_apply = active.def.on_apply.clone();
    if let Some(activation) = net::predicting(world, spec) {
        net::stacked(world, activation, existing);
    }
    refresh(world, registry, existing);
    world.trigger(EffectStacked {
        entity: target,
        effect: existing,
        id,
        stacks,
    });
    chain(world, target, spec, &on_apply, depth);
}

/// Applies each of `effects` to `target`, as `spec` was: a chain.
fn chain(world: &mut World, target: Entity, spec: &EffectSpec, effects: &[Id<Effect>], depth: u32) {
    for effect in effects {
        apply_at(world, target, spec.for_effect(effect.clone()), depth + 1);
    }
}

/// How much `magnitude` is, for `context`.
fn resolve(context: &EffectContext, registry: &EffectRegistry, magnitude: &Magnitude) -> f32 {
    match magnitude {
        Magnitude::Value(value) => *value,
        Magnitude::Scaled { base, curve } => {
            base * registry
                .curves()
                .at(curve, context.spec.level)
                .unwrap_or_default()
        }
        Magnitude::Param(param) => context.spec.param_value(param).unwrap_or_else(|| {
            warn!(
                "effect `{}` reads {param:?}, which its spec doesn't set: 0",
                context.spec.effect
            );
            0.0
        }),
        Magnitude::Attribute {
            attribute,
            from,
            coefficient,
        } => {
            let whose = match from {
                Capture::Source => context.spec.source,
                Capture::Target => Some(context.target),
            };
            whose
                .and_then(|entity| context.value_at(entity, attribute, &AttributeContext::new()))
                .unwrap_or_default()
                * coefficient
        }
        Magnitude::Custom(calculation) => registry
            .calculation(calculation)
            .map_or(0.0, |calculate| calculate(context)),
    }
}

/// What executing `def` on `target` would change, without changing it:
/// its modifiers and executions, worked out.
fn changes(
    world: &World,
    registry: &EffectRegistry,
    target: Entity,
    spec: &EffectSpec,
    def: &Effect,
    stacks: u32,
    source_tags: &TagCounts,
) -> Vec<BaseChange> {
    let Some(owner) = world.get::<OwnedTags>(target) else {
        return Vec::new();
    };
    let check = Check {
        tags: &def.tags,
        owner: owner.counts(),
        source: source_tags,
    };
    let context = EffectContext {
        world,
        target,
        spec,
        stacks,
    };
    let mut changes = Vec::new();
    for modifier in &def.modifiers {
        let weight = modifier
            .condition
            .as_ref()
            .map_or(1.0, |condition| condition.weight(&check));
        if weight <= 0.0 || (modifier.op.whole() && weight < 1.0) {
            continue;
        }
        let mut value = resolve(&context, registry, &modifier.value);
        if !modifier.op.whole() {
            value *= count(stacks) * weight;
        }
        if let Some(per) = &modifier.per {
            let of = context
                .value_at(target, &per.attribute, &AttributeContext::new())
                .unwrap_or_default();
            value *= (of / per.every).floor().max(0.0);
        }
        changes.push(BaseChange {
            attribute: modifier.attribute.clone(),
            op: modifier.op,
            value,
        });
    }
    for execution in &def.executions {
        if let Some(run) = registry.execution(execution) {
            changes.extend(run(&context));
        }
    }
    changes
}

/// What applying `spec` to `target` as an instant effect would change, if
/// there's such an effect: what an ability's cost is checked by.
pub(crate) fn preview(world: &World, target: Entity, spec: &EffectSpec) -> Option<Vec<BaseChange>> {
    let registry = world.get_resource::<EffectRegistry>()?;
    let def = registry.get(&spec.effect)?;
    let source_tags = source_tags(world, spec.source);
    Some(changes(world, registry, target, spec, def, 1, &source_tags))
}

/// Executes `def` on `target`: its modifiers and executions, changing
/// bases.
fn execute(
    world: &mut World,
    registry: &EffectRegistry,
    target: Entity,
    spec: &EffectSpec,
    def: &Effect,
    stacks: u32,
    source_tags: &TagCounts,
) {
    let changes = changes(world, registry, target, spec, def, stacks, source_tags);
    let predicted = net::predicting(world, spec);
    for change in &changes {
        net::change_base(world, predicted, target, change);
    }
    world.trigger(EffectExecuted {
        entity: target,
        id: def.id.clone(),
    });
}

/// `effect`'s modifiers, resolved: amounts counted once per stack, its
/// conditions' source terms settled as its source was.
fn modifiers(world: &World, registry: &EffectRegistry, effect: Entity) -> Vec<Modifier> {
    let (Some(active), Some(of)) = (
        world.get::<ActiveEffect>(effect),
        world.get::<EffectOf>(effect),
    ) else {
        return Vec::new();
    };
    let context = EffectContext {
        world,
        target: of.0,
        spec: &active.spec,
        stacks: active.stacks,
    };
    active
        .def
        .modifiers
        .iter()
        .map(|modifier| {
            let value = resolve(&context, registry, &modifier.value);
            Modifier {
                attribute: modifier.attribute.clone(),
                op: modifier.op,
                value: if modifier.op.whole() {
                    value
                } else {
                    value * count(active.stacks)
                },
                condition: modifier.condition.as_ref().map(|condition| {
                    condition.settle_source(&|tag| active.source_tags.contains(tag))
                }),
                per: modifier.per.clone(),
                local: false,
            }
        })
        .collect()
}

/// Whether `effect`'s ongoing requirement holds.
fn holds_ongoing(world: &World, effect: Entity) -> bool {
    let (Some(active), Some(of)) = (
        world.get::<ActiveEffect>(effect),
        world.get::<EffectOf>(effect),
    ) else {
        return false;
    };
    let Some(ongoing) = &active.def.ongoing else {
        return true;
    };
    world.get::<OwnedTags>(of.0).is_some_and(|owner| {
        ongoing.holds(&Check {
            tags: &active.def.tags,
            owner: owner.counts(),
            source: &active.source_tags,
        })
    })
}

/// Makes `effect` count: its tags granted, its modifiers on its target
/// (unless it's periodic: it executes instead).
pub(crate) fn activate(world: &mut World, registry: &EffectRegistry, effect: Entity) {
    let (Some(active), Some(of)) = (
        world.get::<ActiveEffect>(effect),
        world.get::<EffectOf>(effect),
    ) else {
        return;
    };
    let target = of.0;
    let grants = active.def.grants.clone();
    let periodic = active.def.period.is_some();
    let modifiers = (!periodic).then(|| modifiers(world, registry, effect));
    if let Some(mut owned) = world.get_mut::<OwnedTags>(target) {
        owned.add_all(&grants);
    }
    if let Some(mut active) = world.get_mut::<ActiveEffect>(effect) {
        active.inhibited = false;
        active.granted = grants;
    }
    if let Some(modifiers) = modifiers {
        world
            .entity_mut(effect)
            .insert((Modifiers(modifiers), ModifierOf(target)));
    }
}

/// Makes `effect` stop counting: its tags taken back, its modifiers off
/// its target.
pub(crate) fn deactivate(world: &mut World, effect: Entity) {
    let Some(target) = world.get::<EffectOf>(effect).map(|of| of.0) else {
        return;
    };
    let Some(mut active) = world.get_mut::<ActiveEffect>(effect) else {
        return;
    };
    active.inhibited = true;
    let granted = std::mem::take(&mut active.granted);
    if let Some(mut owned) = world.get_mut::<OwnedTags>(target) {
        owned.remove_all(&granted);
    }
    world.entity_mut(effect).remove::<(Modifiers, ModifierOf)>();
}

/// Resolves `effect`'s modifiers again, if they count: as its stacks
/// change.
fn refresh(world: &mut World, registry: &EffectRegistry, effect: Entity) {
    let counts = world
        .get::<ActiveEffect>(effect)
        .is_some_and(|active| !active.inhibited && active.def.period.is_none());
    if counts {
        let modifiers = modifiers(world, registry, effect);
        world.entity_mut(effect).insert(Modifiers(modifiers));
    }
}

/// Takes `effect` off its target: run out of time if `expired`.
pub(crate) fn remove(world: &mut World, effect: Entity, expired: bool, depth: u32) {
    let (Some(active), Some(of)) = (
        world.get::<ActiveEffect>(effect),
        world.get::<EffectOf>(effect),
    ) else {
        return;
    };
    let target = of.0;
    let def = active.def.clone();
    let spec = active.spec.clone();
    if !active.inhibited {
        deactivate(world, effect);
    }
    world.despawn(effect);
    world.trigger(EffectRemoved {
        entity: target,
        effect,
        id: def.id.clone(),
        expired,
    });
    chain(world, target, &spec, &def.on_remove, depth);
    if expired {
        chain(world, target, &spec, &def.on_expire, depth);
    }
}

/// Takes every effect tagged any of `tags` off `target`.
pub(crate) fn remove_tagged(world: &mut World, target: Entity, tags: &Tags, depth: u32) {
    let tagged: Vec<Entity> = effects_on(world, target)
        .into_iter()
        .filter(|effect| {
            world
                .get::<ActiveEffect>(*effect)
                .is_some_and(|active| active.def.tags.contains_any(tags))
        })
        .collect();
    for effect in tagged {
        remove(world, effect, false, depth);
    }
}

/// Takes every effect applied under `activation` off `target`.
pub(crate) fn remove_activation(world: &mut World, target: Entity, activation: u64) {
    let applied: Vec<Entity> = effects_on(world, target)
        .into_iter()
        .filter(|effect| {
            world
                .get::<ActiveEffect>(*effect)
                .is_some_and(|active| active.spec.activation == Some(activation))
        })
        .collect();
    for effect in applied {
        remove(world, effect, false, 0);
    }
}

/// Moves `effect` on by `seconds`: its removal and ongoing requirements
/// weighed, its period come round, its time run out.
pub(crate) fn step(world: &mut World, registry: &EffectRegistry, effect: Entity, seconds: f32) {
    let (Some(active), Some(of)) = (
        world.get::<ActiveEffect>(effect),
        world.get::<EffectOf>(effect),
    ) else {
        return;
    };
    let target = of.0;
    let inhibited = active.inhibited;
    if net::role(world) == net::NetRole::Client && world.get::<net::Proxy>(effect).is_some() {
        return;
    }
    let (remove_now, ongoing) = {
        let Some(owner) = world.get::<OwnedTags>(target) else {
            return;
        };
        let check = Check {
            tags: &active.def.tags,
            owner: owner.counts(),
            source: &active.source_tags,
        };
        (
            active
                .def
                .remove_if
                .as_ref()
                .is_some_and(|condition| condition.holds(&check)),
            active
                .def
                .ongoing
                .as_ref()
                .is_none_or(|condition| condition.holds(&check)),
        )
    };
    if remove_now {
        remove(world, effect, false, 0);
        return;
    }
    match (inhibited, ongoing) {
        (true, true) => activate(world, registry, effect),
        (false, false) => {
            deactivate(world, effect);
            return;
        }
        (true, false) => return,
        (false, true) => {}
    }
    let Some(mut active) = world.get_mut::<ActiveEffect>(effect) else {
        return;
    };
    let mut executions = 0;
    if let Some(every) = active.period {
        active.until_period -= seconds;
        while active.until_period <= 0.0 {
            executions += 1;
            active.until_period += every;
        }
    }
    let expired = match active.remaining {
        Some(remaining) => {
            active.remaining = Some(remaining - seconds);
            remaining - seconds <= 0.0
        }
        None => false,
    };
    let (spec, def, stacks, source_tags) = (
        active.spec.clone(),
        active.def.clone(),
        active.stacks,
        active.source_tags.clone(),
    );
    for _ in 0..executions {
        execute(world, registry, target, &spec, &def, stacks, &source_tags);
    }
    if expired {
        expire(world, registry, effect, &def);
    }
}

/// `effect` has run out of time: as its stacking says.
fn expire(world: &mut World, registry: &EffectRegistry, effect: Entity, def: &Arc<Effect>) {
    let policy = def.stacking.as_ref().map(|stacking| stacking.expire);
    let Some(mut active) = world.get_mut::<ActiveEffect>(effect) else {
        return;
    };
    match policy {
        Some(Expire::Refresh) => active.remaining = active.duration,
        Some(Expire::RemoveOne) if active.stacks > 1 => {
            active.stacks -= 1;
            active.remaining = active.duration;
            let stacks = active.stacks;
            let target = world.get::<EffectOf>(effect).map(|of| of.0);
            refresh(world, registry, effect);
            if let Some(target) = target {
                world.trigger(EffectStacked {
                    entity: target,
                    effect,
                    id: def.id.clone(),
                    stacks,
                });
            }
        }
        _ => remove(world, effect, true, 0),
    }
}

/// Gives every active effect its definition from `registry`, as it's
/// replaced: one whose effect is gone is taken off.
pub(crate) fn relink(world: &mut World, registry: &EffectRegistry, effect: Entity) {
    let Some(active) = world.get::<ActiveEffect>(effect) else {
        return;
    };
    let Some(def) = registry.get(&active.def.id).cloned() else {
        remove(world, effect, false, 0);
        return;
    };
    if Arc::ptr_eq(&def, &active.def) {
        return;
    }
    let counted = !active.inhibited;
    if counted {
        deactivate(world, effect);
    }
    if let Some(mut active) = world.get_mut::<ActiveEffect>(effect) {
        active.def = def;
    }
    if counted {
        activate(world, registry, effect);
    }
}

/// Takes `effect` off its target with nothing told and nothing chained:
/// a prediction undone, or replaced by the server's own.
pub(crate) fn remove_quietly(world: &mut World, effect: Entity) {
    let Some(active) = world.get::<ActiveEffect>(effect) else {
        return;
    };
    if !active.inhibited {
        deactivate(world, effect);
    }
    world.despawn(effect);
}

/// Takes the stack a prediction added off `effect`.
pub(crate) fn unstack(world: &mut World, registry: &EffectRegistry, effect: Entity) {
    let Some(mut active) = world.get_mut::<ActiveEffect>(effect) else {
        return;
    };
    if active.stacks > 1 {
        active.stacks -= 1;
        refresh(world, registry, effect);
    } else {
        remove_quietly(world, effect);
    }
}

/// Shows, on a client, the server's effect `entity` on `target` from its
/// replica: counting as it does there, but not ticked here. A predicted
/// effect it stands for is taken off.
pub(crate) fn show(
    world: &mut World,
    registry: &EffectRegistry,
    entity: Entity,
    target: Entity,
    replica: &EffectReplica,
) {
    let Some(def) = registry.get(&replica.spec.effect).cloned() else {
        warn!("there's no effect called `{}` to show", replica.spec.effect);
        return;
    };
    if let Some(activation) = replica.spec.activation {
        let predicted: Vec<Entity> = effects_on(world, target)
            .into_iter()
            .filter(|other| {
                *other != entity
                    && world.get::<net::PredictedUnder>(*other)
                        == Some(&net::PredictedUnder(activation))
                    && world
                        .get::<ActiveEffect>(*other)
                        .is_some_and(|active| active.def.id == def.id)
            })
            .collect();
        for other in predicted {
            remove_quietly(world, other);
        }
    }
    world.entity_mut(entity).insert((
        ActiveEffect {
            spec: replica.spec.clone(),
            def,
            stacks: replica.stacks,
            duration: replica.remaining,
            remaining: replica.remaining,
            period: None,
            until_period: 0.0,
            inhibited: true,
            granted: Tags::new(),
            source_tags: TagCounts::default(),
        },
        net::Proxy,
    ));
    if !replica.inhibited {
        activate(world, registry, entity);
    }
}

/// Keeps the shown effect `entity` as its replica now says.
pub(crate) fn update_shown(
    world: &mut World,
    registry: &EffectRegistry,
    entity: Entity,
    replica: &EffectReplica,
) {
    let Some(mut active) = world.get_mut::<ActiveEffect>(entity) else {
        return;
    };
    let restack = active.stacks != replica.stacks;
    let was_inhibited = active.inhibited;
    active.stacks = replica.stacks;
    active.remaining = replica.remaining;
    match (was_inhibited, replica.inhibited) {
        (true, false) => activate(world, registry, entity),
        (false, true) => deactivate(world, entity),
        _ if restack => refresh(world, registry, entity),
        _ => {}
    }
}
