//! An actor's attributes, without their types: bases as last synced, modifiers
//! folded, and current values worked out.

mod fold;
mod slots;
mod sum;
mod systems;

use std::sync::OnceLock;

use bevy::prelude::*;
use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::tag::{OwnedTags, TagCounts};

pub(crate) use self::fold::{Dirty, collect, fold};
pub(crate) use self::slots::Slots;
pub use self::sum::Breakdown;
pub(crate) use self::sum::divisor;
use self::sum::{Counts, Scoped, Sum};
pub(crate) use self::systems::{follow_limits, publish, refresh, remove_base, rework, sync_base};
use crate::modifier::Modifier;
use crate::read::AttributeContext;
use crate::registry::{AttributeId, AttributeRegistry, Bound};

/// How many whole `every` there are in `of`: none for less than one, floored a
/// little above, against float error.
pub(crate) fn times_per(of: f32, every: f32) -> f32 {
    (of / every + 1e-4).floor().max(0.0)
}

/// One read's walk through the attributes it reads: those being worked out, and
/// those worked out already.
#[derive(Default)]
struct Walk {
    /// The attributes being worked out: one reading one of these again is a
    /// cycle, cut there.
    visiting: Vec<AttributeId>,
    /// Values worked out with no cycle cut beneath them: the same, whatever
    /// reads them, so worked out once.
    memo: Slots<f32>,
    /// Whether a cycle was cut beneath the value being worked out.
    cut: bool,
}

impl Walk {
    /// A walk starting at `attribute`.
    fn from(attribute: AttributeId) -> Self {
        Self {
            visiting: vec![attribute],
            ..Self::default()
        }
    }

