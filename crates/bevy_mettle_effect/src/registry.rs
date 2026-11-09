//! The effects there are, checked.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_attribute::modifier::Operation;
use bevy_mettle_attribute::registry::{AnyAttribute, AttributeRegistry};
use bevy_mettle_core::curve::CurveTable;
use bevy_mettle_core::tag::TagTree;
use bevy_mettle_core::{Id, MettleError, MettleErrors};

use crate::code::{Calculation, CalculationFn, EffectCode, Execution, ExecutionFn, RouterFn};
use crate::data::{Effect, EffectDuration, EffectModifier, Magnitude};

/// The effects there are, checked against the tags, curves and effect code.
///
/// Made only by [`check`](Self::check)ing their definitions: every name they
/// use defined, every number one that can be. A resource: in an app with
/// `bevy_mettle`, its content loads it; insert another (a hot reload, a mod)
/// and active effects take their new definitions.
#[derive(Resource, Clone, Default)]
pub struct EffectRegistry {
    effects: Arc<HashMap<Id<Effect>, Arc<Effect>>>,
    curves: CurveTable,
    code: Arc<EffectCode>,
}

impl std::fmt::Debug for EffectRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut effects: Vec<&Id<Effect>> = self.effects.keys().collect();
        effects.sort();
        f.debug_struct("EffectRegistry")
            .field("effects", &effects)
            .field(
                "executions",
                &self.code.executions.keys().collect::<Vec<_>>(),
            )
            .field(
                "calculations",
                &self.code.calculations.keys().collect::<Vec<_>>(),
            )
            .field(
                "meta attributes",
                &self.code.routers.keys().collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

/// What effects are checked against: what they can name.
#[derive(Clone, Copy)]
pub struct EffectChecks<'a> {
    /// The gameplay tags there are.
    pub tags: &'a TagTree,
    /// The curves magnitudes scale by.
    pub curves: &'a CurveTable,
    /// The attributes the app added.
    pub attributes: &'a AttributeRegistry,
    /// The executions and calculations the app added.
    pub code: &'a EffectCode,
}

impl EffectRegistry {
    /// The registry of `definitions`, checked against what they can name: the
    /// only way there is one.
    ///
    /// # Errors
    ///
    /// Everything wrong with them, at once: effects defined twice; tags,
    /// curves, attributes, effects, executions and calculations named that
    /// aren't; numbers that can't be; what an instant effect can't have; meta
    /// attributes read, dealt other than flat, or held by a lasting effect;
    /// and a meta attribute sharing an attribute's name.
    pub fn check(definitions: &[Effect], against: EffectChecks<'_>) -> Result<Self, MettleErrors> {
        let EffectChecks {
            tags,
            curves,
            attributes,
            code,
        } = against;
        let mut errors = Vec::new();
        let mut effects = HashMap::new();
        for def in definitions {
            if effects
                .insert(def.id.clone(), Arc::new(def.clone()))
                .is_some()
            {
                errors.push(MettleError::Duplicate {
                    kind: "effect",
                    id: def.id.to_string(),
                });
            }
        }
        for meta in code.routers.keys() {
            if attributes.get(meta).is_some() {
                errors.push(MettleError::Invalid {
                    what: format!("an attribute and a meta attribute both named `{meta}`"),
                    within: None,
                });
            }
        }
        let registry = Self {
            effects: Arc::new(effects),
            curves: curves.clone(),
            code: Arc::new(code.clone()),
        };
        for def in registry.effects.values() {
            registry.check_def(def, tags, attributes, &mut errors);
        }
        errors.sort_by_key(ToString::to_string);
        if errors.is_empty() {
            Ok(registry)
        } else {
            Err(MettleErrors(errors))
        }
    }

    /// The effect called `id`, if there is one.
    #[must_use]
    pub fn get(&self, id: &Id<Effect>) -> Option<&Arc<Effect>> {
        self.effects.get(id)
    }

