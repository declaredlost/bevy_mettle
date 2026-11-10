//! What an effect works out: its magnitudes and changes, executing, and the
//! modifiers it carries.

use bevy::prelude::*;
use bevy_mettle_attribute::modifier::{Modifier, Operation};
use bevy_mettle_attribute::read::{AttributeContext, AttributeWorld};
use bevy_mettle_attribute::registry::AttributeRegistry;
use bevy_mettle_core::registry::AttributeId;
use bevy_mettle_core::tag::{OwnedTags, TagCounts};

use super::{Check, count, source_tags};
use crate::active::{ActiveEffect, EffectOf};
use crate::code::{BaseChange, Changes, ExecutionContext, RouteId, Whose};
use crate::cue::Cue;
use crate::data::{Capture, CaptureDef, EffectDef, MagnitudeDef, Read};
use crate::events::{Delta, EffectExecuted, Routed};
use crate::{EffectRegistry, EffectSpec, net};

/// How much `magnitude` is, for `context`: its attributes as `context`
/// captured them, or read now.
pub(super) fn resolve(
    context: &ExecutionContext,
    registry: &EffectRegistry,
    magnitude: &MagnitudeDef,
) -> f32 {
    match magnitude {
        MagnitudeDef::Value(value) => *value,
        MagnitudeDef::Level => context.spec.level,
        MagnitudeDef::Param(param) => context.spec.param_value(*param).unwrap_or_else(|| {
            warn!(
                "effect `{}` reads {param:?}, which its spec doesn't set: 0",
                context.def.name
            );
            0.0
        }),
        MagnitudeDef::Attribute(slot) => captured(context, *slot),
        MagnitudeDef::Custom(calculation, reads) => registry
            .calculation(*calculation)
            .map_or(0.0, |calculation| {
                calculation.calculate(&ExecutionContext { reads, ..*context })
            }),
        MagnitudeDef::Curve { curve, at } => registry
            .curves()
            .at(*curve, resolve(context, registry, at))
            .unwrap_or_default(),
        MagnitudeDef::Sum(terms) => terms
            .iter()
            .map(|term| resolve(context, registry, term))
            .sum(),
        MagnitudeDef::Product(terms) => terms
            .iter()
            .map(|term| resolve(context, registry, term))
            .product(),
        MagnitudeDef::Clamp { value, min, max } => {
            crate::registry::clamp(resolve(context, registry, value), *min, *max)
        }
    }
}

/// The value in `context`'s capture `slot`: as captured, or read now if it's
/// read live or wasn't taken.
pub(crate) fn captured(context: &ExecutionContext, slot: usize) -> f32 {
    if let Some(value) = context.captured.get(slot).copied().flatten() {
        return value;
    }
    context
        .def
        .captures
        .get(slot)
        .and_then(|capture| read(context, capture))
        .unwrap_or_default()
}

/// What `capture` reads now, if whose it is has it.
pub(super) fn read(context: &ExecutionContext, capture: &CaptureDef) -> Option<f32> {
    let source = context.spec.context.source;
    let whose = match capture.from {
        Capture::Source => source?,
        Capture::Target => context.target,
    };
    let world = context.world;
    let mut hit = AttributeContext::new().tags(&capture.read_in);
    if let Some(owned) = world.get::<OwnedTags>(context.target) {
        hit = hit.target_tags(owned);
    }
    if let Some(source) = source {
        hit = hit.source(source);
        if let Some(owned) = world.get::<OwnedTags>(source) {
            hit = hit.source_tags(owned);
        }
    }
    let current = || context.value_at(whose, capture.attribute, &hit);
    let base = || context.world.base_at(whose, capture.attribute);
    match capture.value {
        Read::Current => current(),
        Read::Base => base(),
        Read::Bonus => Some(current()? - base()?),
    }
}

/// What executing an effect works out: the changes it makes to bases, and
/// what each route was dealt before its router.
pub(super) struct Worked {
    pub(super) changes: Vec<BaseChange>,
    pub(super) routed: Vec<Routed>,
}

/// What executing `def` on `target` would change, without changing it: its
/// modifiers and executions, worked out, its attribute magnitudes as
/// `captured` if they were, and what it dealt routes routed.
pub(super) fn changes(
    world: &World,
    registry: &EffectRegistry,
    target: Entity,
    spec: &EffectSpec,
    def: &EffectDef,
    stacks: u32,
    source_tags: &TagCounts,
    captured: &[Option<f32>],
) -> Worked {
    let Some(owner) = world.get::<OwnedTags>(target) else {
        return Worked {
            changes: Vec::new(),
            routed: Vec::new(),
        };
    };
    let check = Check {
        tags: &def.tags,
        owner: owner.counts(),
        source: source_tags,
    };
    let context = ExecutionContext {
        world,
        target,
        def,
        spec,
        stacks,
        captured,
        reads: &[],
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
                .value_at(target, per.attribute, &AttributeContext::new())
                .unwrap_or_default();
            value *= per.times(of);
        }
        changes.push(BaseChange {
            changes: modifier.changes,
            op: modifier.op,
            value,
            on: Whose::Target,
        });
    }
    for (execution, reads) in &def.executions {
        if let Some(execution) = registry.execution(*execution) {
            let running = ExecutionContext { reads, ..context };
            changes.extend(execution.execute(&running));
        }
    }
    route(&context, registry, changes)
}

