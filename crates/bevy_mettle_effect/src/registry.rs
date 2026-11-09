//! The effects there are, checked: their data compiled to what the app runs.

use std::any::TypeId;
use std::sync::Arc;

use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy_mettle_attribute::modifier::{Operation, Per};
use bevy_mettle_attribute::registry::AttributeRegistry;
use bevy_mettle_core::curve::CurveRegistry;
use bevy_mettle_core::registry::{Key, Name, Registry};
use bevy_mettle_core::tag::TagRegistry;
use bevy_mettle_core::{Error, Errors};

use crate::Effect;
use crate::code::{
    AnyRoute, CalculationFn, CalculationId, CalculationRegistry, Changes, ExecutionFn, ExecutionId,
    ExecutionRegistry, RouteId, RouteRegistry,
};
use crate::data::{
    AnyEffect, EffectData, EffectDef, EffectDuration, EffectModifier, Magnitude, MagnitudeDef,
    ModifierDef, Period, Stacking,
};

/// An effect's number in the [`EffectRegistry`], as the app knows it.
///
/// What an active effect, a replica, a chain or an event knows it by, as Bevy
/// knows a component by its `ComponentId`.
///
/// Numbered in order of name as content is checked, so the same content is
/// numbered the same on a server and its clients; a reload keeps the
/// numbers effects had.
pub type EffectId = Key<AnyEffect>;

/// The effects there are, checked against what they name, and compiled.
///
/// Checked against the tags, curves, attributes and effect code, compiled to
/// [`EffectDef`]s, and numbered.
///
/// Made by checking their definitions: every name they use defined, every
/// number one that can be. Each effect is numbered, an [`EffectId`], in order
/// of name. A resource: in an app with `bevy_mettle`, its content loads it, and
/// active effects take the new definitions.
///
/// An app's registry also holds the effect types code names
/// ([`add_effect`](crate::code::EffectApp::add_effect)), which content has to
/// define. So an app's content is checked by [`recheck`](Self::recheck)ing its
/// registry, which keeps those types and the numbers effects had: what a load
/// or a hot reload does. [`check`](Self::check) makes a registry from nothing,
/// knowing no types: for tools and tests outside an app. Inserted into an app,
/// it would forget what code names.
///
/// Read as a [`Registry`]: `get(id)`, `id(name)`, `id_of::<E>()`, `name(id)`,
/// `iter()`.
#[derive(Resource, Deref, Clone, Default)]
pub struct EffectRegistry {
    #[deref]
    registry: Registry<AnyEffect, Arc<EffectDef>>,
    curves: CurveRegistry,
    executions: ExecutionRegistry,
    calculations: CalculationRegistry,
    routes: RouteRegistry,
}

impl std::fmt::Debug for EffectRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EffectRegistry")
            .field("effects", &self.registry)
            .field("executions", &self.executions)
            .field("calculations", &self.calculations)
            .field("routes", &self.routes)
            .finish_non_exhaustive()
    }
}

/// What effects are checked against: the tags, curves and code they can name.
#[derive(Clone, Copy)]
pub struct EffectChecks<'a> {
    /// The gameplay tags there are.
    pub tags: &'a TagRegistry,
    /// The curves magnitudes scale by.
    pub curves: &'a CurveRegistry,
    /// The attributes the app added.
    pub attributes: &'a AttributeRegistry,
    /// The executions the app added.
    pub executions: &'a ExecutionRegistry,
    /// The calculations the app added.
    pub calculations: &'a CalculationRegistry,
    /// The routes the app added.
    pub routes: &'a RouteRegistry,
}

/// What compiling one effect reads: what it can name, and where errors go.
struct Compile<'a> {
    registry: &'a Registry<AnyEffect, Arc<EffectDef>>,
    checks: EffectChecks<'a>,
    errors: &'a mut Vec<Error>,
}

impl EffectRegistry {
    /// A registry of `definitions`, from nothing, checked against what they
    /// can name and compiled: knowing no effect types code names. For tools
    /// and tests; an app's content is [`recheck`](Self::recheck)ed.
    ///
    /// # Errors
    ///
    /// Everything wrong with them, at once: effects defined twice; tags,
    /// curves, attributes, routes, effects, executions and calculations named
    /// that aren't; numbers that can't be; what an instant effect can't have;
    /// and routes dealt other than flat, or by a lasting effect.
    pub fn check(definitions: &[EffectData], against: EffectChecks<'_>) -> Result<Self, Errors> {
        Self::default().recheck(definitions, against)
    }

