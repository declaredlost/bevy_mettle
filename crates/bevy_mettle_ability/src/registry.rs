//! The abilities there are, checked.

use std::any::TypeId;
use std::sync::Arc;

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy_mettle_core::registry::{
    AbilityId, AnyAbility, AnyEffect, Definitions, EffectId, Key, Registry,
};
use bevy_mettle_core::tag::{TagKey, TagRegistry};
use bevy_mettle_core::{Error, Errors};
use bevy_mettle_effect::EffectRegistry;
use bevy_mettle_effect::data::{EffectData, EffectDuration};

use crate::ability::Ability;
use crate::behaviour::{AnyBehaviour, BehaviourRegistry};
use crate::data::{AbilityData, AbilityDef, NetExecution, Trigger};

/// The abilities there are, checked against what they name, and numbered.
///
/// Checked against the tags, effects and behaviours there are.
///
/// Made by checking their definitions. Each ability is numbered, an
/// [`AbilityId`], in order of name. A resource: in an app with `bevy_mettle`,
/// it's built as the app starts and as content changes, and granted abilities
/// take the new definitions.
///
/// An app's registry also holds the ability types code defines
/// ([`add_ability`](crate::AbilityApp::add_ability)). So an app's abilities
/// are checked by [`rebuild`](Self::rebuild)ing its registry, which defines
/// those types again, adds content's, and keeps the numbers abilities had:
/// what a load, a reload or a hot patch does. [`build`](Self::build) makes a
/// registry of content alone, knowing no types: for tools and tests outside
/// an app. Inserted into an app, it would forget what code defines.
///
/// What abilities pay with (their costs and cooldowns) are effects mettle
/// makes of them, [`upkeep`](Self::upkeep), checked with the other effects
/// before the abilities are.
///
/// Read as a [`Registry`]: `get(id)`, `id(name)`, `id_of::<A>()`, `name(id)`,
/// `iter()`.
#[derive(Resource, Deref, Clone, Default)]
pub struct AbilityRegistry {
    #[deref]
    registry: Registry<AnyAbility, Arc<AbilityDef>>,
    definitions: Definitions<AnyAbility, AbilityData>,
    /// The abilities each effect grants.
    grants: Arc<HashMap<EffectId, Vec<AbilityId>>>,
}

impl std::fmt::Debug for AbilityRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut grants: Vec<(&EffectId, &Vec<AbilityId>)> = self.grants.iter().collect();
        grants.sort();
        f.debug_struct("AbilityRegistry")
            .field("abilities", &self.registry)
            .field("definitions", &self.definitions)
            .field("grants", &grants)
            .finish()
    }
}

/// What abilities are checked against: what they can name.
#[derive(Clone, Copy)]
pub struct AbilityScope<'a> {
    /// The gameplay tags there are.
    pub tags: &'a TagRegistry,
    /// The effects there are: what abilities pay with, and what grants them.
    pub effects: &'a EffectRegistry,
    /// The behaviours the app added.
    pub behaviours: &'a BehaviourRegistry,
}

/// What abilities pay with: their costs and cooldowns, as effects and tags.
///
/// What's checked as content, with the effects and tags data adds: each cost
/// an effect, each cooldown an effect and the tag it grants.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Upkeep {
    /// Each cost, an instant effect `<ability>.cost`, and each cooldown, a
    /// lasting effect `<ability>.cooldown`.
    pub effects: Vec<EffectData>,
    /// The tags cooldowns grant unless they say, `cooldown.<ability>`, and
    /// those they're under.
    pub tags: Vec<TagKey>,
}

impl AbilityRegistry {
    /// A registry of `content`, from nothing, checked against what it can
    /// name: knowing no ability types code defines. For tools and tests; an
    /// app's abilities go through [`rebuild`](Self::rebuild).
    ///
    /// # Errors
    ///
    /// Everything wrong, at once: abilities defined twice; tags, effects and
    /// behaviours named that aren't; an ability with no behaviour; what it
    /// pays with missing from `against`'s effects; and grants by effects
    /// that aren't, or don't last.
    pub fn build(content: &[AbilityData], against: AbilityScope<'_>) -> Result<Self, Errors> {
        Self::default().rebuild(content, against)
    }

