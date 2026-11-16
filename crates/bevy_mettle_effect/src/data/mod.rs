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

pub use self::def::{CaptureDef, EffectDef, ExecutionDef, MagnitudeDef, ModifierDef, Period};
pub use self::magnitude::{AttributeRead, Magnitude, Read, Side};
pub use self::stacking::{Expire, Reapply, StackBy, Stacking};
use crate::Effect;
use crate::code::{AnyExecution, AnyRoute, Execution, Route};

/// An effect's definition: what it changes, for how long, and what it tags.
///
/// Unreal's `GameplayEffect`: also what it needs, and how it stacks. Named by
/// a `Key<AnyEffect>`. An [`Effect`] type defines one, each key of its
/// derive the builder method of the same name; content adds more, written in
/// whatever format the game loads with serde.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy::ecs::entity::MapEntities;
/// # use bevy_mettle_attribute::prelude::*;
/// # use bevy_mettle_core::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Attribute)] #[attribute(base = 100.0)] struct Life;
/// # #[derive(Tag)] struct Burning;
/// # #[derive(Tag)] struct Wet;
/// # #[derive(Reflect, Clone, PartialEq, Serialize, Deserialize, MapEntities)]
/// # #[reflect(ContextData)] struct Damage { amount: f32 }
/// #[derive(Effect)]
/// #[effect(lasting = 4.0, period = 0.5, execute_on_apply, owner_tag = Burning)]
/// #[effect(flat::<Life>(Magnitude::context::<Damage>("amount").times(-1.0)))]
/// #[effect(ongoing = !Condition::owner_has(Wet))]
/// struct Burn;
///
/// // The same, by hand:
/// fn burn(effect: EffectData) -> EffectData {
///     effect
///         .lasting(4.0)
///         .period(0.5)
///         .execute_on_apply()
///         .owner_tag(Burning)
///         .flat::<Life>(Magnitude::context::<Damage>("amount").times(-1.0))
///         .ongoing(!Condition::owner_has(Wet))
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectData {
    /// Its name.
    pub name: Key<AnyEffect>,
    /// What it is: what ending, blocking and requirements match on.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
    /// How long it lasts.
    pub duration: EffectDuration,
    /// That it executes every so many seconds while it lasts, instead of its
    /// modifiers counting all along: damage over time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period: Option<Magnitude>,
    /// That a periodic effect executes as it's applied, too.
    #[serde(default, skip_serializing_if = "is_false")]
    pub execute_on_apply: bool,
    /// What it changes. An instant or periodic effect changes bases as it
    /// executes; a lasting one counts as modifiers while it lasts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modifiers: Vec<EffectModifier>,
    /// Formulas in code, run as it executes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub executions: Vec<Key<AnyExecution>>,
    /// Tags its target, the owner, holds while it lasts.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub owner_tags: Tags,
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
    /// What ends it as soon as it holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_if: Option<Condition>,
    /// Active effects with any of these tags, ended as it applies.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub ends: Tags,
    /// Effects with any of these tags can't apply while it lasts.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub blocks: Tags,
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

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_false(value: &bool) -> bool {
    !*value
}

/// A modifier as data writes it: what it changes, by name, how, and how much.
///
/// It changes an attribute, or routes its amount into a
/// [route](crate::code::Route):
///
/// ```
/// # use bevy_mettle_attribute::prelude::*;
/// # use bevy_mettle_core::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// # #[derive(Attribute)] #[attribute(base = 100.0)] struct Life;
/// # #[derive(Route)] struct IncomingDamage;
/// # #[derive(Tag)] struct Undead;
/// EffectModifier::new::<Life>(Operation::Multiplicative, 0.5)
///     .condition(Condition::target_has(Undead));
/// EffectModifier::route::<IncomingDamage>(40.0);
/// ```
///
/// One that's flat and counts always is
/// [`EffectData::flat`] or [`EffectData::route`].
///
/// One of `attribute` and `route`, never both; a route takes flat amounts,
/// and `op` says so unless it says otherwise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectModifier {
    /// The attribute it changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<Key<AnyAttribute>>,
    /// The route it routes its amount into, for its router.
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
        Self::named(Key::of::<A>(), op, value)
    }

    /// `value` routed into the route `R`, for its router: flat, as routes take.
    #[must_use]
    pub fn route<R: Route>(value: impl Into<Magnitude>) -> Self {
        Self {
            attribute: None,
            route: Some(Key::of::<R>()),
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
            attribute: Key::of::<A>(),
            every,
        });
        self
    }
}

