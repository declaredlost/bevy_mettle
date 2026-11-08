//! Abilities as data, and what effects grant.

use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::tag::{Tag, Tags};
use bevy_mettle_core::{Id, IdTarget};
use bevy_mettle_effect::data::Effect;
use serde::{Deserialize, Serialize};

use crate::behaviour::AnyBehaviour;

/// An ability, as data: what runs it, what it costs, and what activates it.
///
/// Unreal's `GameplayAbility`, its properties. Named by an `Id<Ability>`, its
/// `id`.
///
/// ```ron
/// (
///     id: "fireball",
///     tags: ["ability.spell.fire"],
///     behaviour: "projectile_cast",
///     cost: "fireball.cost",
///     cooldown: "fireball.cooldown",
///     require: Not(OwnerHas("state.silenced")),
///     blocked_by: ["state.stunned"],
///     cancels: ["ability.channel"],
///     blocks: ["ability.spell"],
///     grants: ["state.casting"],
/// )
/// ```
#[derive(IdTarget)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ability {
    /// Its id.
    pub id: Id<Ability>,
    /// What it is: what blocking, cancelling and activating by tag match on.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
    /// The behaviour that runs it, registered in code.
    pub behaviour: Id<AnyBehaviour>,
    /// An instant effect it costs, applied to its owner as it commits: it can't
    /// activate if paying would take an attribute below 0.
    #[serde(
        default,
        with = "bevy_mettle_core::optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub cost: Option<Id<Effect>>,
    /// A lasting effect applied to its owner as it commits: it can't activate
    /// while its owner owns what that effect grants.
    #[serde(
        default,
        with = "bevy_mettle_core::optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub cooldown: Option<Id<Effect>>,
    /// What has to hold for it to activate: weighed in its own tags
    /// (`ReadTagged`) and its owner's (`OwnerHas`).
    #[serde(
        default,
        with = "bevy_mettle_core::optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub require: Option<Condition>,
    /// It can't activate while its owner owns any of these.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub blocked_by: Tags,
    /// Active abilities tagged any of these are cancelled as it activates.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub cancels: Tags,
    /// Abilities tagged any of these can't activate while it's active.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub blocks: Tags,
    /// Tags its owner owns while it's active.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub grants: Tags,
    /// Lasting cues held on its owner while it's active: a cast's glow.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub cues: Tags,
    /// What activates it besides being asked to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub triggers: Vec<Trigger>,
    /// When its cost and cooldown are applied: as it activates, unless it says.
    #[serde(default)]
    pub commit: Commit,
}

/// What activates an ability besides being asked to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trigger {
    /// A gameplay event sent to its owner with this tag, or one under it:
    /// activated with the event.
    Event(Id<Tag>),
    /// Its owner coming to own this tag.
    TagAdded(Id<Tag>),
    /// Its owner owning this tag: activated as it comes to, cancelled as it no
    /// longer does.
    TagPresent(Id<Tag>),
}

/// When an ability's cost and cooldown are applied.
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

impl Ability {
    /// An ability called `id`, run by `behaviour`, costing nothing yet: for
    /// abilities defined in code.
    #[must_use]
    pub fn new(id: impl Into<Id<Ability>>, behaviour: impl Into<Id<AnyBehaviour>>) -> Self {
        Self {
            id: id.into(),
            tags: Tags::new(),
            behaviour: behaviour.into(),
            cost: None,
            cooldown: None,
            require: None,
            blocked_by: Tags::new(),
            cancels: Tags::new(),
            blocks: Tags::new(),
            grants: Tags::new(),
            cues: Tags::new(),
            triggers: Vec::new(),
            commit: Commit::OnActivate,
        }
    }

    /// Labelled `tag` too.
    #[must_use]
    pub fn tag(mut self, tag: impl Into<Id<Tag>>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Costing `effect`.
    #[must_use]
    pub fn cost(mut self, effect: impl Into<Id<Effect>>) -> Self {
        self.cost = Some(effect.into());
        self
    }

    /// Cooling down by `effect`.
    #[must_use]
    pub fn cooldown(mut self, effect: impl Into<Id<Effect>>) -> Self {
        self.cooldown = Some(effect.into());
        self
    }

    /// Activating only where `require` holds.
    #[must_use]
    pub fn require(mut self, require: Condition) -> Self {
        self.require = Some(require);
        self
    }

    /// Kept from activating while its owner owns `tag`.
    #[must_use]
    pub fn blocked_by(mut self, tag: impl Into<Id<Tag>>) -> Self {
        self.blocked_by.insert(tag.into());
        self
    }

    /// Cancelling active abilities tagged `tag` as it activates.
    #[must_use]
    pub fn cancels(mut self, tag: impl Into<Id<Tag>>) -> Self {
        self.cancels.insert(tag.into());
        self
    }

    /// Keeping abilities tagged `tag` from activating while it's active.
    #[must_use]
    pub fn blocks(mut self, tag: impl Into<Id<Tag>>) -> Self {
        self.blocks.insert(tag.into());
        self
    }

    /// Holding the cue `tag` on its owner while it's active.
    #[must_use]
    pub fn cue(mut self, tag: impl Into<Id<Tag>>) -> Self {
        self.cues.insert(tag.into());
        self
    }

    /// Granting its owner `tag` while it's active.
    #[must_use]
    pub fn grant(mut self, tag: impl Into<Id<Tag>>) -> Self {
        self.grants.insert(tag.into());
        self
    }

    /// Activated by `trigger` too.
    #[must_use]
    pub fn trigger(mut self, trigger: Trigger) -> Self {
        self.triggers.push(trigger);
        self
    }

    /// Committing as `commit` says.
    #[must_use]
    pub fn commit(mut self, commit: Commit) -> Self {
        self.commit = commit;
        self
    }
}

/// What an effect grants while it lasts: the abilities it gives its target.
///
/// The ability crate's part of an effect, as Unreal's abilities component is
/// (an effect knows nothing of abilities). Equipping a staff applies its
/// effect, which grants its skills; taking it off takes them away.
///
/// ```ron
/// [(effect: "staff", abilities: ["channel"])]
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    /// The effect: a lasting one.
    pub effect: Id<Effect>,
    /// The abilities its target is granted while it lasts.
    pub abilities: Vec<Id<Ability>>,
}

impl Grant {
    /// `effect` granting `abilities`.
    #[must_use]
    pub fn new<A: Into<Id<Ability>>>(
        effect: impl Into<Id<Effect>>,
        abilities: impl IntoIterator<Item = A>,
    ) -> Self {
        Self {
            effect: effect.into(),
            abilities: abilities.into_iter().map(Into::into).collect(),
        }
    }
}