    /// Every effect.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<Effect>> {
        self.effects.values()
    }

    pub(crate) fn curves(&self) -> &CurveTable {
        &self.curves
    }

    pub(crate) fn execution(&self, id: &Id<Execution>) -> Option<&ExecutionFn> {
        self.code.executions.get(id)
    }

    /// The router of the meta attribute `id`, if it's one.
    pub(crate) fn router(&self, id: &Id<AnyAttribute>) -> Option<&RouterFn> {
        self.code.routers.get(id)
    }

    /// Checks `attribute` is one actors have, whose value can be read: not a
    /// meta attribute, which has none.
    fn readable(
        &self,
        attribute: &Id<AnyAttribute>,
        attributes: &AttributeRegistry,
        within: Option<&str>,
    ) -> Result<(), MettleError> {
        if self.router(attribute).is_some() {
            return Err(MettleError::Invalid {
                what: format!("reading meta attribute `{attribute}`: it has no value"),
                within: within.map(ToOwned::to_owned),
            });
        }
        attributes.check(attribute, within)
    }

    pub(crate) fn calculation(&self, id: &Id<Calculation>) -> Option<&CalculationFn> {
        self.code.calculations.get(id)
    }

    /// Checks `def`, noting what's wrong in `errors`.
    fn check_def(
        &self,
        def: &Effect,
        tags: &TagTree,
        attributes: &AttributeRegistry,
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
            &def.grants,
            &def.cues,
            &def.removes,
            &def.immunity,
        ] {
            push(tags.known_all(set.written(), within));
        }
        for condition in [&def.require, &def.ongoing, &def.remove_if]
            .into_iter()
            .flatten()
        {
            push(condition.check(tags, within));
        }
        if let EffectDuration::For(seconds) = &def.duration {
            push(self.positive(seconds, "a duration", tags, attributes, within));
        }
        if let Some(period) = &def.period {
            push(self.positive(&period.every, "a period", tags, attributes, within));
        }
        for modifier in &def.modifiers {
            self.check_modifier(def, modifier, tags, attributes, &mut push);
        }
        for execution in &def.executions {
            if self.execution(execution).is_none() {
                push(Err(MettleError::unknown::<Execution>(
                    execution.as_str(),
                    within,
                )));
            }
        }
        let chained = def
            .on_apply
            .iter()
            .chain(&def.on_remove)
            .chain(&def.on_expire)
            .chain(def.stacking.iter().flat_map(|stacking| &stacking.overflow));
        for effect in chained {
            if self.get(effect).is_none() {
                push(Err(MettleError::unknown::<Effect>(effect.as_str(), within)));
            }
        }
        if let Some(stacking) = &def.stacking
            && stacking.limit == 0
        {
            push(Err(invalid("a stacking limit of 0".to_owned())));
        }
        if !def.lasts() {
            let lasting = [
                ("a period", def.period.is_some()),
                ("granted tags", !def.grants.is_empty()),
                ("immunity", !def.immunity.is_empty()),
                ("an ongoing requirement", def.ongoing.is_some()),
                ("a removal requirement", def.remove_if.is_some()),
                ("stacking", def.stacking.is_some()),
                ("effects on removal", !def.on_remove.is_empty()),
                ("effects on expiry", !def.on_expire.is_empty()),
            ];
            for (what, has) in lasting {
                if has {
                    push(Err(invalid(format!(
                        "an instant effect with {what}: it doesn't last"
                    ))));
                }
            }
        } else if def.period.is_none() && !def.executions.is_empty() {
            // A lasting effect executes only as its period comes round.
            push(Err(invalid(
                "executions on a lasting effect with no period: it never executes".to_owned(),
            )));
        }
    }

    /// Checks `def`'s `modifier`, pushing what's wrong to `push`.
    fn check_modifier(
        &self,
        def: &Effect,
        modifier: &EffectModifier,
        tags: &TagTree,
        attributes: &AttributeRegistry,
        push: &mut impl FnMut(Result<(), MettleError>),
    ) {
        let within = Some(def.id.as_str());
        let invalid = |what: String| MettleError::Invalid {
            what,
            within: within.map(ToOwned::to_owned),
        };
        if self.router(&modifier.attribute).is_some() {
            if modifier.op != Operation::Flat {
                push(Err(invalid(format!(
                    "an {:?} modifier of meta attribute `{}`: it's dealt flat amounts only",
                    modifier.op, modifier.attribute
                ))));
            }
            if def.lasts() && def.period.is_none() {
                push(Err(invalid(format!(
                    "meta attribute `{}` on a lasting effect with no period: it's routed only as \
                     an effect executes",
                    modifier.attribute
                ))));
            }
        } else {
            push(attributes.check(&modifier.attribute, within));
        }
        push(self.magnitude(&modifier.value, tags, attributes, within));
        if let Some(condition) = &modifier.condition {
            push(condition.check(tags, within));
        }
        if let Some(per) = &modifier.per {
            push(self.readable(&per.attribute, attributes, within));
            if !(per.every.is_finite() && per.every > 0.0) {
                push(Err(invalid(format!(
                    "a modifier per {} {:?}",
                    per.every, per.attribute
                ))));
            }
        }
    }

    /// Checks `magnitude` can be worked out.
    fn magnitude(
        &self,
        magnitude: &Magnitude,
        tags: &TagTree,
        attributes: &AttributeRegistry,
        within: Option<&str>,
    ) -> Result<(), MettleError> {
        if let Some(attribute) = magnitude.attribute() {
            self.readable(attribute, attributes, within)?;
        }
        let invalid = |what: String| MettleError::Invalid {
            what,
            within: within.map(ToOwned::to_owned),
        };
        match magnitude {
            Magnitude::Value(value) if !value.is_finite() => {
                Err(invalid(format!("a value of {value}")))
            }
            Magnitude::Scaled { base, curve } => {
                self.curves.known(curve, within)?;
                if base.is_finite() {
                    Ok(())
                } else {
                    Err(invalid(format!("a base of {base}")))
                }
            }
            Magnitude::Attribute { coefficient, .. } if !coefficient.is_finite() => {
                Err(invalid(format!("a coefficient of {coefficient}")))
            }
            Magnitude::Param(param) => tags.known(param, within),
            Magnitude::Custom(calculation) if self.calculation(calculation).is_none() => Err(
                MettleError::unknown::<Calculation>(calculation.as_str(), within),
            ),
            _ => Ok(()),
        }
    }

    /// Checks `magnitude` can be worked out, and is more than 0 if it's a
    /// value.
    fn positive(
        &self,
        magnitude: &Magnitude,
        what: &str,
        tags: &TagTree,
        attributes: &AttributeRegistry,
        within: Option<&str>,
    ) -> Result<(), MettleError> {
        self.magnitude(magnitude, tags, attributes, within)?;
        match magnitude {
            Magnitude::Value(value) if *value <= 0.0 => Err(MettleError::Invalid {
                what: format!("{what} of {value}: it has to be more than 0"),
                within: within.map(ToOwned::to_owned),
            }),
            _ => Ok(()),
        }
    }
}
