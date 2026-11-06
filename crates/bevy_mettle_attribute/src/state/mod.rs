//! An actor's attributes, without their types: bases as last synced,
//! modifiers folded, and current values worked out.

mod fold;
mod sum;
mod systems;

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_mettle_core::Id;
use bevy_mettle_core::tag::{OwnedTags, TagCounts};

pub(crate) use self::fold::{Dirty, collect, fold};
pub use self::sum::Breakdown;
use self::sum::{Counts, Scoped, Sum};
pub(crate) use self::systems::{publish, refresh, sync_base};
use crate::context::AttributeContext;
use crate::{AnyAttribute, AttributeRegistry, Limit, Modifier};

/// One read's walk through the attributes it reads: those being worked
/// out, and those worked out already.
#[derive(Default)]
struct Walk {
    /// The attributes being worked out: one reading one of these again is
    /// a cycle, cut there.
    visiting: Vec<Id<AnyAttribute>>,
    /// Values worked out with no cycle cut beneath them: the same,
    /// whatever reads them, so worked out once.
    memo: HashMap<Id<AnyAttribute>, f32>,
    /// Whether a cycle was cut beneath the value being worked out.
    cut: bool,
}

impl Walk {
    /// A walk starting at `attribute`.
    fn from(attribute: &Id<AnyAttribute>) -> Self {
        Self {
            visiting: vec![attribute.clone()],
            ..Self::default()
        }
    }

    /// Whether reading `attribute` would be a cycle: if so, noted as cut.
    fn cuts(&mut self, attribute: &Id<AnyAttribute>) -> bool {
        let cycle = self.visiting.contains(attribute);
        self.cut |= cycle;
        cycle
    }
}

/// One attribute's modifiers, folded.
#[derive(Default)]
struct Folded {
    /// What always counts, summed.
    sum: Sum,
    /// What only sometimes counts.
    scoped: Vec<Scoped>,
}

/// On an actor: its attributes, whatever their types. Each one's base, a
/// cache of its [`Base`](crate::Base), which stays the truth; every
/// modifier that counts for it, folded; and each value with nothing in
/// play, as last worked out. What [`Current`](crate::Current) is written
/// from, and [`AttributeReader`](crate::AttributeReader) and
/// [`AttributeWorld`](crate::AttributeWorld) read: mettle's to keep, and
/// read only through them.
#[derive(Component, Default)]
#[require(OwnedTags)]
pub struct AttributeState {
    bases: HashMap<Id<AnyAttribute>, f32>,
    folded: HashMap<Id<AnyAttribute>, Folded>,
    current: HashMap<Id<AnyAttribute>, f32>,
    stale: bool,
}

impl AttributeState {
    /// Whether the actor has `attribute`: its `Base`.
    #[must_use]
    pub(crate) fn has(&self, attribute: &Id<AnyAttribute>) -> bool {
        self.bases.contains_key(attribute)
    }

    /// Sets `attribute`'s base: synced from its `Base`, or as an effect
    /// changes it.
    pub(crate) fn set_base(&mut self, attribute: Id<AnyAttribute>, value: f32) {
        self.bases.insert(attribute, value);
        self.stale = true;
    }

    /// Folds `modifiers`, each with its source. One naming what `registry`
    /// doesn't have, or whose numbers can't be, is skipped, with a
    /// warning.
    pub(crate) fn fold<'a>(
        &mut self,
        registry: &AttributeRegistry,
        modifiers: impl IntoIterator<Item = (Entity, &'a Modifier)>,
    ) {
        self.folded.clear();
        self.stale = true;
        for (order, (source, modifier)) in (0u32..).zip(modifiers) {
            if let Err(error) = modifier.check_values(registry) {
                warn!("a modifier from {source} is skipped: {error}");
                continue;
            }
            let folded = self.folded.entry(modifier.attribute.clone()).or_default();
            if modifier.always() {
                folded.sum.add(modifier.op, modifier.value, order);
            } else {
                folded.scoped.push(Scoped {
                    op: modifier.op,
                    value: modifier.value,
                    per: modifier
                        .per
                        .as_ref()
                        .map(|per| (per.attribute.clone(), per.every)),
                    counts: Counts::of(modifier, source),
                    order,
                });
            }
        }
    }

    /// `attribute`'s value in `context`, with the owner owning `owned`;
    /// with nothing in play and nothing changed since it was last worked
    /// out, as it was then.
    #[must_use]
    pub(crate) fn value(
        &self,
        registry: &AttributeRegistry,
        owned: &OwnedTags,
        attribute: &Id<AnyAttribute>,
        context: &AttributeContext,
    ) -> f32 {
        if context.is_empty()
            && !self.stale
            && !owned.is_pending()
            && let Some(value) = self.current.get(attribute)
        {
            return *value;
        }
        self.eval(
            registry,
            owned.counts(),
            attribute,
            context,
            &mut Walk::default(),
        )
    }

