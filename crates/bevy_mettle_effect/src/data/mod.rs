//! Effects as data: what one does, for how long, and how it stacks.

mod def;
mod magnitude;
mod stacking;

use bevy_mettle_attribute::Attribute;
use bevy_mettle_attribute::modifier::Operation;
use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::registry::{AnyAttribute, AnyEffect, Key};
use bevy_mettle_core::tag::{TagKey, Tags};
use serde::{Deserialize, Serialize};

pub use self::def::{CaptureDef, EffectDef, MagnitudeDef, ModifierDef};
pub use self::magnitude::{AttributeRead, Capture, Magnitude, Read};
pub use self::stacking::{Expire, Reapply, StackBy, Stacking};
use crate::code::{AnyExecution, AnyRoute, Route};

/// An effect, as data: what it changes, for how long, and what it grants.
///
/// Unreal's `GameplayEffect`: also what it needs, and how it stacks. Named by
/// a `Key<AnyEffect>`, its `id`. Code applies it by an
/// [`Effect`](crate::Effect) type naming it.
///
/// Written by a game's content, in whatever format it loads with serde, or
/// built in code:
///
/// ```
/// # use bevy_mettle_attribute::prelude::*;
/// # use bevy_mettle_core::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// # #[derive(Attribute)] #[attribute(base = 100.0)] struct Life;
/// let burning = EffectData::lasting("burning", 4.0)
///     .tag("effect.affliction.burning")
///     .period(0.5, true)
///     .modifier(EffectModifier::new::<Life>(
///         Operation::Flat,
///         Magnitude::param("param.damage"),
///     ))
///     .grant("state.burning")
///     .require(!Condition::owner_has("immune.fire"))
///     .ongoing(!Condition::owner_has("state.wet"));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectData {
    /// Its name.
    pub name: Key<AnyEffect>,
    /// What it is: what removal, immunity and requirements match on.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
    /// How long it lasts.
    pub duration: EffectDuration,
    /// That it executes every so often while it lasts, instead of its modifiers
    /// counting all along: damage over time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Period>,
    /// What it changes. An instant or periodic effect changes bases as it
    /// executes; a lasting one counts as modifiers while it lasts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<EffectModifier>,
    /// Formulas in code, run as it executes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub executions: Vec<Key<AnyExecution>>,
    /// Tags its target owns while it lasts.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub grants: Tags,
    /// Cues it plays: held on its target while it counts, and played once each
    /// time it executes.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub cues: Tags,
    /// What has to hold for it to apply at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require: Option<Condition>,
    /// What has to hold for it to count while it lasts: inhibited (its
    /// modifiers and tags taken back, its time paused) while it doesn't.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ongoing: Option<Condition>,
    /// What takes it off as soon as it holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove_if: Option<Condition>,
    /// Active effects with any of these tags, taken off as it applies.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub removes: Tags,
    /// Effects with any of these tags can't apply while it lasts.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub immunity: Tags,
    /// How applying it again stacks: a new active effect each time, unless it
    /// says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stacking: Option<Stacking>,
    /// Effects applied to its target as it applies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_apply: Vec<Key<AnyEffect>>,
    /// Effects applied to its target as it's taken off, however.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_remove: Vec<Key<AnyEffect>>,
    /// Effects applied to its target as it runs out of time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub on_expire: Vec<Key<AnyEffect>>,
}

/// How long an effect lasts: an instant, for a while, or until it's removed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EffectDuration<M = Magnitude> {
    /// Not at all: it executes once, changing bases, and is gone.
    Instant,
    /// Until it's taken off.
    Infinite,
    /// So many seconds.
    For(M),
}

/// How often a lasting effect executes, and whether on apply too.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Period<M = Magnitude> {
    /// Every so many seconds: more than 0.
    pub every: M,
    /// Whether it executes as it's applied, too.
    #[serde(default)]
    pub on_apply: bool,
}

/// A modifier as data writes it: what it changes, by name, how, and how much.
///
/// It changes an attribute, or deals a [route](crate::code::Route) its amount:
///
/// ```
/// # use bevy_mettle_attribute::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// # #[derive(Attribute)] #[attribute(base = 100.0)] struct Life;
/// # #[derive(Route)] struct IncomingDamage;
/// EffectModifier::new::<Life>(
///     Operation::Flat,
///     Magnitude::param("param.damage"),
/// );
/// EffectModifier::route::<IncomingDamage>(40.0);
/// ```
///
/// One of `attribute` and `route`, never both; a route is dealt flat amounts,
/// and `op` says so unless it says otherwise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectModifier {
    /// The attribute it changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<Key<AnyAttribute>>,
    /// The route it deals its amount, for its router.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Key<AnyRoute>>,
    /// How: flat, unless it says.
    #[serde(default = "flat")]
    pub op: Operation,
    /// By how much.
    pub value: Magnitude,
    /// When it counts, and how much, as an attribute's modifier does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
    /// That it counts once for each whole so much of an attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per: Option<EffectPer>,
}

/// That a modifier counts once for each whole `every` of an attribute, by name.
///
/// The attribute named as data names it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectPer {
    /// The attribute it scales with.
    pub attribute: Key<AnyAttribute>,
    /// How much of it makes one: more than 0.
    pub every: f32,
}

/// What a modifier does unless it says: adds.
fn flat() -> Operation {
    Operation::Flat
}

impl EffectModifier {
    /// `op` of `value` to `A`.
    #[must_use]
    pub fn new<A: Attribute>(op: Operation, value: impl Into<Magnitude>) -> Self {
        Self::named(Key::from_static(A::NAME), op, value)
    }

