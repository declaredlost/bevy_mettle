//! The abilities there are, checked.

use std::any::TypeId;
use std::sync::Arc;

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy_mettle_core::registry::{AbilityId, AnyAbility, AnyEffect, EffectId, Registry};
use bevy_mettle_core::tag::TagRegistry;
use bevy_mettle_core::{Error, Errors};
use bevy_mettle_effect::EffectRegistry;
use bevy_mettle_effect::data::EffectDuration;

use crate::ability::Ability;
use crate::behaviour::{AnyBehaviour, BehaviourRegistry};
use crate::data::{AbilityData, AbilityDef, Grant, NetExecution, Trigger};

/// The abilities there are, checked against what they name, and numbered.
///
/// Checked against the tags, effects and behaviours there are.
///
/// Made by checking their definitions, and what the effects grant. Each
/// ability is numbered, an [`AbilityId`], in order of name. A resource: in an
/// app with `bevy_mettle`, its content loads it, and granted abilities take the
/// new definitions.
///
/// An app's registry also holds the ability types code names
/// ([`add_ability`](crate::AbilityApp::add_ability)), which content has to
/// define. So an app's content is checked by [`rebuild`](Self::rebuild)ing its
/// registry, which keeps those types and the numbers abilities had: what a
/// load or a hot reload does. [`build`](Self::build) makes a registry from
/// nothing, knowing no types: for tools and tests outside an app. Inserted
/// into an app, it would forget what code names.
///
/// Read as a [`Registry`]: `get(id)`, `id(name)`, `id_of::<A>()`, `name(id)`,
/// `iter()`.
#[derive(Resource, Deref, Clone, Default)]
pub struct AbilityRegistry {
    #[deref]
    registry: Registry<AnyAbility, Arc<AbilityDef>>,
    /// The abilities each effect grants.
    grants: Arc<HashMap<EffectId, Vec<AbilityId>>>,
}

impl std::fmt::Debug for AbilityRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut grants: Vec<(&EffectId, &Vec<AbilityId>)> = self.grants.iter().collect();
        grants.sort();
        f.debug_struct("AbilityRegistry")
            .field("abilities", &self.registry)
            .field("grants", &grants)
            .finish()
    }
}

/// What abilities, and the abilities effects grant, are checked against.
///
/// What they can name.
#[derive(Clone, Copy)]
pub struct AbilityScope<'a> {
    /// The gameplay tags there are.
    pub tags: &'a TagRegistry,
    /// The effects there are: costs, cooldowns, and what grants.
    pub effects: &'a EffectRegistry,
    /// The behaviours the app added.
    pub behaviours: &'a BehaviourRegistry,
}

impl AbilityRegistry {
    /// A registry of `definitions`, and of what effects `grants`, from nothing,
    /// checked against what they can name: knowing no ability types code
    /// names. For tools and tests; an app's content goes
    /// through [`rebuild`](Self::rebuild).
    ///
    /// # Errors
    ///
    /// Everything wrong, at once: abilities defined twice; tags, effects and
    /// behaviours named that aren't; a cost that isn't instant, a cooldown that
    /// doesn't last or grants nothing; and grants of effects that aren't, or
    /// don't last, or of abilities that aren't.
    pub fn build(
        definitions: &[AbilityData],
        grants: &[Grant],
        against: AbilityScope<'_>,
    ) -> Result<Self, Errors> {
        Self::default().rebuild(definitions, grants, against)
    }

    /// The registry of `definitions` and `grants`, as [`build`](Self::build)
    /// makes it, each ability here keeping its number: new ones numbered after
    /// the last, in order of name. What a reload does. The ability types code
    /// names here are kept.
    ///
    /// # Errors
    ///
    /// As `build`'s, and an ability code names that content doesn't define.
    pub fn rebuild(
        &self,
        definitions: &[AbilityData],
        grants: &[Grant],
        against: AbilityScope<'_>,
    ) -> Result<Self, Errors> {
        let AbilityScope {
            tags,
            effects,
            behaviours,
        } = against;
        let mut errors = Vec::new();
        let mut names: HashSet<&str> = HashSet::default();
        for def in definitions {
            if !names.insert(def.name.as_str()) {
                errors.push(Error::Duplicate {
                    kind: "ability",
                    name: def.name.to_string(),
                });
            }
        }
        let mut names: Vec<&str> = names.into_iter().collect();
        names.sort_unstable();
        let mut registry = self.registry.renumbered(names);
        errors.extend(registry.unnumbered().map(|name| Error::NotInContent {
            kind: "ability",
            name: name.to_owned(),
        }));
        let mut compiled = Vec::new();
        for def in definitions {
            let Some(id) = registry.id(def.name.as_str()) else {
                continue;
            };
            if compiled.iter().any(|done: &AbilityDef| done.id == id) {
                continue;
            }
            if let Some(def) = compile(id, def, tags, effects, behaviours, &mut errors) {
                compiled.push(def);
            }
        }
        let mut granted: HashMap<EffectId, Vec<AbilityId>> = HashMap::default();
        for grant in grants {
            let within = grant.effect.as_str();
            let effect = check_grant(grant, effects, &mut errors);
            let mut numbers = Vec::new();
            for ability in &grant.abilities {
                match registry.id(ability.as_str()) {
                    Some(id) => numbers.push(id),
                    None => {
                        errors.push(Error::unknown::<AnyAbility>(ability.as_str(), Some(within)));
                    }
                }
            }
            if let Some(effect) = effect {
                granted.entry(effect).or_default().extend(numbers);
            }
        }
        for def in compiled {
            registry.set(def.id, Arc::new(def));
        }
        errors.sort_by_key(ToString::to_string);
        if errors.is_empty() {
            Ok(Self {
                registry,
                grants: Arc::new(granted),
            })
        } else {
            Err(Errors(errors))
        }
    }

