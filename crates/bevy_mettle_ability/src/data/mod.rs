//! Abilities as data: what runs them, what they cost, and what effects grant.

mod def;

use bevy_mettle_attribute::Attribute;
use bevy_mettle_attribute::modifier::Operation;
use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::registry::{AnyAbility, AnyAttribute, AnyEffect, Key};
use bevy_mettle_core::tag::{TagKey, Tags};
use bevy_mettle_effect::Effect;
use bevy_mettle_effect::data::{EffectData, EffectModifier, Magnitude};
use serde::{Deserialize, Serialize};

pub use self::def::AbilityDef;
use crate::behaviour::{AnyBehaviour, Behaviour};

/// An ability's definition: what runs it, what it costs, what activates it.
///
/// Unreal's `GameplayAbility`, its properties. Named by a `Key<AnyAbility>`.
/// An [`Ability`](crate::Ability) type defines one, each key of its derive
/// the builder method of the same name; content adds more, written in
/// whatever format the game loads with serde.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mettle_ability::prelude::*;
/// # use bevy_mettle_attribute::prelude::*;
/// # use bevy_mettle_core::prelude::*;
/// # #[derive(Attribute)] struct Mana;
/// # #[derive(Component, Default, Behaviour)] struct CastProjectile;
/// # #[derive(Tag)] struct Fire;
/// # #[derive(Tag)] struct Silenced;
/// # #[derive(Tag)] struct Casting;
/// #[derive(Ability)]
/// #[ability(behaviour::<CastProjectile>, tag = Fire, owner_tag = Casting)]
/// #[ability(cost = Cost::of::<Mana>(20.0), cooldown = 2.0)]
/// #[ability(require = !Condition::owner_has(Silenced))]
/// struct Fireball;
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilityData {
    /// Its name.
    pub name: Key<AnyAbility>,
    /// What it is: what blocking, cancelling and activating by tag match on.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
    /// The behaviour that runs it, registered in code: every ability has one.
    pub behaviour: Option<Key<AnyBehaviour>>,
    /// What it costs, paid from its owner as it commits: it can't activate if
    /// paying would take an attribute below 0. Paid by an instant effect
    /// mettle makes of them, `<its key>.cost`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cost: Vec<Cost>,
    /// How many seconds it cools down for once it commits: it can't activate
    /// while it does. Run by a lasting effect mettle makes, `<its
    /// key>.cooldown`, granting its owner its
    /// [`cooldown_tag`](Self::cooldown_tag).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown: Option<Magnitude>,
    /// The tag its owner holds while it cools down, which it can't activate
    /// while its owner holds: `cooldown.<its key>`, unless it says. Abilities
    /// sharing one share a cooldown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooldown_tag: Option<TagKey>,
    /// What has to hold for it to activate: weighed in its own tags
    /// (`ReadTagged`) and its owner's (`OwnerHas`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub require: Option<Condition>,
    /// What interrupts it: while active, it's ended (cancelled) as soon as
    /// this holds, weighed as `require` is; holding already, it can't
    /// activate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_if: Option<Condition>,
    /// Active abilities tagged any of these are ended (cancelled) as it
    /// activates.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub ends: Tags,
    /// Abilities tagged any of these can't activate while it's active.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub blocks: Tags,
    /// Tags its owner holds while it's active.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub owner_tags: Tags,
    /// Lasting cues held on its owner while it's active: a cast's glow.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub cues: Tags,
    /// What activates it besides being asked to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<Trigger>,
    /// Lasting effects that grant it to their target while they last:
    /// equipping a staff applies its effect, which grants its skills.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub granted_by: Vec<Key<AnyEffect>>,
    /// When its cost and cooldown are applied: as it activates, unless it says.
    #[serde(default)]
    pub commit: Commit,
    /// How it runs over a network: predicted by the client, unless it says.
    #[serde(default)]
    pub net: NetExecution,
}

