//! The abilities there are, checked.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_core::tag::TagTree;
use bevy_mettle_core::{Id, MettleError, MettleErrors};
use bevy_mettle_effect::{Effect, EffectDuration, EffectRegistry};

use crate::{Ability, Behaviour, Behaviours, Grant, Trigger};

/// The abilities there are, built from their definitions and checked
/// against the tag tree, the effects and the behaviours there are; and
/// what the effects grant checked against them. Insert it as a resource.
#[derive(Resource, Clone, Default)]
pub struct AbilityRegistry {
    abilities: Arc<HashMap<Id<Ability>, Arc<Ability>>>,
    /// The abilities each effect grants.
    grants: Arc<HashMap<Id<Effect>, Vec<Id<Ability>>>>,
}

impl std::fmt::Debug for AbilityRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut abilities: Vec<&Id<Ability>> = self.abilities.keys().collect();
        abilities.sort();
        let mut grants: Vec<(&Id<Effect>, &Vec<Id<Ability>>)> = self.grants.iter().collect();
        grants.sort();
        f.debug_struct("AbilityRegistry")
            .field("abilities", &abilities)
            .field("grants", &grants)
            .finish()
    }
}

impl AbilityRegistry {
    /// Checks `definitions`, and what effects `grants`, against `tags`,
    /// `effects` and `behaviours`.
    ///
    /// # Errors
    ///
    /// Everything wrong, at once: abilities defined twice; tags, effects
    /// and behaviours named that aren't; a cost that isn't instant, a
    /// cooldown that doesn't last or grants nothing; and grants of effects
    /// that aren't, or don't last, or of abilities that aren't.
    pub fn build<'a, 'g>(
        definitions: impl IntoIterator<Item = &'a Ability>,
        grants: impl IntoIterator<Item = &'g Grant>,
        tags: &TagTree,
        effects: &EffectRegistry,
        behaviours: &Behaviours,
    ) -> Result<Self, MettleErrors> {
        let mut errors = Vec::new();
        let mut abilities = HashMap::new();
        for def in definitions {
            if abilities
                .insert(def.id.clone(), Arc::new(def.clone()))
                .is_some()
            {
                errors.push(MettleError::Duplicate {
                    kind: "ability",
                    id: def.id.to_string(),
                });
            }
        }
        for def in abilities.values() {
            check(def, tags, effects, behaviours, &mut errors);
        }
        let mut granted: HashMap<Id<Effect>, Vec<Id<Ability>>> = HashMap::new();
        for grant in grants {
            check_grant(grant, &abilities, effects, &mut errors);
            granted
                .entry(grant.effect.clone())
                .or_default()
                .extend(grant.abilities.iter().cloned());
        }
        errors.sort_by_key(ToString::to_string);
        if errors.is_empty() {
            Ok(Self {
                abilities: Arc::new(abilities),
                grants: Arc::new(granted),
            })
        } else {
            Err(MettleErrors(errors))
        }
    }

    /// The ability called `id`, if there is one.
    #[must_use]
    pub fn get(&self, id: &Id<Ability>) -> Option<&Arc<Ability>> {
        self.abilities.get(id)
    }

    /// Every ability.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<Ability>> {
        self.abilities.values()
    }

    /// The abilities `effect` grants while it lasts.
    #[must_use]
    pub fn granted_by(&self, effect: &Id<Effect>) -> &[Id<Ability>] {
        self.grants.get(effect).map_or(&[], Vec::as_slice)
    }
}

/// Checks `grant`: its effect is one there is, and lasts; its abilities
/// are among `abilities`.
fn check_grant(
    grant: &Grant,
    abilities: &HashMap<Id<Ability>, Arc<Ability>>,
    effects: &EffectRegistry,
    errors: &mut Vec<MettleError>,
) {
    let within = grant.effect.as_str();
    match effects.get(&grant.effect) {
        None => errors.push(MettleError::unknown::<Effect>(
            grant.effect.as_str(),
            Some(within),
        )),
        Some(effect) if matches!(effect.duration, EffectDuration::Instant) => {
            errors.push(MettleError::Invalid {
                what: format!(
                    "a grant of `{}`: it's instant, so it grants nothing for any while",
                    grant.effect
                ),
                within: None,
            });
        }
        Some(_) => {}
    }
    for ability in &grant.abilities {
        if !abilities.contains_key(ability) {
            errors.push(MettleError::unknown::<Ability>(
                ability.as_str(),
                Some(within),
            ));
        }
    }
}

/// Checks `def`, noting what's wrong in `errors`.
fn check(
    def: &Ability,
    tags: &TagTree,
    effects: &EffectRegistry,
    behaviours: &Behaviours,
    errors: &mut Vec<MettleError>,
) {
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
    ] {
        push(tags.known_all(set.written(), within));
    }
    if let Some(require) = &def.require {
        push(require.check(tags, within));
    }
    for trigger in &def.triggers {
        let (Trigger::Event(tag) | Trigger::TagAdded(tag) | Trigger::TagPresent(tag)) = trigger;
        push(tags.known(tag, within));
    }
    if !behaviours.contains(&def.behaviour) {
        push(Err(MettleError::unknown::<Behaviour>(
            def.behaviour.as_str(),
            within,
        )));
    }
    if let Some(cost) = &def.cost {
        match effects.get(cost) {
            None => push(Err(MettleError::unknown::<Effect>(cost.as_str(), within))),
            Some(effect) if !matches!(effect.duration, EffectDuration::Instant) => {
                push(Err(invalid(format!(
                    "a cost of `{cost}`: it isn't instant"
                ))));
            }
            Some(_) => {}
        }
    }
    if let Some(cooldown) = &def.cooldown {
        match effects.get(cooldown) {
            None => push(Err(MettleError::unknown::<Effect>(
                cooldown.as_str(),
                within,
            ))),
            Some(effect)
                if matches!(effect.duration, EffectDuration::Instant)
                    || effect.grants.is_empty() =>
            {
                push(Err(invalid(format!(
                    "a cooldown of `{cooldown}`: it has to last, and grant a tag"
                ))));
            }
            Some(_) => {}
        }
    }
}
