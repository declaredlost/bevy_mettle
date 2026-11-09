//! What an effect works out: its magnitudes and changes, executing, and the
//! modifiers it carries.

use bevy::prelude::*;
use bevy_mettle_attribute::modifier::{Modifier, Operation};
use bevy_mettle_attribute::read::AttributeContext;
use bevy_mettle_core::tag::{OwnedTags, TagCounts};

use super::{Check, count, source_tags};
use crate::active::{ActiveEffect, EffectOf};
use crate::code::{BaseChange, EffectContext, Whose};
use crate::cue::Cue;
use crate::data::{Capture, Effect, Magnitude};
use crate::events::{Delta, EffectExecuted};
use crate::{EffectRegistry, EffectSpec, net};

/// How much `magnitude` is: as `captured`, if it was.
pub(super) fn magnitude(
    context: &EffectContext,
    registry: &EffectRegistry,
    magnitude: &Magnitude,
    captured: Option<f32>,
) -> f32 {
    captured.unwrap_or_else(|| resolve(context, registry, magnitude))
}

/// How much `magnitude` is, for `context`.
pub(super) fn resolve(
    context: &EffectContext,
    registry: &EffectRegistry,
    magnitude: &Magnitude,
) -> f32 {
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

/// What executing an effect works out: the changes it makes to bases, and
/// what each meta attribute was dealt before its router.
pub(super) struct Worked {
    pub(super) changes: Vec<BaseChange>,
    pub(super) routed: Vec<Delta>,
}

/// What executing `def` on `target` would change, without changing it: its
/// modifiers and executions, worked out, its attribute magnitudes as
/// `captured` if they were, and what it dealt meta attributes routed.
pub(super) fn changes(
    world: &World,
    registry: &EffectRegistry,
    target: Entity,
    spec: &EffectSpec,
    def: &Effect,
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
    let context = EffectContext {
        world,
        target,
        spec,
        stacks,
    };
    let mut changes = Vec::new();
    for (index, modifier) in def.modifiers.iter().enumerate() {
        let weight = modifier
            .condition
            .as_ref()
            .map_or(1.0, |condition| condition.weight(&check));
        if weight <= 0.0 || (modifier.op.whole() && weight < 1.0) {
            continue;
        }
        let captured = captured.get(index).copied().flatten();
        let mut value = magnitude(&context, registry, &modifier.value, captured);
        if !modifier.op.whole() {
            value *= count(stacks) * weight;
        }
        if let Some(per) = &modifier.per {
            let of = context
                .value_at(target, &per.attribute, &AttributeContext::new())
                .unwrap_or_default();
            value *= per.times(of);
        }
        changes.push(BaseChange {
            attribute: modifier.attribute.clone(),
            op: modifier.op,
            value,
            on: Whose::Target,
        });
    }
    for execution in &def.executions {
        if let Some(run) = registry.execution(execution) {
            changes.extend(run(&context));
        }
    }
    route(&context, registry, changes)
}

/// Routes what `changes` deal meta attributes: each one's amount summed, then
/// handed to its router, its changes after the effect's own. Single-pass: what
/// a router deals a meta attribute is dropped.
fn route(context: &EffectContext, registry: &EffectRegistry, changes: Vec<BaseChange>) -> Worked {
    let mut kept = Vec::with_capacity(changes.len());
    let mut routed: Vec<Delta> = Vec::new();
    for change in changes {
        if registry.router(&change.attribute).is_none() {
            kept.push(change);
            continue;
        }
        if change.op != Operation::Flat || change.on != Whose::Target {
            warn!(
                "effect `{}` deals meta attribute `{}` other than flat to its target: dropped",
                context.spec.effect, change.attribute
            );
            continue;
        }
        match routed
            .iter_mut()
            .find(|dealt| dealt.attribute == change.attribute)
        {
            Some(dealt) => dealt.delta += change.value,
            None => routed.push(Delta {
                attribute: change.attribute,
                delta: change.value,
            }),
        }
    }
    for dealt in &routed {
        let Some(run) = registry.router(&dealt.attribute) else {
            continue;
        };
        for change in run(context, dealt.delta) {
            if registry.router(&change.attribute).is_some() {
                warn!(
                    "the router of `{}` deals meta attribute `{}`: dropped, as routing is \
                     single-pass",
                    dealt.attribute, change.attribute
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
    let def = registry.get(&spec.effect)?;
    let source_tags = source_tags(world, spec.source);
    Some(changes(world, registry, target, spec, def, 1, &source_tags, &[]).changes)
}

/// Executes `def` on `target`: its modifiers and executions, changing bases.
/// Nothing, on a client, under an activation the server answered: its own
/// changes arrive.
pub(super) fn execute(
    world: &mut World,
    registry: &EffectRegistry,
    target: Entity,
    spec: &EffectSpec,
    def: &Effect,
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
        let (entity, deltas) = match change.on {
            Whose::Target => (target, &mut dealt),
            Whose::Source => {
                let Some(source) = spec.source else {
                    warn!(
                        "effect `{}` changes its source's `{}`, but has no source: dropped",
                        def.id, change.attribute
                    );
                    continue;
                };
                (source, &mut stolen)
            }
        };
        let delta = net::change_base(world, predicted, entity, change);
        match deltas
            .iter_mut()
            .find(|dealt| dealt.attribute == change.attribute)
        {
            Some(dealt) => dealt.delta += delta,
            None => deltas.push(Delta {
                attribute: change.attribute.clone(),
                delta,
            }),
        }
    }
    world.trigger(EffectExecuted {
        entity: target,
        id: def.id.clone(),
        source: spec.source,
        changes: dealt.clone(),
        source_changes: stolen,
        routed: worked.routed,
    });
    for tag in def.cues.written() {
        let mut cue = Cue::new(tag.clone())
            .magnitude(dealt.first().map_or(0.0, |first| first.delta))
            .level(spec.level);
        cue.source = spec.source;
        cue.changes.clone_from(&dealt);
        crate::cue::execute(world, target, &cue, spec.activation, Some(def.id.clone()));
    }
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
        .enumerate()
        .map(|(index, modifier)| {
            let captured = active.captured.get(index).copied().flatten();
            let value = magnitude(&context, registry, &modifier.value, captured);
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