impl EffectData {
    /// An instant effect called `name`, doing nothing yet: what an
    /// [`Effect`]'s `define` is handed, named.
    #[must_use]
    pub fn new(name: impl Into<Key<AnyEffect>>) -> Self {
        Self {
            name: name.into(),
            tags: Tags::new(),
            duration: EffectDuration::Instant,
            period: None,
            execute_on_apply: false,
            modifiers: Vec::new(),
            executions: Vec::new(),
            owner_tags: Tags::new(),
            cues: Tags::new(),
            require: None,
            ongoing: None,
            end_if: None,
            ends: Tags::new(),
            blocks: Tags::new(),
            stacking: None,
            on_apply: Vec::new(),
            on_remove: Vec::new(),
            on_expire: Vec::new(),
        }
    }

    /// Lasting `seconds`.
    #[must_use]
    pub fn lasting(mut self, seconds: impl Into<Magnitude>) -> Self {
        self.duration = EffectDuration::For(seconds.into());
        self
    }

    /// Lasting until it's taken off.
    #[must_use]
    pub fn infinite(mut self) -> Self {
        self.duration = EffectDuration::Infinite;
        self
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

    /// Adding `value` to `A`: a flat modifier, counting always.
    #[must_use]
    pub fn flat<A: Attribute>(self, value: impl Into<Magnitude>) -> Self {
        self.modifier(EffectModifier::new::<A>(Operation::Flat, value))
    }

    /// Routing `value` into the route `R`, for its router.
    #[must_use]
    pub fn route<R: Route>(self, value: impl Into<Magnitude>) -> Self {
        self.modifier(EffectModifier::route::<R>(value))
    }

    /// Executing every `seconds` while it lasts.
    #[must_use]
    pub fn period(mut self, seconds: impl Into<Magnitude>) -> Self {
        self.period = Some(seconds.into());
        self
    }

    /// Executing as it's applied too, as well as every period.
    #[must_use]
    pub fn execute_on_apply(mut self) -> Self {
        self.execute_on_apply = true;
        self
    }

    /// Running the execution `E` too.
    #[must_use]
    pub fn execution<E: Execution>(mut self) -> Self {
        self.executions.push(Key::of::<E>());
        self
    }

    /// Playing the cue `tag`: held on its target while it counts, and played
    /// once each time it executes.
    #[must_use]
    pub fn cue(mut self, tag: impl Into<TagKey>) -> Self {
        self.cues.insert(tag.into());
        self
    }

    /// Its target, the owner, holding `tag` while it lasts.
    #[must_use]
    pub fn owner_tag(mut self, tag: impl Into<TagKey>) -> Self {
        self.owner_tags.insert(tag.into());
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

    /// Ended as soon as `end_if` holds.
    #[must_use]
    pub fn end_if(mut self, end_if: Condition) -> Self {
        self.end_if = Some(end_if);
        self
    }

    /// Ending active effects with `tag` as it applies.
    #[must_use]
    pub fn ends(mut self, tag: impl Into<TagKey>) -> Self {
        self.ends.insert(tag.into());
        self
    }

    /// Keeping effects with `tag` from applying while it lasts.
    #[must_use]
    pub fn blocks(mut self, tag: impl Into<TagKey>) -> Self {
        self.blocks.insert(tag.into());
        self
    }

    /// Stacking as `stacking` says.
    #[must_use]
    pub fn stacking(mut self, stacking: Stacking) -> Self {
        self.stacking = Some(stacking);
        self
    }

    /// Applying `E` to its target as it applies.
    #[must_use]
    pub fn on_apply<E: Effect>(mut self) -> Self {
        self.on_apply.push(Key::of::<E>());
        self
    }

    /// Applying `E` to its target as it's taken off.
    #[must_use]
    pub fn on_remove<E: Effect>(mut self) -> Self {
        self.on_remove.push(Key::of::<E>());
        self
    }

    /// Applying `E` to its target as it runs out of time.
    #[must_use]
    pub fn on_expire<E: Effect>(mut self) -> Self {
        self.on_expire.push(Key::of::<E>());
        self
    }

    /// Whether it lasts at all.
    #[must_use]
    pub fn lasts(&self) -> bool {
        !matches!(self.duration, EffectDuration::Instant)
    }
}
