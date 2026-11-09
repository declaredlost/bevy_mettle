//! The abilities there are, checked.

use std::any::TypeId;
use std::sync::Arc;

use bevy::platform::collections::{HashMap, HashSet};
use bevy::prelude::*;
use bevy_mettle_core::tag::TagRegistry;
use bevy_mettle_core::{MettleError, MettleErrors, Registry};
use bevy_mettle_effect::data::{AnyEffect, EffectDuration};
use bevy_mettle_effect::{EffectId, EffectRegistry};

use crate::ability::Ability;
use crate::behaviour::{AnyBehaviour, BehaviourRegistry};
use crate::data::{AbilityData, AbilityDef, AnyAbility, Grant, Trigger};

bevy_mettle_core::number! {
    /// An ability's number in the [`AbilityRegistry`]: what a granted ability,
    /// a request, a replica or an event knows it by.
    ///
    /// Numbered in order of name as content is checked, so the same content is
    /// numbered the same on a server and its clients; a reload keeps the
    /// numbers abilities had.
    pub struct AbilityId;
}

/// The abilities there are, checked against the tags, effects and behaviours,
/// and numbered.
///
/// Made by checking their definitions, and what the effects grant. Each
/// ability is numbered, an [`AbilityId`], in order of name. A resource: in an
/// app with `bevy_mettle`, its content loads it, and granted abilities take the
/// new definitions.
///
/// An app's registry also holds the ability types code names
/// ([`add_ability`](crate::AbilityApp::add_ability)), which content has to
/// define. So an app's content is checked by [`recheck`](Self::recheck)ing its
/// registry, which keeps those types and the numbers abilities had: what a
/// load or a hot reload does. [`check`](Self::check) makes a registry from
/// nothing, knowing no types: for tools and tests outside an app. Inserted
/// into an app, it would forget what code names.
///
/// Read as a [`Registry`]: `get(id)`, `id(name)`, `id_of::<A>()`, `name(id)`,
/// `iter()`.
#[derive(Resource, Deref, Clone, Default)]
pub struct AbilityRegistry {
    #[deref]
    registry: Registry<AbilityId, Arc<AbilityDef>>,
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

/// What abilities, and what effects grant, are checked against: what they
/// can name.
#[derive(Clone, Copy)]
pub struct AbilityChecks<'a> {
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
    /// names. For tools and tests; an app's content is
    /// [`recheck`](Self::recheck)ed.
    ///
    /// # Errors
    ///
    /// Everything wrong, at once: abilities defined twice; tags, effects and
    /// behaviours named that aren't; a cost that isn't instant, a cooldown that
    /// doesn't last or grants nothing; and grants of effects that aren't, or
    /// don't last, or of abilities that aren't.
    pub fn check(
        definitions: &[AbilityData],
        grants: &[Grant],
        against: AbilityChecks<'_>,
    ) -> Result<Self, MettleErrors> {
        Self::default().recheck(definitions, grants, against)
    }