    /// `value` dealt the route `R`, for its router: flat, as routes are dealt.
    #[must_use]
    pub fn route<R: Route>(value: impl Into<Magnitude>) -> Self {
        Self {
            attribute: None,
            route: Some(Key::from_static(R::NAME)),
            op: Operation::Flat,
            value: value.into(),
            condition: None,
            per: None,
        }
    }

    /// `op` of `value` to the attribute named `attribute`.
    #[must_use]
    pub fn named(attribute: Key<AnyAttribute>, op: Operation, value: impl Into<Magnitude>) -> Self {
        Self {
            attribute: Some(attribute),
            route: None,
            op,
            value: value.into(),
            condition: None,
            per: None,
        }
    }

    /// Only counting as `condition` says.
    #[must_use]
    pub fn condition(mut self, condition: Condition) -> Self {
        self.condition = Some(match self.condition.take() {
            Some(before) => before.and(condition),
            None => condition,
        });
        self
    }

    /// Counting once for each whole `every` of the target's `A`.
    #[must_use]
    pub fn per<A: Attribute>(mut self, every: f32) -> Self {
        self.per = Some(EffectPer {
            attribute: Key::from_static(A::NAME),
            every,
        });
        self
    }
}

impl EffectData {
    /// An effect called `id` lasting `duration`, doing nothing yet: for effects
    /// defined in code.
    #[must_use]
    pub fn new(id: impl Into<Key<AnyEffect>>, duration: EffectDuration) -> Self {
        Self {
            name: id.into(),
            tags: Tags::new(),
            duration,
            period: None,
            modifiers: Vec::new(),
            executions: Vec::new(),
            grants: Tags::new(),
            cues: Tags::new(),
            require: None,
            ongoing: None,
            remove_if: None,
            removes: Tags::new(),
            immunity: Tags::new(),
            stacking: None,
            on_apply: Vec::new(),
            on_remove: Vec::new(),
            on_expire: Vec::new(),
        }
    }

    /// An instant effect called `id`.
    #[must_use]
    pub fn instant(id: impl Into<Key<AnyEffect>>) -> Self {
        Self::new(id, EffectDuration::Instant)
    }

    /// An effect called `id` lasting until it's taken off.
    #[must_use]
    pub fn infinite(id: impl Into<Key<AnyEffect>>) -> Self {
        Self::new(id, EffectDuration::Infinite)
    }

    /// An effect called `id` lasting `seconds`.
    #[must_use]
    pub fn lasting(id: impl Into<Key<AnyEffect>>, seconds: impl Into<Magnitude>) -> Self {
        Self::new(id, EffectDuration::For(seconds.into()))
    }

    /// Labelled `tag` too.
    #[must_use]
    pub fn tag(mut self, tag: impl Into<TagKey>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Changing `modifier` too.
    #[must_use]
    pub fn modifier(mut self, modifier: EffectModifier) -> Self {
        self.modifiers.push(modifier);
        self
    }

    /// Executing every `every` seconds, and as it's applied if `on_apply`.
    #[must_use]
    pub fn period(mut self, every: impl Into<Magnitude>, on_apply: bool) -> Self {
        self.period = Some(Period {
            every: every.into(),
            on_apply,
        });
        self
    }

    /// Running the execution `execution` too.
    #[must_use]
    pub fn execution(mut self, execution: impl Into<Key<AnyExecution>>) -> Self {
        self.executions.push(execution.into());
        self
    }

    /// Playing the cue `tag`: held on its target while it counts, and played
    /// once each time it executes.
    #[must_use]
    pub fn cue(mut self, tag: impl Into<TagKey>) -> Self {
        self.cues.insert(tag.into());
        self
    }

    /// Granting its target `tag` while it lasts.
    #[must_use]
    pub fn grant(mut self, tag: impl Into<TagKey>) -> Self {
        self.grants.insert(tag.into());
        self
    }

    /// Applying only where `require` holds.
    #[must_use]
    pub fn require(mut self, require: Condition) -> Self {
        self.require = Some(require);
        self
    }

    /// Counting only while `ongoing` holds.
    #[must_use]
    pub fn ongoing(mut self, ongoing: Condition) -> Self {
        self.ongoing = Some(ongoing);
        self
    }

    /// Taken off as soon as `remove_if` holds.
    #[must_use]
    pub fn remove_if(mut self, remove_if: Condition) -> Self {
        self.remove_if = Some(remove_if);
        self
    }

    /// Taking active effects with `tag` off as it applies.
    #[must_use]
    pub fn removes(mut self, tag: impl Into<TagKey>) -> Self {
        self.removes.insert(tag.into());
        self
    }

    /// Keeping effects with `tag` from applying while it lasts.
    #[must_use]
    pub fn immunity(mut self, tag: impl Into<TagKey>) -> Self {
        self.immunity.insert(tag.into());
        self
    }

    /// Stacking as `stacking` says.
    #[must_use]
    pub fn stacking(mut self, stacking: Stacking) -> Self {
        self.stacking = Some(stacking);
        self
    }

    /// Applying `effect` to its target as it applies.
    #[must_use]
    pub fn on_apply(mut self, effect: impl Into<Key<AnyEffect>>) -> Self {
        self.on_apply.push(effect.into());
        self
    }

    /// Applying `effect` to its target as it's taken off.
    #[must_use]
    pub fn on_remove(mut self, effect: impl Into<Key<AnyEffect>>) -> Self {
        self.on_remove.push(effect.into());
        self
    }

    /// Applying `effect` to its target as it runs out of time.
    #[must_use]
    pub fn on_expire(mut self, effect: impl Into<Key<AnyEffect>>) -> Self {
        self.on_expire.push(effect.into());
        self
    }

    /// Whether it lasts at all.
    #[must_use]
    pub fn lasts(&self) -> bool {
        !matches!(self.duration, EffectDuration::Instant)
    }
}