/// Routes what `changes` deal routes: each one's amount summed, then handed to
/// its router, its changes after the effect's own. Single-pass: what a router
/// deals a route is dropped.
fn route(
    context: &ExecutionContext,
    registry: &EffectRegistry,
    changes: Vec<BaseChange>,
) -> Worked {
    let registered = registry.routes();
    let name = |route: RouteId| registered.name(route).unwrap_or("?");
    let mut kept = Vec::with_capacity(changes.len());
    let mut routed: Vec<Routed> = Vec::new();
    for change in changes {
        let Changes::Route(route) = change.changes else {
            kept.push(change);
            continue;
        };
        if change.op != Operation::Flat || change.on != Whose::Target {
            warn!(
                "effect `{}` deals route `{}` other than flat to its target: dropped",
                context.def.name,
                name(route)
            );
            continue;
        }
        match routed.iter_mut().find(|dealt| dealt.route == route) {
            Some(dealt) => dealt.amount += change.value,
            None => routed.push(Routed {
                route,
                amount: change.value,
            }),
        }
    }
    for dealt in &routed {
        let Some(run) = registered.get(dealt.route) else {
            continue;
        };
        for change in run.route(context, dealt.amount) {
            if let Changes::Route(other) = change.changes {
                warn!(
                    "the router of `{}` deals route `{}`: dropped, as routing is single-pass",
                    name(dealt.route),
                    name(other)
                );
                continue;
            }
            kept.push(change);
        }
    }
    Worked {
        changes: kept,
        routed,
    }
}

/// What applying `spec` to `target` as an instant effect would change, if
/// there's such an effect: what an ability's cost is checked by.
pub(crate) fn preview(world: &World, target: Entity, spec: &EffectSpec) -> Option<Vec<BaseChange>> {
    let registry = world.get_resource::<EffectRegistry>()?;
    let def = registry.get(spec.effect_id(registry)?)?;
    let source_tags = source_tags(world, spec.context.source);
    let captured = super::apply::capture(world, target, spec, def);
    Some(
        changes(
            world,
            registry,
            target,
            spec,
            def,
            1,
            &source_tags,
            &captured,
        )
        .changes,
    )
}

/// Executes `def` on `target`: its modifiers and executions, changing bases.
/// Nothing, on a client, under an activation the server answered: its own
/// changes arrive.
pub(super) fn execute(
    world: &mut World,
    registry: &EffectRegistry,
    target: Entity,
    spec: &EffectSpec,
    def: &EffectDef,
    stacks: u32,
    source_tags: &TagCounts,
    captured: &[Option<f32>],
) {
    if net::answered(world, spec) {
        return;
    }
    let worked = changes(
        world,
        registry,
        target,
        spec,
        def,
        stacks,
        source_tags,
        captured,
    );
    let predicted = net::predicting(world, spec);
    // What each attribute's base was changed by, on each side, in the order
    // the effect first changes it.
    let mut dealt: Vec<Delta> = Vec::new();
    let mut stolen: Vec<Delta> = Vec::new();
    for change in &worked.changes {
        // Routes are routed by now: what's left changes attributes.
        let Changes::Attribute(attribute) = change.changes else {
            continue;
        };
        let (entity, deltas) = match change.on {
            Whose::Target => (target, &mut dealt),
            Whose::Source => {
                let Some(source) = spec.context.source else {
                    warn!(
                        "effect `{}` changes its source's `{}`, but has no source: dropped",
                        def.name,
                        attribute_name(world, attribute)
                    );
                    continue;
                };
                (source, &mut stolen)
            }
        };
        let delta = net::change_base(world, predicted, entity, attribute, change.op, change.value);
        match deltas.iter_mut().find(|dealt| dealt.attribute == attribute) {
            Some(dealt) => dealt.delta += delta,
            None => deltas.push(Delta { attribute, delta }),
        }
    }
    world.trigger(EffectExecuted {
        entity: target,
        id: def.id,
        context: spec.context.clone(),
        changes: dealt.clone(),
        source_changes: stolen,
        routed: worked.routed,
    });
    for tag in def.cues.written() {
        let mut cue = Cue::new(tag)
            .magnitude(dealt.first().map_or(0.0, |first| first.delta))
            .level(spec.level);
        cue.context.clone_from(&spec.context);
        cue.changes.clone_from(&dealt);
        crate::cue::execute(world, target, &cue, spec.activation, Some(def.id));
    }
}

/// `attribute`'s name, for a message.
fn attribute_name(world: &World, attribute: AttributeId) -> &'static str {
    world
        .get_resource::<AttributeRegistry>()
        .and_then(|registry| registry.get(attribute))
        .map_or("?", |info| info.name)
}

/// `effect`'s modifiers, resolved: amounts counted once per stack, its
/// conditions' source terms settled as its source was.
pub(super) fn modifiers(world: &World, registry: &EffectRegistry, effect: Entity) -> Vec<Modifier> {
    let (Some(active), Some(of)) = (
        world.get::<ActiveEffect>(effect),
        world.get::<EffectOf>(effect),
    ) else {
        return Vec::new();
    };
    let context = ExecutionContext {
        world,
        target: of.0,
        def: &active.def,
        spec: &active.spec,
        stacks: active.stacks,
        captured: &active.captured,
        reads: &[],
    };
    active
        .def
        .modifiers
        .iter()
        .filter_map(|modifier| {
            // A route is only dealt as an effect executes: it counts for none.
            let Changes::Attribute(attribute) = modifier.changes else {
                return None;
            };
            let value = resolve(&context, registry, &modifier.value);
            Some(Modifier {
                attribute,
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
            })
        })
        .collect()
}