    /// Notes that code names the ability `A`: content has to define it.
    pub fn name_type<A: Ability>(&mut self) {
        self.registry.name_type(TypeId::of::<A>(), A::KEY);
    }

    /// The abilities `effect` grants while it lasts.
    #[must_use]
    pub fn granted_by(&self, effect: EffectId) -> &[AbilityId] {
        self.grants.get(&effect).map_or(&[], Vec::as_slice)
    }
}

/// Checks `grant`'s effect is one there is, and lasts: its number if so.
fn check_grant(
    grant: &Grant,
    effects: &EffectRegistry,
    errors: &mut Vec<Error>,
) -> Option<EffectId> {
    let within = grant.effect.as_str();
    let Some(id) = effects.id(within) else {
        errors.push(Error::unknown::<AnyEffect>(within, Some(within)));
        return None;
    };
    let lasts = effects
        .get(id)
        .is_some_and(|effect| !matches!(effect.duration, EffectDuration::Instant));
    if !lasts {
        errors.push(Error::invalid(
            format!(
                "a grant of `{}`: it's instant, so it grants nothing for any while",
                grant.effect
            ),
            None,
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
    let behaviour = behaviours.id(def.behaviour.as_str());
    match behaviour {
        None => push(Err(Error::unknown::<AnyBehaviour>(
            def.behaviour.as_str(),
            within,
        ))),
        Some(behaviour) => {
            if let Some(input) = behaviours.input(behaviour)
                && !def.triggers.is_empty()
            {
                push(Err(Error::invalid(
                    format!(
                        "triggers on an ability whose behaviour takes {input}: nothing triggering it \
                     gives it"
                    ),
                    within,
                )));
            }
        }
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
/// those there are; what's wrong with them, or with paying at all as it runs
/// over a network, noted in `errors`.
fn paid(
    def: &AbilityData,
    effects: &EffectRegistry,
    errors: &mut Vec<Error>,
) -> (Option<EffectId>, Option<EffectId>) {
    let within = Some(def.name.as_str());
    let mut push = |result: Result<(), Error>| {
        if let Err(error) = result {
            errors.push(error);
        }
    };
    let cost = def.cost.as_ref().and_then(|cost| {
        let Some(id) = effects.id(cost.as_str()) else {
            push(Err(Error::unknown::<AnyEffect>(cost.as_str(), within)));
            return None;
        };
        if effects
            .get(id)
            .is_some_and(|effect| !matches!(effect.duration, EffectDuration::Instant))
        {
            push(Err(Error::invalid(
                format!("a cost of `{cost}`: it isn't instant"),
                within,
            )));
        }
        Some(id)
    });
    let cooldown = def.cooldown.as_ref().and_then(|cooldown| {
        let Some(id) = effects.id(cooldown.as_str()) else {
            push(Err(Error::unknown::<AnyEffect>(cooldown.as_str(), within)));
            return None;
        };
        if effects.get(id).is_some_and(|effect| {
            matches!(effect.duration, EffectDuration::Instant) || effect.owner_tags.is_empty()
        }) {
            push(Err(Error::invalid(
                format!("a cooldown of `{cooldown}`: it has to last, and grant a tag"),
                within,
            )));
        }
        Some(id)
    });
    if def.net == NetExecution::LocalOnly && (def.cost.is_some() || def.cooldown.is_some()) {
        push(Err(Error::invalid(
            "a cost or a cooldown on an ability run locally only: a server's values would undo \
             what it paid"
                .to_owned(),
            within,
        )));
    }
    (cost, cooldown)
}