    /// The registry of `definitions`, as [`check`](Self::check) makes it, each
    /// effect here keeping its number: new ones numbered after the last, in
    /// order of name. What a reload does, so what holds an effect's number
    /// still names it. The effect types code names here are kept.
    ///
    /// # Errors
    ///
    /// As `check`'s, and an effect code names that content doesn't define.
    pub fn recheck(
        &self,
        definitions: &[EffectData],
        against: EffectChecks<'_>,
    ) -> Result<Self, Errors> {
        let mut errors = Vec::new();
        let mut names: HashSet<&str> = HashSet::default();
        for def in definitions {
            if !names.insert(def.name.as_str()) {
                errors.push(Error::Duplicate {
                    kind: "effect",
                    name: def.name.to_string(),
                });
            }
        }
        let mut names: Vec<&str> = names.into_iter().collect();
        names.sort_unstable();
        let mut registry = self.registry.renumbered(names);
        errors.extend(registry.unnumbered().map(|name| Error::NotInContent {
            kind: "effect",
            name: name.to_owned(),
        }));
        let mut compiled = Vec::new();
        for def in definitions {
            let mut compile = Compile {
                registry: &registry,
                checks: against,
                errors: &mut errors,
            };
            if let Some(effect) = compile.effect(def) {
                compiled.push(effect);
            }
        }
        for effect in compiled {
            registry.set(effect.id, Arc::new(effect));
        }
        errors.sort_by_key(ToString::to_string);
        if errors.is_empty() {
            Ok(Self {
                registry,
                curves: against.curves.clone(),
                executions: against.executions.clone(),
                calculations: against.calculations.clone(),
                routes: against.routes.clone(),
            })
        } else {
            Err(Errors(errors))
        }
    }

    /// Notes that code names the effect `E`: content has to define it.
    pub fn name_type<E: Effect>(&mut self) {
        self.registry.name_type(TypeId::of::<E>(), E::NAME);
    }

    pub(crate) fn curves(&self) -> &CurveRegistry {
        &self.curves
    }

    pub(crate) fn execution(&self, id: ExecutionId) -> Option<&ExecutionFn> {
        self.executions.get(id)
    }

    pub(crate) fn calculation(&self, id: CalculationId) -> Option<&CalculationFn> {
        self.calculations.get(id)
    }

    pub(crate) fn routes(&self) -> &RouteRegistry {
        &self.routes
    }
}