    /// The registry of `definitions` and `grants`, as [`check`](Self::check)
    /// makes it, each ability here keeping its number: new ones numbered after
    /// the last, in order of name. What a reload does. The ability types code
    /// names here are kept.
    ///
    /// # Errors
    ///
    /// As `check`'s, and an ability code names that content doesn't define.
    pub fn recheck(
        &self,
        definitions: &[AbilityData],
        grants: &[Grant],
        against: AbilityChecks<'_>,
    ) -> Result<Self, MettleErrors> {
        let AbilityChecks {
            tags,
            effects,
            behaviours,
        } = against;
        let mut errors = Vec::new();
        let mut names: HashSet<&str> = HashSet::default();
        for def in definitions {
            if !names.insert(def.id.as_str()) {
                errors.push(MettleError::Duplicate {
                    kind: "ability",
                    id: def.id.to_string(),
                });
            }
        }
        let mut names: Vec<&str> = names.into_iter().collect();
        names.sort_unstable();
        let mut registry = self.registry.renumbered(names);
        errors.extend(registry.unnumbered().map(|name| MettleError::NotInContent {
            kind: "ability",
            id: name.to_owned(),
        }));
        let mut compiled = Vec::new();
        for def in definitions {
            let Some(id) = registry.id(def.id.as_str()) else {
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
                    None => errors.push(MettleError::unknown::<AnyAbility>(
                        ability.as_str(),
                        Some(within),
                    )),
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
            Err(MettleErrors(errors))
        }
    }

    /// Notes that code names the ability `A`: content has to define it.
    pub fn name_type<A: Ability>(&mut self) {
        self.registry.name_type(TypeId::of::<A>(), A::NAME);
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
    errors: &mut Vec<MettleError>,
) -> Option<EffectId> {
    let within = grant.effect.as_str();
    let Some(id) = effects.id(within) else {
        errors.push(MettleError::unknown::<AnyEffect>(within, Some(within)));
        return None;
    };
    let lasts = effects
        .get(id)
        .is_some_and(|effect| !matches!(effect.duration, EffectDuration::Instant));
    if !lasts {
        errors.push(MettleError::Invalid {
            what: format!(
                "a grant of `{}`: it's instant, so it grants nothing for any while",
                grant.effect
            ),
            within: None,
        });
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
    errors: &mut Vec<MettleError>,
) -> Option<AbilityDef> {
    let before = errors.len();
    let within = Some(def.id.as_str());
    let invalid = |what: String| MettleError::Invalid {
        what,
        within: within.map(ToOwned::to_owned),
    };
    let mut push = |result: Result<(), MettleError>| {
        if let Err(error) = result {
            errors.push(error);
        }
    };
    for set in [
        &def.tags,
        &def.blocked_by,
        &def.cancels,
        &def.blocks,
        &def.grants,
        &def.cues,
    ] {
        push(tags.known_all(set.written(), within));
    }
    for condition in [&def.require, &def.cancel_if].into_iter().flatten() {
        push(condition.check(tags, within));
    }
    for trigger in &def.triggers {
        let (Trigger::Event(tag) | Trigger::TagAdded(tag) | Trigger::TagPresent(tag)) = trigger;
        push(tags.known(*tag, within));
    }
    let behaviour = behaviours.id(def.behaviour.as_str());
    match behaviour {
        None => push(Err(MettleError::unknown::<AnyBehaviour>(
            def.behaviour.as_str(),
            within,
        ))),
        Some(behaviour) => {
            if let Some(input) = behaviours.input(behaviour)
                && !def.triggers.is_empty()
            {
                push(Err(invalid(format!(
                    "triggers on an ability whose behaviour takes {input}: nothing triggering it \
                     gives it"
                ))));
            }
        }
    }
    let cost = def.cost.as_ref().and_then(|cost| {
        let Some(id) = effects.id(cost.as_str()) else {
            push(Err(MettleError::unknown::<AnyEffect>(
                cost.as_str(),
                within,
            )));
            return None;
        };
        if effects
            .get(id)
            .is_some_and(|effect| !matches!(effect.duration, EffectDuration::Instant))
        {
            push(Err(invalid(format!(
                "a cost of `{cost}`: it isn't instant"
            ))));
        }
        Some(id)
    });
    let cooldown = def.cooldown.as_ref().and_then(|cooldown| {
        let Some(id) = effects.id(cooldown.as_str()) else {
            push(Err(MettleError::unknown::<AnyEffect>(
                cooldown.as_str(),
                within,
            )));
            return None;
        };
        if effects.get(id).is_some_and(|effect| {
            matches!(effect.duration, EffectDuration::Instant) || effect.grants.is_empty()
        }) {
            push(Err(invalid(format!(
                "a cooldown of `{cooldown}`: it has to last, and grant a tag"
            ))));
        }
        Some(id)
    });
    if errors.len() > before {
        return None;
    }
    Some(AbilityDef {
        id,
        name: def.id.clone(),
        tags: def.tags.clone(),
        behaviour: behaviour?,
        cost,
        cooldown,
        require: def.require.clone(),
        cancel_if: def.cancel_if.clone(),
        blocked_by: def.blocked_by.clone(),
        cancels: def.cancels.clone(),
        blocks: def.blocks.clone(),
        grants: def.grants.clone(),
        cues: def.cues.clone(),
        triggers: def.triggers.clone(),
        commit: def.commit,
    })
}