    /// Whether reading `attribute` would be a cycle: if so, noted as cut.
    fn cuts(&mut self, attribute: AttributeId) -> bool {
        let cycle = self.visiting.contains(&attribute);
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

/// On an actor: its attributes, whatever their types, as mettle keeps them.
///
/// Each one's base, a cache of its [`Base`](crate::Base), which stays the
/// truth; every modifier that counts for it, folded; and each value with
/// nothing in play, as last worked out. What [`Current`](crate::Current) is
/// written from, and [`AttributeReader`](crate::read::AttributeReader) and
/// [`AttributeWorld`](crate::read::AttributeWorld) read: mettle's to keep, and
/// read only through them.
///
/// On a [`Remote`](crate::authority::Remote) actor, each value with nothing
/// in play is the one served, as [`Served`](crate::authority::Served) says,
/// unless a [`Provisional`](crate::modifier::Provisional) source counts for
/// it: then it's its own, the prediction.
///
/// Reflected for inspectors: its bases and values, not what's folded.
#[derive(Component, Default, Reflect)]
#[reflect(Component, Default)]
#[require(OwnedTags)]
pub struct AttributeState {
    bases: Slots<f32>,
    #[reflect(ignore)]
    folded: Slots<Folded>,
    current: Slots<f32>,
    /// Each value with nothing in play as worked out the time before: what a
    /// limit moved from.
    #[reflect(ignore)]
    previous: Slots<f32>,
    /// The values served by the authority, as synced from `Served`.
    served: Slots<f32>,
    /// The attributes whose values can differ by read: a modifier of one, or
    /// of what it's worked out through (what a modifier's per, a limit
    /// that's another attribute), counts by the read's context. Any other is
    /// read as cached, whatever the context. Worked out the first time a read
    /// in context needs it, and again after modifiers are folded: a frame
    /// with no such read works out nothing.
    #[reflect(ignore)]
    contextual: OnceLock<Slots<bool>>,
    /// Whether the authority is another world: its values are served.
    remote: bool,
    /// Whether a prediction counts for it: its values are its own.
    provisional: bool,
    /// What changed since values were last worked out.
    #[reflect(ignore)]
    stale: Stale,
}

/// What changed of an actor's attributes since its values were last worked
/// out: what's worked out again.
#[derive(Debug, Clone, Default, PartialEq)]
enum Stale {
    /// Nothing: its values are as worked out.
    #[default]
    Fresh,
    /// These attributes: each worked out again, with what's worked out
    /// through it.
    These(Vec<AttributeId>),
    /// Everything: every value worked out again.
    All,
}

impl AttributeState {
    /// Whether the actor has `attribute`: its `Base`.
    #[must_use]
    pub(crate) fn has(&self, attribute: AttributeId) -> bool {
        self.bases.contains(attribute)
    }

    /// Sets `attribute`'s value as served: synced from its `Served`.
    pub(crate) fn set_served(&mut self, attribute: AttributeId, value: f32) {
        self.served.insert(attribute, value);
        self.mark(attribute);
    }

    /// Sets whether the actor's authority is another world.
    pub(crate) fn set_remote(&mut self, remote: bool) {
        self.remote = remote;
        self.mark_all();
    }

    /// Notes `attribute` changed: worked out again, with what's worked out
    /// through it, as values are next.
    fn mark(&mut self, attribute: AttributeId) {
        match &mut self.stale {
            Stale::All => {}
            Stale::These(changed) => {
                if !changed.contains(&attribute) {
                    changed.push(attribute);
                }
            }
            Stale::Fresh => self.stale = Stale::These(vec![attribute]),
        }
    }

    /// Notes everything changed: every value worked out again.
    fn mark_all(&mut self) {
        self.stale = Stale::All;
    }

    /// `attribute`'s value with nothing in play as served, if it's shown so:
    /// the actor's remote, and no prediction counts for it.
    fn shown_served(&self, attribute: AttributeId) -> Option<f32> {
        (self.remote && !self.provisional)
            .then(|| self.served.get(attribute).copied())
            .flatten()
    }

    /// `attribute`'s base as cached, if it's there.
    pub(crate) fn cached_base(&self, attribute: AttributeId) -> Option<f32> {
        self.bases.get(attribute).copied()
    }

    /// Sets `attribute`'s base: synced from its `Base`, or as an effect changes
    /// it.
    pub(crate) fn set_base(&mut self, attribute: AttributeId, value: f32) {
        self.bases.insert(attribute, value);
        self.mark(attribute);
    }

    /// Forgets `attribute`: its `Base` was taken off.
    pub(crate) fn remove(&mut self, attribute: AttributeId) {
        self.bases.remove(attribute);
        self.current.remove(attribute);
        self.mark_all();
    }

    /// Folds `modifiers`, each with its source, in the order they were
    /// applied. One naming what `registry`
    /// doesn't have, or whose numbers can't be, is skipped, with a warning.
    /// `provisional` if any of them is a prediction.
    pub(crate) fn fold<'a>(
        &mut self,
        registry: &AttributeRegistry,
        modifiers: impl IntoIterator<Item = (Entity, &'a Modifier)>,
        provisional: bool,
    ) {
        self.folded.clear();
        self.provisional = provisional;
        self.mark_all();
        for (order, (source, modifier)) in (0u32..).zip(modifiers) {
            if let Err(error) = modifier.check_values(registry) {
                warn!("a modifier from {source} is skipped: {error}");
                continue;
            }
            let folded = self.folded.or_default(modifier.attribute);
            if modifier.always() {
                folded.sum.add(modifier.op, modifier.value, order);
            } else {
                folded.scoped.push(Scoped {
                    op: modifier.op,
                    value: modifier.value,
                    per: modifier.per.as_ref().map(|per| (per.attribute, per.every)),
                    counts: Counts::of(modifier, source),
                    order,
                });
            }
        }
        self.contextual = OnceLock::new();
    }

    /// Which attributes' values can differ by read: worked out the first time
    /// it's asked after modifiers are folded.
    fn contextual(&self, registry: &AttributeRegistry) -> &Slots<bool> {
        self.contextual.get_or_init(|| {
            let mut contextual = Slots::default();
            // With nothing that counts only sometimes, nothing counts by a
            // read.
            if self
                .folded
                .iter()
                .all(|(_, folded)| folded.scoped.is_empty())
            {
                return contextual;
            }
            let mut visiting = Vec::new();
            for (attribute, _) in registry.iter() {
                reads_context(
                    &self.folded,
                    registry,
                    attribute,
                    &mut contextual,
                    &mut visiting,
                );
            }
            contextual
        })
    }

    /// `attribute`'s value in `context`, with the owner owning `owned`; with
    /// nothing in play, as served if it's shown so, or, with nothing changed
    /// since it was last worked out, as it was then.
    #[must_use]
    pub(crate) fn value(
        &self,
        registry: &AttributeRegistry,
        owned: &OwnedTags,
        attribute: AttributeId,
        context: &AttributeContext,
    ) -> f32 {
        // A context nothing of it counts by reads as no context: as cached.
        let plain = context.is_empty() || self.contextual(registry).get(attribute) != Some(&true);
        if plain && let Some(value) = self.shown_served(attribute) {
            return value;
        }
        if plain
            && self.stale == Stale::Fresh
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
        attribute: AttributeId,
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

    /// Works out every value the actor has with nothing in play, or takes it
    /// as served if it's shown so: whether any changed.
    pub(crate) fn refresh(
        &mut self,
        registry: &AttributeRegistry,
        owned: &TagCounts,
        owned_changed: bool,
    ) -> bool {
        // What a condition counts by, the owner's tags, changed: what has a
        // modifier that counts only sometimes is worked out again.
        if owned_changed && self.stale != Stale::All {
            let scoped: Vec<AttributeId> = self
                .folded
                .iter()
                .filter(|(_, folded)| !folded.scoped.is_empty())
                .map(|(attribute, _)| attribute)
                .collect();
            for attribute in scoped {
                self.mark(attribute);
            }
        }
        let context = AttributeContext::new();
        let affected = match &self.stale {
            Stale::These(_) => Some(self.through(registry)),
            Stale::Fresh | Stale::All => None,
        };
        let mut walk = self.walk_knowing(None, affected.as_deref());
        let work = |state: &Self, attribute: AttributeId, walk: &mut Walk| {
            state
                .shown_served(attribute)
                .unwrap_or_else(|| state.eval(registry, owned, attribute, &context, walk))
        };
        let current: Slots<f32> = if self.stale == Stale::All {
            self.bases
                .keys()
                .map(|attribute| (attribute, work(self, attribute, &mut walk)))
                .collect()
        } else {
            let mut current = self.current.clone();
            for attribute in affected.unwrap_or_default() {
                if self.has(attribute) {
                    current.insert(attribute, work(self, attribute, &mut walk));
                }
            }
            current
        };
        let changed = current != self.current;
        self.previous = std::mem::replace(&mut self.current, current);
        self.stale = Stale::Fresh;
        changed
    }

    /// A walk with nothing in play, from `start` if it's given, that knows
    /// each value as last worked out but those `affected` (all of them, if
    /// it's `None`): what didn't change isn't worked out again. Not on an
    /// actor shown as served: what it last worked out is what it was served.
    fn walk_knowing(&self, start: Option<AttributeId>, affected: Option<&[AttributeId]>) -> Walk {
        let mut walk = start.map_or_else(Walk::default, Walk::from);
        let Some(affected) = affected else {
            return walk;
        };
        if self.remote && !self.provisional {
            return walk;
        }
        for (attribute, value) in self.current.iter() {
            if Some(attribute) != start && !affected.contains(&attribute) {
                walk.memo.insert(attribute, *value);
            }
        }
        walk
    }

    /// What keeping `attribute` within its limits reads: each limit that's
    /// another attribute, and what that's worked out through, in turn.
    pub(crate) fn limits_read(
        &self,
        registry: &AttributeRegistry,
        attribute: AttributeId,
    ) -> Vec<AttributeId> {
        let mut read: Vec<AttributeId> = Vec::new();
        let mut next = 0;
        let add = |read: &mut Vec<AttributeId>, other: AttributeId| {
            if other != attribute && !read.contains(&other) {
                read.push(other);
            }
        };
        let limits = |of: AttributeId| {
            registry.get(of).into_iter().flat_map(|info| {
                [info.min.as_ref(), info.max.as_ref()]
                    .into_iter()
                    .flatten()
                    .filter_map(|bound| match bound {
                        Bound::Attribute(other) => Some(*other),
                        Bound::Value(_) => None,
                    })
            })
        };
        for other in limits(attribute) {
            add(&mut read, other);
        }
        while let Some(&of) = read.get(next) {
            next += 1;
            let per = self
                .folded
                .get(of)
                .into_iter()
                .flat_map(|folded| folded.scoped.iter().filter_map(|scoped| scoped.per))
                .map(|(per, _)| per);
            for other in limits(of).chain(per).collect::<Vec<_>>() {
                add(&mut read, other);
            }
        }
        read
    }

    /// What's changed, and every attribute worked out through it: what a
    /// limit is, what a modifier's per.
    fn through(&self, registry: &AttributeRegistry) -> Vec<AttributeId> {
        let mut affected = match &self.stale {
            Stale::These(changed) => changed.clone(),
            Stale::Fresh | Stale::All => Vec::new(),
        };
        let mut next = 0;
        while let Some(&changed) = affected.get(next) {
            next += 1;
            let limited = registry.iter().filter_map(|(attribute, info)| {
                [info.min.as_ref(), info.max.as_ref()]
                    .into_iter()
                    .flatten()
                    .any(|bound| matches!(bound, Bound::Attribute(other) if *other == changed))
                    .then_some(attribute)
            });
            let per = self.folded.iter().filter_map(|(attribute, folded)| {
                folded
                    .scoped
                    .iter()
                    .any(|scoped| scoped.per.is_some_and(|(per, _)| per == changed))
                    .then_some(attribute)
            });
            for attribute in limited.chain(per).collect::<Vec<_>>() {
                if !affected.contains(&attribute) {
                    affected.push(attribute);
                }
            }
        }
        affected
    }

    /// What `attribute`'s value with nothing in play moved from and to, the
    /// last time values were worked out, if it moved.
    pub(crate) fn moved(&self, attribute: AttributeId) -> Option<(f32, f32)> {
        let was = *self.previous.get(attribute)?;
        let is = *self.current.get(attribute)?;
        (was.to_bits() != is.to_bits()).then_some((was, is))
    }

    /// Whether the actor's authority is another world: its bases aren't its
    /// own to move.
    pub(crate) fn is_remote(&self) -> bool {
        self.remote
    }

    /// Whether something changed since values were last worked out.
    pub(crate) fn is_stale(&self) -> bool {
        self.stale != Stale::Fresh
    }

    /// `attribute`'s value with nothing in play, as last worked out.
    pub(crate) fn current(&self, attribute: AttributeId) -> Option<f32> {
        self.current.get(attribute).copied()
    }

    /// `value` rounded as `attribute` is, and kept within its limits, read with
    /// nothing in play.
    pub(crate) fn within_limits(
        &self,
        registry: &AttributeRegistry,
        owned: &TagCounts,
        attribute: AttributeId,
        value: f32,
    ) -> f32 {
        let context = AttributeContext::new();
        let affected = match &self.stale {
            Stale::These(_) => Some(self.through(registry)),
            Stale::Fresh => Some(Vec::new()),
            Stale::All => None,
        };
        let mut walk = self.walk_knowing(Some(attribute), affected.as_deref());
        self.kept(registry, owned, attribute, value, &context, &mut walk)
    }

    /// `value` rounded as `attribute` is, then kept within its limits: the
    /// limits win over the rounding.
    fn kept(
        &self,
        registry: &AttributeRegistry,
        owned: &TagCounts,
        attribute: AttributeId,
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
    fn base(&self, registry: &AttributeRegistry, attribute: AttributeId) -> f32 {
        self.bases
            .get(attribute)
            .copied()
            .unwrap_or_else(|| registry.get(attribute).map_or(0.0, |info| info.base))
    }

    /// `attribute`'s modifiers in `context`, summed over its base: a `per` one
    /// of the attributes being worked out doesn't count.
    fn sum(
        &self,
        registry: &AttributeRegistry,
        owned: &TagCounts,
        attribute: AttributeId,
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
                    if walk.cuts(*per) {
                        continue;
                    }
                    let of = self.eval(registry, owned, *per, context, walk);
                    times_per(of, *every)
                }
            };
            sum.add_weighted(scoped.op, scoped.value, weight, times, scoped.order);
        }
        sum.over(base)
    }

    /// `limit`'s value: a value, or the other attribute's, read in the same
    /// context, if it isn't being worked out.
    fn limit(
        &self,
        registry: &AttributeRegistry,
        owned: &TagCounts,
        limit: Option<&Bound>,
        context: &AttributeContext,
        walk: &mut Walk,
    ) -> Option<f32> {
        match limit? {
            Bound::Value(value) => Some(*value),
            Bound::Attribute(other) => {
                if walk.cuts(*other) {
                    None
                } else {
                    Some(self.eval(registry, owned, *other, context, walk))
                }
            }
        }
    }

    /// `attribute`'s value: worked out, or as `walk` worked it out already.
    fn eval(
        &self,
        registry: &AttributeRegistry,
        owned: &TagCounts,
        attribute: AttributeId,
        context: &AttributeContext,
        walk: &mut Walk,
    ) -> f32 {
        if let Some(value) = walk.memo.get(attribute) {
            return *value;
        }
        let outer = std::mem::replace(&mut walk.cut, false);
        walk.visiting.push(attribute);
        let total = self.sum(registry, owned, attribute, context, walk).total();
        let value = self.kept(registry, owned, attribute, total, context, walk);
        walk.visiting.pop();
        if !walk.cut {
            walk.memo.insert(attribute, value);
        }
        walk.cut |= outer;
        value
    }
}

/// Whether `attribute`'s value can differ by read: a modifier of it counts by
/// the read's context (its tags, the target's, the source's, a fact; or it's
/// local to a source), or what it's worked out through does. Each answer is
/// kept in `known`; `visiting` holds what's being worked out, so a cycle ends.
fn reads_context(
    folded: &Slots<Folded>,
    registry: &AttributeRegistry,
    attribute: AttributeId,
    known: &mut Slots<bool>,
    visiting: &mut Vec<AttributeId>,
) -> bool {
    if let Some(&reads) = known.get(attribute) {
        return reads;
    }
    if visiting.contains(&attribute) {
        return false;
    }
    visiting.push(attribute);
    let mut reads = false;
    if let Some(folded_here) = folded.get(attribute) {
        for scoped in &folded_here.scoped {
            reads = scoped.counts.local
                || scoped
                    .counts
                    .condition
                    .as_ref()
                    .is_some_and(Condition::reads_context)
                || scoped
                    .per
                    .is_some_and(|(per, _)| reads_context(folded, registry, per, known, visiting));
            if reads {
                break;
            }
        }
    }
    if !reads && let Some(info) = registry.get(attribute) {
        reads = [info.min.as_ref(), info.max.as_ref()]
            .into_iter()
            .flatten()
            .any(|bound| match bound {
                Bound::Attribute(other) => reads_context(folded, registry, *other, known, visiting),
                Bound::Value(_) => false,
            });
    }
    visiting.pop();
    known.insert(attribute, reads);
    reads
}