impl Compile<'_> {
    /// `def` compiled, or what's wrong with it noted.
    fn effect(&mut self, def: &EffectData) -> Option<EffectDef> {
        let within = def.name.as_str().to_owned();
        let errors_before = self.errors.len();
        for set in [
            &def.tags,
            &def.grants,
            &def.cues,
            &def.removes,
            &def.immunity,
        ] {
            self.push(self.checks.tags.known_all(set.written(), Some(&within)));
        }
        for condition in [&def.require, &def.ongoing, &def.remove_if]
            .into_iter()
            .flatten()
        {
            self.push(condition.check(self.checks.tags, Some(&within)));
        }
        let timing = self.timing(def, &within);
        let modifiers: Vec<Option<ModifierDef>> = def
            .modifiers
            .iter()
            .map(|modifier| self.modifier(def, modifier, &within))
            .collect();
        let executions: Vec<Option<ExecutionId>> = def
            .executions
            .iter()
            .map(|execution| {
                let id = self.checks.executions.resolve(execution, Some(&within));
                self.held(id)
            })
            .collect();
        let on_apply = self.effects(&def.on_apply, &within);
        let on_remove = self.effects(&def.on_remove, &within);
        let on_expire = self.effects(&def.on_expire, &within);
        let stacking = def.stacking.as_ref().map(|stacking| Stacking {
            by: stacking.by,
            limit: stacking.limit,
            reapply: stacking.reapply,
            reset_period: stacking.reset_period,
            expire: stacking.expire,
            overflow: self.effects(&stacking.overflow, &within),
        });
        self.lasting(def, &within);
        if self.errors.len() > errors_before {
            return None;
        }
        let (duration, period) = timing?;
        Some(EffectDef {
            id: self.registry.id(def.name.as_str())?,
            name: def.name.clone(),
            tags: def.tags.clone(),
            duration,
            period,
            modifiers: modifiers.into_iter().collect::<Option<_>>()?,
            executions: executions.into_iter().collect::<Option<_>>()?,
            grants: def.grants.clone(),
            cues: def.cues.clone(),
            require: def.require.clone(),
            ongoing: def.ongoing.clone(),
            remove_if: def.remove_if.clone(),
            removes: def.removes.clone(),
            immunity: def.immunity.clone(),
            stacking,
            on_apply,
            on_remove,
            on_expire,
        })
    }

    /// The numbers of the effects `names` names, those there are; what's
    /// wrong noted.
    fn effects(&mut self, names: &[Name<AnyEffect>], within: &str) -> Vec<EffectId> {
        names
            .iter()
            .filter_map(|name| {
                let id = self.registry.id(name.as_str());
                if id.is_none() {
                    self.push(Err(Error::unknown::<AnyEffect>(
                        name.as_str(),
                        Some(within),
                    )));
                }
                id
            })
            .collect()
    }

    /// How long `def` lasts and how often it executes, compiled, or what's
    /// wrong with them noted.
    fn timing(
        &mut self,
        def: &EffectData,
        within: &str,
    ) -> Option<(EffectDuration<MagnitudeDef>, Option<Period<MagnitudeDef>>)> {
        let duration = match &def.duration {
            EffectDuration::Instant => Some(EffectDuration::Instant),
            EffectDuration::Infinite => Some(EffectDuration::Infinite),
            EffectDuration::For(seconds) => self
                .positive(seconds, "a duration", within)
                .map(EffectDuration::For),
        };
        // Both worked out, so what's wrong with either is noted.
        let period = match &def.period {
            None => Ok(None),
            Some(period) => self
                .positive(&period.every, "a period", within)
                .map(|every| {
                    Some(Period {
                        every,
                        on_apply: period.on_apply,
                    })
                })
                .ok_or(()),
        };
        Some((duration?, period.ok()?))
    }

    /// Notes what `def` can't have for how long it lasts: what an instant one
    /// can't, executions a lasting one never runs, and a stacking limit of 0.
    fn lasting(&mut self, def: &EffectData, within: &str) {
        if let Some(stacking) = &def.stacking
            && stacking.limit == 0
        {
            self.invalid("a stacking limit of 0".to_owned(), within);
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
                    self.invalid(
                        format!("an instant effect with {what}: it doesn't last"),
                        within,
                    );
                }
            }
        } else if def.period.is_none() && !def.executions.is_empty() {
            // A lasting effect executes only as its period comes round.
            self.invalid(
                "executions on a lasting effect with no period: it never executes".to_owned(),
                within,
            );
        }
    }

    /// `def`'s `modifier` compiled, or what's wrong with it noted.
    fn modifier(
        &mut self,
        def: &EffectData,
        modifier: &EffectModifier,
        within: &str,
    ) -> Option<ModifierDef> {
        let changes = match (&modifier.attribute, &modifier.route) {
            (Some(attribute), None) => self.attribute(attribute, within).map(Changes::Attribute),
            (None, Some(route)) => {
                if modifier.op != Operation::Flat {
                    self.invalid(
                        format!(
                            "an {:?} modifier of route `{route}`: it's dealt flat amounts only",
                            modifier.op
                        ),
                        within,
                    );
                }
                if def.lasts() && def.period.is_none() {
                    self.invalid(
                        format!(
                            "route `{route}` on a lasting effect with no period: it's routed only \
                             as an effect executes"
                        ),
                        within,
                    );
                }
                self.route(route, within).map(Changes::Route)
            }
            (Some(_), Some(_)) => {
                self.invalid(
                    "a modifier of both an attribute and a route: it's one or the other".to_owned(),
                    within,
                );
                None
            }
            (None, None) => {
                self.invalid(
                    "a modifier of neither an attribute nor a route".to_owned(),
                    within,
                );
                None
            }
        };
        let value = self.magnitude(&modifier.value, within);
        if let Some(condition) = &modifier.condition {
            self.push(condition.check(self.checks.tags, Some(within)));
        }
        let per = match &modifier.per {
            None => Some(None),
            Some(per) => {
                if !(per.every.is_finite() && per.every > 0.0) {
                    self.invalid(
                        format!("a modifier per {} `{}`", per.every, per.attribute),
                        within,
                    );
                }
                self.attribute(&per.attribute, within).map(|attribute| {
                    Some(Per {
                        attribute,
                        every: per.every,
                    })
                })
            }
        };
        Some(ModifierDef {
            changes: changes?,
            op: modifier.op,
            value: value?,
            condition: modifier.condition.clone(),
            per: per?,
        })
    }

    /// `magnitude` compiled, or what's wrong with it noted.
    fn magnitude(&mut self, magnitude: &Magnitude, within: &str) -> Option<MagnitudeDef> {
        match magnitude {
            Magnitude::Value(value) => {
                if !value.is_finite() {
                    self.invalid(format!("a value of {value}"), within);
                    return None;
                }
                Some(MagnitudeDef::Value(*value))
            }
            Magnitude::Scaled { base, curve } => {
                let curve = self.checks.curves.known(curve, Some(within));
                if !base.is_finite() {
                    self.invalid(format!("a base of {base}"), within);
                }
                match curve {
                    Err(error) => {
                        self.errors.push(error);
                        None
                    }
                    Ok(curve) => base
                        .is_finite()
                        .then_some(MagnitudeDef::Scaled { base: *base, curve }),
                }
            }
            Magnitude::Param(param) => {
                self.push(self.checks.tags.known(*param, Some(within)));
                Some(MagnitudeDef::Param(*param))
            }
            Magnitude::Attribute {
                attribute,
                from,
                coefficient,
            } => {
                if !coefficient.is_finite() {
                    self.invalid(format!("a coefficient of {coefficient}"), within);
                }
                let attribute = self.attribute(attribute, within)?;
                coefficient.is_finite().then_some(MagnitudeDef::Attribute {
                    attribute,
                    from: *from,
                    coefficient: *coefficient,
                })
            }
            Magnitude::Custom(calculation) => {
                let id = self.checks.calculations.resolve(calculation, Some(within));
                self.held(id).map(MagnitudeDef::Custom)
            }
        }
    }

    /// `magnitude` compiled, checked more than 0 if it's a value.
    fn positive(
        &mut self,
        magnitude: &Magnitude,
        what: &str,
        within: &str,
    ) -> Option<MagnitudeDef> {
        let compiled = self.magnitude(magnitude, within)?;
        if let MagnitudeDef::Value(value) = compiled
            && value <= 0.0
        {
            self.invalid(
                format!("{what} of {value}: it has to be more than 0"),
                within,
            );
            return None;
        }
        Some(compiled)
    }

    /// The attribute named `name`'s number, or that there's none noted.
    fn attribute(
        &mut self,
        name: &Name<bevy_mettle_attribute::registry::AnyAttribute>,
        within: &str,
    ) -> Option<bevy_mettle_attribute::registry::AttributeId> {
        let resolved = self.checks.attributes.resolve(name, Some(within));
        match resolved {
            Ok(id) => Some(id),
            Err(error) => {
                self.errors.push(error);
                None
            }
        }
    }

    /// The route named `name`'s number, or that there's none noted.
    fn route(&mut self, name: &Name<AnyRoute>, within: &str) -> Option<RouteId> {
        let id = self.checks.routes.resolve(name, Some(within));
        self.held(id)
    }

    /// What `result` holds, or its error noted.
    fn held<T>(&mut self, result: Result<T, Error>) -> Option<T> {
        result.map_err(|error| self.errors.push(error)).ok()
    }

    fn push(&mut self, result: Result<(), Error>) {
        if let Err(error) = result {
            self.errors.push(error);
        }
    }

    fn invalid(&mut self, what: String, within: &str) {
        self.errors.push(Error::Invalid {
            what,
            within: Some(within.to_owned()),
        });
    }
}