    /// The registry of the ability types code defines here and `content`'s,
    /// as [`build`](Self::build) makes it, each ability here keeping its
    /// number: new ones numbered after the last, in order of name. What a
    /// load, a reload or a hot patch does. Each type's `define` is run again.
    ///
    /// # Errors
    ///
    /// As `build`'s, and content defining an ability code defines.
    pub fn rebuild(
        &self,
        content: &[AbilityData],
        against: AbilityScope<'_>,
    ) -> Result<Self, Errors> {
        let AbilityScope {
            tags,
            effects,
            behaviours,
        } = against;
        let (definitions, mut errors) = self.definitions(content);
        let mut names: Vec<&str> = definitions.iter().map(|def| def.name.as_str()).collect();
        names.sort_unstable();
        let mut registry = self.registry.renumbered(names);
        let mut compiled = Vec::new();
        let mut granted: HashMap<EffectId, Vec<AbilityId>> = HashMap::default();
        for def in &definitions {
            let Some(id) = registry.id(def.name.as_str()) else {
                continue;
            };
            for effect in &def.granted_by {
                if let Some(effect) = granting(effect, def, effects, &mut errors) {
                    granted.entry(effect).or_default().push(id);
                }
            }
            if let Some(def) = compile(id, def, tags, effects, behaviours, &mut errors) {
                compiled.push(def);
            }
        }
        for def in compiled {
            registry.set(def.id, Arc::new(def));
        }
        errors.sort_by_key(ToString::to_string);
        if errors.is_empty() {
            Ok(Self {
                registry,
                definitions: self.definitions.clone(),
                grants: Arc::new(granted),
            })
        } else {
            Err(Errors(errors))
        }
    }

    /// What the ability types code defines here and `content`'s pay with:
    /// what's checked with the effects and the tags, before the abilities
    /// are.
    #[must_use]
    pub fn upkeep(&self, content: &[AbilityData]) -> Upkeep {
        let mut upkeep = Upkeep::default();
        let (definitions, _) = self.definitions(content);
        for def in &definitions {
            let paid = def.upkeep();
            upkeep.effects.extend(paid.cost);
            upkeep.effects.extend(paid.cooldown);
            if def.cooldown_tag.is_none()
                && let Some(tag) = paid.tag
            {
                for tag in tag.lineage() {
                    if !upkeep.tags.contains(&tag) {
                        upkeep.tags.push(tag);
                    }
                }
            }
        }
        upkeep
    }

    /// Adds the ability `A` code defines: checked as the registry's rebuilt.
    pub fn add<A: Ability>(&mut self) {
        self.definitions.add::<A>(A::define);
        self.registry.name_type(TypeId::of::<A>(), A::KEY);
    }

    /// The abilities `effect` grants while it lasts.
    #[must_use]
    pub fn granted_by(&self, effect: EffectId) -> &[AbilityId] {
        self.grants.get(&effect).map_or(&[], Vec::as_slice)
    }

    /// The ability types code defines here, defined, and `content`'s, with
    /// what's wrong with them: content defining one twice, or one code does.
    fn definitions(&self, content: &[AbilityData]) -> (Vec<AbilityData>, Vec<Error>) {
        let mut errors = Vec::new();
        let mut definitions = self.definitions.definitions(AbilityData::new);
        let mut names: HashSet<String> =
            definitions.iter().map(|def| def.name.to_string()).collect();
        for def in content {
            if self.definitions.defines(def.name.as_str()) {
                errors.push(Error::DefinedInCode {
                    kind: "ability",
                    name: def.name.to_string(),
                });
            } else if !names.insert(def.name.to_string()) {
                errors.push(Error::Duplicate {
                    kind: "ability",
                    name: def.name.to_string(),
                });
            } else {
                definitions.push(def.clone());
            }
        }
        (definitions, errors)
    }
}