/// What an ability costs: an amount of one of its owner's attributes.
///
/// ```
/// # use bevy_mettle_ability::prelude::*;
/// # use bevy_mettle_attribute::prelude::*;
/// # #[derive(Attribute)] struct Mana;
/// let cost = Cost::of::<Mana>(20.0);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    /// The attribute paid from.
    pub attribute: Key<AnyAttribute>,
    /// How much: taken off its base.
    pub amount: Magnitude,
}

impl Cost {
    /// `amount` of `A`.
    #[must_use]
    pub fn of<A: Attribute>(amount: impl Into<Magnitude>) -> Self {
        Self {
            attribute: Key::of::<A>(),
            amount: amount.into(),
        }
    }
}

/// What activates an ability besides being asked to: an event, or a tag owned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trigger {
    /// A gameplay event sent to its owner with this tag, or one under it:
    /// activated with the event.
    Event(TagKey),
    /// Its owner coming to own this tag.
    TagAdded(TagKey),
    /// Its owner owning this tag: activated as it comes to, cancelled as it no
    /// longer does.
    TagPresent(TagKey),
}

/// When an ability's cost and cooldown are applied: as it activates, or later.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize
)]
pub enum Commit {
    /// As it activates.
    #[default]
    OnActivate,
    /// When its behaviour commits it (after a cast time, say): cancelled if it
    /// can't pay then.
    Manual,
}

/// How an ability runs over a network: who runs it, and who's told.
///
/// Unreal's net execution policies. Standalone, every one runs where it's
/// activated. Chosen in content, as how an ability plays is.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize
)]
pub enum NetExecution {
    /// The client runs it at once and asks the server, which runs it too and
    /// confirms or rejects it: what a player's own abilities want.
    #[default]
    Predicted,
    /// Only the server runs it. A client's ask is sent, not run: the client
    /// sees what it does through replicas and cues, and is told if it fails.
    /// For outcomes a client can't know: a random proc, a hidden target.
    ServerOnly,
    /// The server runs it, then tells the owning client, which runs its
    /// behaviour under the server's activation, without paying or checking
    /// again: for client-side logic (aim, camera, local feedback) on what the
    /// server decides.
    ServerInitiated,
    /// Runs only where it's activated, and is never sent: zoom, an aim
    /// preview, a menu. It can't have a cost or a cooldown: what it'd pay, a
    /// server's replicated values would undo.
    LocalOnly,
}

impl AbilityData {
    /// An ability called `name`, with no behaviour yet, costing nothing:
    /// what an [`Ability`](crate::Ability)'s `define` is handed, named.
    #[must_use]
    pub fn new(name: impl Into<Key<AnyAbility>>) -> Self {
        Self {
            name: name.into(),
            tags: Tags::new(),
            behaviour: None,
            cost: Vec::new(),
            cooldown: None,
            cooldown_tag: None,
            require: None,
            end_if: None,
            ends: Tags::new(),
            blocks: Tags::new(),
            owner_tags: Tags::new(),
            cues: Tags::new(),
            triggers: Vec::new(),
            granted_by: Vec::new(),
            commit: Commit::OnActivate,
            net: NetExecution::Predicted,
        }
    }