    /// `attribute`'s modifiers in `context`, summed but not applied.
    #[must_use]
    pub(crate) fn breakdown(
        &self,
        registry: &AttributeRegistry,
        owned: &OwnedTags,
        attribute: &Id<AnyAttribute>,
        context: &AttributeContext,
    ) -> Breakdown {
        self.sum(
            registry,
            owned.counts(),
            attribute,
            context,
            &mut Walk::from(attribute),
        )
    }

    /// Works out every value the actor has with nothing in play: whether
    /// any changed.
    pub(crate) fn refresh(&mut self, registry: &AttributeRegistry, owned: &TagCounts) -> bool {
        let context = AttributeContext::new();
        let mut walk = Walk::default();
        let current: HashMap<Id<AnyAttribute>, f32> = self
            .bases
            .keys()
            .map(|attribute| {
                let value = self.eval(registry, owned, attribute, &context, &mut walk);
                (attribute.clone(), value)
            })
            .collect();
        let changed = current != self.current;
        self.current = current;
        self.stale = false;
        changed
    }

    /// `attribute`'s value with nothing in play, as last worked out.
    pub(crate) fn current(&self, attribute: &Id<AnyAttribute>) -> Option<f32> {
        self.current.get(attribute).copied()
    }

    /// `value` rounded as `attribute` is, and kept within its limits, read
    /// with nothing in play.
    pub(crate) fn within_limits(
        &self,
        registry: &AttributeRegistry,
        owned: &TagCounts,
        attribute: &Id<AnyAttribute>,
        value: f32,
    ) -> f32 {
        let context = AttributeContext::new();
        self.kept(
            registry,
            owned,
            attribute,
            value,
            &context,
            &mut Walk::from(attribute),
        )
    }

    /// `value` rounded as `attribute` is, then kept within its limits: the
    /// limits win over the rounding.
    fn kept(
        &self,
        registry: &AttributeRegistry,
        owned: &TagCounts,
        attribute: &Id<AnyAttribute>,
        value: f32,
        context: &AttributeContext,
        walk: &mut Walk,
    ) -> f32 {
        let Some(info) = registry.get(attribute) else {
            return value;
        };
        let mut value = info
            .rounding
            .map_or(value, |rounding| rounding.round(value));
        if let Some(min) = self.limit(registry, owned, info.min.as_ref(), context, walk) {
            value = value.max(min);
        }
        if let Some(max) = self.limit(registry, owned, info.max.as_ref(), context, walk) {
            value = value.min(max);
        }
        value
    }

    /// `attribute`'s base: as synced, or where it starts.
    fn base(&self, registry: &AttributeRegistry, attribute: &Id<AnyAttribute>) -> f32 {
        self.bases
            .get(attribute)
            .copied()
            .unwrap_or_else(|| registry.get(attribute).map_or(0.0, |info| info.base))
    }

    /// `attribute`'s modifiers in `context`, summed over its base: a `per`
    /// one of the attributes being worked out doesn't count.
    fn sum(
        &self,
        registry: &AttributeRegistry,
        owned: &TagCounts,
        attribute: &Id<AnyAttribute>,
        context: &AttributeContext,
        walk: &mut Walk,
    ) -> Breakdown {
        let base = self.base(registry, attribute);
        let Some(folded) = self.folded.get(attribute) else {
            return Sum::default().over(base);
        };
        let mut sum = folded.sum;
        for scoped in &folded.scoped {
            let weight = scoped.counts.weight(context, owned);
            if weight <= 0.0 {
                continue;
            }
            let times = match &scoped.per {
                None => 1.0,
                Some((per, every)) => {
                    if walk.cuts(per) {
                        continue;
                    }
                    let of = self.eval(registry, owned, per, context, walk);
                    (of / every).floor().max(0.0)
                }
            };
            sum.add_weighted(scoped.op, scoped.value * times, weight, scoped.order);
        }
        sum.over(base)
    }

    /// `limit`'s value: a value, or the other attribute's, read in the same
    /// context, if it isn't being worked out.
    fn limit(
        &self,
        registry: &AttributeRegistry,
        owned: &TagCounts,
        limit: Option<&Limit>,
        context: &AttributeContext,
        walk: &mut Walk,
    ) -> Option<f32> {
        match limit? {
            Limit::Value(value) => Some(*value),
            Limit::Attribute(other) => {
                if walk.cuts(other) {
                    None
                } else {
                    Some(self.eval(registry, owned, other, context, walk))
                }
            }
        }
    }

    /// `attribute`'s value: worked out, or as `walk` worked it out already.
    fn eval(
        &self,
        registry: &AttributeRegistry,
        owned: &TagCounts,
        attribute: &Id<AnyAttribute>,
        context: &AttributeContext,
        walk: &mut Walk,
    ) -> f32 {
        if let Some(value) = walk.memo.get(attribute) {
            return *value;
        }
        let outer = std::mem::replace(&mut walk.cut, false);
        walk.visiting.push(attribute.clone());
        let total = self.sum(registry, owned, attribute, context, walk).total();
        let value = self.kept(registry, owned, attribute, total, context, walk);
        walk.visiting.pop();
        if !walk.cut {
            walk.memo.insert(attribute.clone(), value);
        }
        walk.cut |= outer;
        value
    }
}
