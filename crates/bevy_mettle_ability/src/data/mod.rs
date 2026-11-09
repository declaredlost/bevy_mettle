//! Abilities as data: what runs them, what they cost, and what effects grant.

mod def;

use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::registry::{Kind, Name};
use bevy_mettle_core::tag::{TagKey, Tags};
use bevy_mettle_effect::data::AnyEffect;
use serde::{Deserialize, Serialize};

pub use self::def::AbilityDef;
use crate::behaviour::AnyBehaviour;

/// The ability kind: what content names, and an [`AbilityId`](crate::AbilityId)
/// numbers.
///
/// An ability's name in content is a `Name<AnyAbility>`, until it's resolved
/// to an [`AbilityId`](crate::AbilityId).
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyAbility {}

/// An ability, as data: what runs it, what it costs, and what activates it.
///
/// Unreal's `GameplayAbility`, its properties. Named by an `Name<AnyAbility>`,
/// its `id`.
///
/// Written by a game's content, in whatever format it loads with serde, or
/// built in code:
///
/// ```
/// # use bevy_mettle_ability::prelude::*;
/// # use bevy_mettle_core::prelude::*;
/// let fireball = AbilityData::new("fireball", "projectile_cast")
///     .tag("ability.spell.fire")
///     .cost("fireball.cost")
///     .cooldown("fireball.cooldown")
///     .require(!Condition::owner_has("state.silenced"))
///     .blocked_by("state.stunned")
///     .cancels("ability.channel")
///     .blocks("ability.spell")
///     .grant("state.casting");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbilityData {
    /// Its name.
    pub name: Name<AnyAbility>,
    /// What it is: what blocking, cancelling and activating by tag match on.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
    /// The behaviour that runs it, registered in code.
    pub behaviour: Name<AnyBehaviour>,
    /// An instant effect it costs, applied to its owner as it commits: it can't
    /// activate if paying would take an attribute below 0.
    #[serde(
        default,
        with = "bevy_mettle_core::optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub cost: Option<Name<AnyEffect>>,
    /// A lasting effect applied to its owner as it commits: it can't activate
    /// while its owner owns what that effect grants.
    #[serde(
        default,
        with = "bevy_mettle_core::optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub cooldown: Option<Name<AnyEffect>>,
    /// What has to hold for it to activate: weighed in its own tags
    /// (`ReadTagged`) and its owner's (`OwnerHas`).
    #[serde(
        default,
        with = "bevy_mettle_core::optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub require: Option<Condition>,
    /// What interrupts it: while active, it's cancelled as soon as this holds,
    /// weighed as `require` is; holding already, it can't activate.
    #[serde(
        default,
        with = "bevy_mettle_core::optional",
        skip_serializing_if = "Option::is_none"
    )]
    pub cancel_if: Option<Condition>,
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
    /// How it runs over a network: predicted by the client, unless it says.
    #[serde(default)]
    pub net: NetExecution,
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
    /// An ability called `id`, run by `behaviour`, costing nothing yet: for
    /// abilities defined in code.
    #[must_use]
    pub fn new(id: impl Into<Name<AnyAbility>>, behaviour: impl Into<Name<AnyBehaviour>>) -> Self {
        Self {
            name: id.into(),
            tags: Tags::new(),
            behaviour: behaviour.into(),
            cost: None,
            cooldown: None,
            require: None,
            cancel_if: None,
            blocked_by: Tags::new(),
            cancels: Tags::new(),
            blocks: Tags::new(),
            grants: Tags::new(),
            cues: Tags::new(),
            triggers: Vec::new(),
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

    /// Costing `effect`.
    #[must_use]
    pub fn cost(mut self, effect: impl Into<Name<AnyEffect>>) -> Self {
        self.cost = Some(effect.into());
        self
    }

    /// Cooling down by `effect`.
    #[must_use]
    pub fn cooldown(mut self, effect: impl Into<Name<AnyEffect>>) -> Self {
        self.cooldown = Some(effect.into());
        self
    }

    /// Activating only where `require` holds.
    #[must_use]
    pub fn require(mut self, require: Condition) -> Self {
        self.require = Some(require);
        self
    }

    /// Interrupted as soon as `cancel_if` holds while it's active: stunned
    /// mid-cast.
    #[must_use]
    pub fn cancel_if(mut self, cancel_if: Condition) -> Self {
        self.cancel_if = Some(cancel_if);
        self
    }

    /// Kept from activating while its owner owns `tag`.
    #[must_use]
    pub fn blocked_by(mut self, tag: impl Into<TagKey>) -> Self {
        self.blocked_by.insert(tag.into());
        self
    }

    /// Cancelling active abilities tagged `tag` as it activates.
    #[must_use]
    pub fn cancels(mut self, tag: impl Into<TagKey>) -> Self {
        self.cancels.insert(tag.into());
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

    /// Granting its owner `tag` while it's active.
    #[must_use]
    pub fn grant(mut self, tag: impl Into<TagKey>) -> Self {
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

    /// Run over a network as `net` says.
    #[must_use]
    pub fn net(mut self, net: NetExecution) -> Self {
        self.net = net;
        self
    }
}

/// What an effect grants while it lasts: the abilities it gives its target.
///
/// The ability crate's part of an effect, as Unreal's abilities component is
/// (an effect knows nothing of abilities). Equipping a staff applies its
/// effect, which grants its skills; taking it off takes them away.
///
/// ```
/// # use bevy_mettle_ability::prelude::*;
/// let staff = Grant::new("staff", ["channel"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    /// The effect: a lasting one.
    pub effect: Name<AnyEffect>,
    /// The abilities its target is granted while it lasts.
    pub abilities: Vec<Name<AnyAbility>>,
}

impl Grant {
    /// `effect` granting `abilities`.
    #[must_use]
    pub fn new<A: Into<Name<AnyAbility>>>(
        effect: impl Into<Name<AnyEffect>>,
        abilities: impl IntoIterator<Item = A>,
    ) -> Self {
        Self {
            effect: effect.into(),
            abilities: abilities.into_iter().map(Into::into).collect(),
        }
    }
}