    /// Labelled `tag` too.
    #[must_use]
    pub fn tag(mut self, tag: impl Into<TagKey>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Run by the behaviour `B`.
    #[must_use]
    pub fn behaviour<B: Behaviour>(mut self) -> Self {
        self.behaviour = Some(Key::of::<B>());
        self
    }

    /// Costing `cost` too.
    #[must_use]
    pub fn cost(mut self, cost: Cost) -> Self {
        self.cost.push(cost);
        self
    }

    /// Cooling down for `seconds` once it commits.
    #[must_use]
    pub fn cooldown(mut self, seconds: impl Into<Magnitude>) -> Self {
        self.cooldown = Some(seconds.into());
        self
    }

    /// Its owner holding `tag` while it cools down, in place of
    /// `cooldown.<its key>`: abilities sharing a tag share a cooldown.
    #[must_use]
    pub fn cooldown_tag(mut self, tag: impl Into<TagKey>) -> Self {
        self.cooldown_tag = Some(tag.into());
        self
    }

    /// Activating only where `require` holds.
    #[must_use]
    pub fn require(mut self, require: Condition) -> Self {
        self.require = Some(require);
        self
    }

    /// Interrupted as soon as `end_if` holds while it's active: stunned
    /// mid-cast.
    #[must_use]
    pub fn end_if(mut self, end_if: Condition) -> Self {
        self.end_if = Some(end_if);
        self
    }

    /// Ending (cancelling) active abilities tagged `tag` as it activates.
    #[must_use]
    pub fn ends(mut self, tag: impl Into<TagKey>) -> Self {
        self.ends.insert(tag.into());
        self
    }

    /// Keeping abilities tagged `tag` from activating while it's active.
    #[must_use]
    pub fn blocks(mut self, tag: impl Into<TagKey>) -> Self {
        self.blocks.insert(tag.into());
        self
    }

    /// Holding the cue `tag` on its owner while it's active.
    #[must_use]
    pub fn cue(mut self, tag: impl Into<TagKey>) -> Self {
        self.cues.insert(tag.into());
        self
    }

    /// Its owner holding `tag` while it's active.
    #[must_use]
    pub fn owner_tag(mut self, tag: impl Into<TagKey>) -> Self {
        self.owner_tags.insert(tag.into());
        self
    }

    /// Activated by `trigger` too.
    #[must_use]
    pub fn trigger(mut self, trigger: Trigger) -> Self {
        self.triggers.push(trigger);
        self
    }

    /// Granted to their target by `E` while it lasts.
    #[must_use]
    pub fn granted_by<E: Effect>(mut self) -> Self {
        self.granted_by.push(Key::of::<E>());
        self
    }

    /// Committing as `commit` says.
    #[must_use]
    pub fn commit(mut self, commit: Commit) -> Self {
        self.commit = commit;
        self
    }

    /// Run over a network as `net` says.
    #[must_use]
    pub fn net(mut self, net: NetExecution) -> Self {
        self.net = net;
        self
    }
}

/// What an ability pays with, made from its definition: its cost and its
/// cooldown, as effects, and the tag its cooldown grants.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Upkeep {
    /// Its cost, an instant effect, if it costs anything.
    pub(crate) cost: Option<EffectData>,
    /// Its cooldown, a lasting effect granting `tag`, if it has one.
    pub(crate) cooldown: Option<EffectData>,
    /// The tag its cooldown grants.
    pub(crate) tag: Option<TagKey>,
}

impl AbilityData {
    /// The key of the effect mettle makes of its cost.
    pub(crate) fn cost_key(&self) -> Key<AnyEffect> {
        Key::new(format!("{}.cost", self.name))
    }

    /// The key of the effect mettle makes of its cooldown.
    pub(crate) fn cooldown_key(&self) -> Key<AnyEffect> {
        Key::new(format!("{}.cooldown", self.name))
    }

    /// What it pays with, as effects.
    pub(crate) fn upkeep(&self) -> Upkeep {
        let cost = (!self.cost.is_empty()).then(|| {
            self.cost
                .iter()
                .fold(EffectData::new(self.cost_key()), |effect, cost| {
                    effect.modifier(EffectModifier::named(
                        cost.attribute.clone(),
                        Operation::Flat,
                        cost.amount.clone().times(-1.0),
                    ))
                })
        });
        let tag = self.cooldown.as_ref().map(|_| {
            self.cooldown_tag
                .unwrap_or_else(|| TagKey::named(&format!("cooldown.{}", self.name)))
        });
        let cooldown = self.cooldown.as_ref().zip(tag).map(|(seconds, tag)| {
            EffectData::new(self.cooldown_key())
                .lasting(seconds.clone())
                .owner_tag(tag)
        });
        Upkeep {
            cost,
            cooldown,
            tag,
        }
    }
}