/// The number of `effect`, which grants `def`, if it's an effect there is and
/// lasts; what's wrong noted if not.
fn granting(
    effect: &Key<AnyEffect>,
    def: &AbilityData,
    effects: &EffectRegistry,
    errors: &mut Vec<Error>,
) -> Option<EffectId> {
    let within = Some(def.name.as_str());
    let Some(id) = effects.id(effect.as_str()) else {
        errors.push(Error::unknown::<AnyEffect>(effect.as_str(), within));
        return None;
    };
    let lasts = effects
        .get(id)
        .is_some_and(|effect| !matches!(effect.duration, EffectDuration::Instant));
    if !lasts {
        errors.push(Error::invalid(
            format!("granted by `{effect}`: it's instant, so it grants nothing for any while"),
            within,
        ));
        return None;
    }
    Some(id)
}

/// `def` compiled, or what's wrong with it noted in `errors`.
fn compile(
    id: AbilityId,
    def: &AbilityData,
    tags: &TagRegistry,
    effects: &EffectRegistry,
    behaviours: &BehaviourRegistry,
    errors: &mut Vec<Error>,
) -> Option<AbilityDef> {
    let before = errors.len();
    let within = Some(def.name.as_str());
    let mut push = |result: Result<(), Error>| {
        if let Err(error) = result {
            errors.push(error);
        }
    };
    for set in [
        &def.tags,
        &def.ends,
        &def.blocks,
        &def.owner_tags,
        &def.cues,
    ] {
        push(tags.resolve_all(set.written(), within));
    }
    for condition in [&def.require, &def.end_if].into_iter().flatten() {
        push(condition.check(tags, within));
    }
    for trigger in &def.triggers {
        let (Trigger::Event(tag) | Trigger::TagAdded(tag) | Trigger::TagPresent(tag)) = trigger;
        push(tags.resolve(*tag, within).map(drop));
    }
    let behaviour = match &def.behaviour {
        None => {
            push(Err(Error::invalid(
                "an ability with no behaviour: nothing runs it".to_owned(),
                within,
            )));
            None
        }
        Some(named) => {
            let found = behaviours.id(named.as_str());
            if found.is_none() {
                push(Err(Error::unknown::<AnyBehaviour>(named.as_str(), within)));
            }
            found
        }
    };
    if let Some(input) = behaviour.and_then(|behaviour| behaviours.input(behaviour))
        && !def.triggers.is_empty()
    {
        push(Err(Error::invalid(
            format!(
                "triggers on an ability whose behaviour takes {input}: nothing triggering it gives \
                 it"
            ),
            within,
        )));
    }
    let (cost, cooldown) = paid(def, effects, errors);
    if errors.len() > before {
        return None;
    }
    Some(AbilityDef {
        id,
        name: def.name.clone(),
        tags: def.tags.clone(),
        behaviour: behaviour?,
        cost,
        cooldown,
        require: def.require.clone(),
        end_if: def.end_if.clone(),
        ends: def.ends.clone(),
        blocks: def.blocks.clone(),
        owner_tags: def.owner_tags.clone(),
        cues: def.cues.clone(),
        triggers: def.triggers.clone(),
        commit: def.commit,
        net: def.net,
    })
}

/// The numbers of the effects `def` pays with, its cost and its cooldown,
/// those it has; what's wrong with paying at all as it runs over a network
/// noted in `errors`.
fn paid(
    def: &AbilityData,
    effects: &EffectRegistry,
    errors: &mut Vec<Error>,
) -> (Option<EffectId>, Option<EffectId>) {
    let within = Some(def.name.as_str());
    let mut made = |key: Key<AnyEffect>| {
        let id = effects.id(key.as_str());
        if id.is_none() {
            errors.push(Error::unknown::<AnyEffect>(key.as_str(), within));
        }
        id
    };
    let cost = if def.cost.is_empty() {
        None
    } else {
        made(def.cost_key())
    };
    let cooldown = if def.cooldown.is_some() {
        made(def.cooldown_key())
    } else {
        None
    };
    if def.net == NetExecution::LocalOnly && (!def.cost.is_empty() || def.cooldown.is_some()) {
        errors.push(Error::invalid(
            "a cost or a cooldown on an ability run locally only: a server's values would undo \
             what it paid"
                .to_owned(),
            within,
        ));
    }
    (cost, cooldown)
}
