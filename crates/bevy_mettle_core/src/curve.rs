//! Curves: values by level from content, sampled as Bevy's curves, numbered.

use std::sync::Arc;

use bevy_curve::{
    ConstantCurve, Curve as BevyCurve, EaseFunction, Interval, UnevenSampleAutoCurve,
    UnevenSampleCurve,
};
use bevy_ecs::resource::Resource;
use bevy_platform::collections::HashMap;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::error::{Error, Errors};
use crate::registry::{Key, Kind, Name, Registry};

/// How a curve goes between its keys: held, straight, or eased.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum Interpolation {
    /// Each key's value until the next.
    Constant,
    /// A straight line from each key to the next.
    #[default]
    Linear,
    /// From each key to the next as one of Bevy's easing functions goes from 0
    /// to 1: `Ease(SineInOut)`, `Ease(ExponentialIn)`.
    Ease(EaseFunction),
}

/// The curve kind: what a `Name<AnyCurve>` names and a [`CurveId`] numbers.
///
/// A curve's name in content is a `Name<AnyCurve>`, until it's resolved to a
/// [`CurveId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyCurve {}

/// A curve: a value by level, from keys `(level, value)` with levels rising.
///
/// What an effect's magnitude scales by, as Unreal's `ScalableFloat` does;
/// defined in data, and named by an `Name<AnyCurve>`.
///
/// ```
/// # use bevy_mettle_core::prelude::*;
/// # use bevy_curve::EaseFunction;
/// let damage = LevelCurve {
///     name: "fireball.damage".into(),
///     interpolation: Interpolation::Ease(EaseFunction::QuadraticIn),
///     keys: vec![(1.0, 10.0), (10.0, 85.0), (20.0, 240.0)],
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelCurve {
    /// Its name.
    pub name: Name<AnyCurve>,
    /// How it goes between keys: linearly, unless it says.
    #[serde(default)]
    pub interpolation: Interpolation,
    /// Its keys, `(level, value)`, levels rising. Before the first and past the
    /// last, it's as at that one.
    pub keys: Vec<(f32, f32)>,
}

/// A curve, built: one of Bevy's, over every level.
type Built = dyn BevyCurve<f32> + Send + Sync;

impl LevelCurve {
    /// It as one of Bevy's curves, its keys checked: kept as at its ends beyond
    /// them.
    fn build(&self) -> Result<Arc<Built>, &'static str> {
        let keys = &self.keys;
        if keys.is_empty() {
            return Err("with no keys");
        }
        if keys
            .iter()
            .any(|(level, value)| !level.is_finite() || !value.is_finite())
        {
            return Err("with a key that isn't finite");
        }
        if keys.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err("whose levels don't rise");
        }
        if let [(_, value)] = keys.as_slice() {
            return Ok(Arc::new(ConstantCurve::new(Interval::EVERYWHERE, *value)));
        }
        let keys = keys.iter().copied();
        let built: Arc<Built> = match self.interpolation {
            Interpolation::Linear => {
                Arc::new(UnevenSampleAutoCurve::new(keys).map_err(|_| "that can't be built")?)
            }
            Interpolation::Constant => Arc::new(
                UnevenSampleCurve::new(keys, |from: &f32, _: &f32, _: f32| *from)
                    .map_err(|_| "that can't be built")?,
            ),
            Interpolation::Ease(ease) => Arc::new(
                UnevenSampleCurve::new(keys, move |from: &f32, to: &f32, t: f32| {
                    from + (to - from) * ease.sample_clamped(t)
                })
                .map_err(|_| "that can't be built")?,
            ),
        };
        Ok(built)
    }
}

/// A curve's number in the [`CurveRegistry`]: what magnitudes scale by.
///
/// Magnitudes name a curve by number once content is checked.
pub type CurveId = Key<AnyCurve>;

/// The curves there are, checked, built as Bevy's curves, and numbered.
///
/// Each with keys, all finite, levels rising, and numbered, a [`CurveId`], in
/// order of name; [`rebuild`](Self::rebuild) keeps the numbers curves had.
/// Read as a [`Registry`] of each curve, built: `get(id)`, `id(name)`,
/// `name(id)`, `iter()`. Cheap to clone; empty by default. A resource: the
/// curves the app's content was checked against.
#[derive(Resource, Debug, Clone, Default)]
pub struct CurveRegistry(Registry<AnyCurve, Arc<Built>>);

impl CurveRegistry {
    /// Checks and builds `definitions`, numbered in order of name.
    ///
    /// # Errors
    ///
    /// Every curve defined twice, or whose keys can't be.
    pub fn build<'a>(
        definitions: impl IntoIterator<Item = &'a LevelCurve>,
    ) -> Result<Self, Errors> {
        Self::default().rebuild(definitions)
    }

    /// Checks and builds `definitions` as [`build`](Self::build) does, each
    /// curve here keeping its number: new ones numbered after the last, in
    /// order of name. What a reload does.
    ///
    /// # Errors
    ///
    /// As `build`'s.
    pub fn rebuild<'a>(
        &self,
        definitions: impl IntoIterator<Item = &'a LevelCurve>,
    ) -> Result<Self, Errors> {
        let mut errors = Vec::new();
        let mut built: HashMap<SmolStr, Arc<Built>> = HashMap::new();
        for def in definitions {
            match def.build() {
                Ok(curve) => {
                    if built
                        .insert(SmolStr::new(def.name.as_str()), curve)
                        .is_some()
                    {
                        errors.push(Error::Duplicate {
                            kind: AnyCurve::NAME,
                            name: def.name.to_string(),
                        });
                    }
                }
                Err(what) => errors.push(Error::Invalid {
                    what: format!("a curve {what}"),
                    within: Some(def.name.to_string()),
                }),
            }
        }
        if !errors.is_empty() {
            return Err(Errors(errors));
        }
        let mut names: Vec<SmolStr> = built.keys().cloned().collect();
        names.sort_unstable();
        let mut registry = self.0.renumbered(names);
        for (name, curve) in built {
            if let Some(id) = registry.id(&name) {
                registry.set(id, curve);
            }
        }
        Ok(Self(registry))
    }

    /// `curve`'s value at `level`, if there's such a curve.
    #[must_use]
    pub fn at(&self, curve: CurveId, level: f32) -> Option<f32> {
        self.curve(curve).map(|built| built.sample_clamped(level))
    }

    /// `curve`, as one of Bevy's curves: for graphs, tooling, and Bevy's curve
    /// adaptors.
    #[must_use]
    pub fn curve(&self, curve: CurveId) -> Option<&(dyn BevyCurve<f32> + Send + Sync)> {
        self.0.get(curve).map(AsRef::as_ref)
    }

    /// The number of the curve called `curve`, named within `within` if it's
    /// named in something.
    ///
    /// # Errors
    ///
    /// If there's no such curve.
    pub fn known(&self, curve: &Name<AnyCurve>, within: Option<&str>) -> Result<CurveId, Error> {
        self.id(curve.as_str())
            .ok_or_else(|| Error::unknown::<AnyCurve>(curve.as_str(), within))
    }
}

/// Read as the [`Registry`] of each curve, built, by number.
impl std::ops::Deref for CurveRegistry {
    type Target = Registry<AnyCurve, Arc<Built>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(interpolation: Interpolation) -> CurveRegistry {
        CurveRegistry::build(&[LevelCurve {
            name: Name::from_static("damage"),
            interpolation,
            keys: vec![(1.0, 10.0), (10.0, 100.0), (20.0, 300.0)],
        }])
        .unwrap()
    }

    fn at(table: &CurveRegistry, level: f32) -> f32 {
        table.at(table.id("damage").unwrap(), level).unwrap()
    }

    #[test]
    fn a_curve_goes_between_its_keys_as_it_says() {
        let linear = table(Interpolation::Linear);
        assert!((at(&linear, 5.5) - 55.0).abs() < 1e-4);
        assert!((at(&linear, 0.0) - 10.0).abs() < 1e-4, "before the first");
        assert!((at(&linear, 30.0) - 300.0).abs() < 1e-4, "past the last");
        let constant = table(Interpolation::Constant);
        assert!((at(&constant, 9.9) - 10.0).abs() < 1e-4);
        assert!((at(&constant, 10.0) - 100.0).abs() < 1e-4);
        let eased = table(Interpolation::Ease(EaseFunction::QuadraticIn));
        assert!((at(&eased, 10.0) - 100.0).abs() < 1e-4, "through its keys");
        // Halfway from 1 to 10, quadratic in: a quarter of the way up.
        assert!((at(&eased, 5.5) - 32.5).abs() < 1e-3);
    }

    #[test]
    fn a_single_key_is_its_value_everywhere() {
        let table = CurveRegistry::build(&[LevelCurve {
            name: Name::from_static("flat"),
            interpolation: Interpolation::Linear,
            keys: vec![(1.0, 7.0)],
        }])
        .unwrap();
        assert_eq!(table.at(table.id("flat").unwrap(), 50.0), Some(7.0));
    }

    fn curve(name: &'static str) -> LevelCurve {
        LevelCurve {
            name: Name::from_static(name),
            interpolation: Interpolation::Linear,
            keys: vec![(1.0, 1.0)],
        }
    }

    #[test]
    fn curves_are_numbered_by_name_and_keep_their_numbers() {
        let first = CurveRegistry::build(&[curve("b"), curve("a")]).unwrap();
        assert_eq!(first.id("a"), Some(CurveId::new(0)));
        assert_eq!(first.id("b"), Some(CurveId::new(1)));
        let reloaded = first.rebuild(&[curve("b"), curve("c")]).unwrap();
        assert_eq!(reloaded.id("b"), Some(CurveId::new(1)), "kept");
        assert_eq!(reloaded.id("c"), Some(CurveId::new(2)), "after the last");
        assert_eq!(reloaded.at(CurveId::new(0), 1.0), None, "gone");
        assert_eq!(reloaded.name(CurveId::new(2)), Some("c"));
    }

    #[test]
    fn a_curve_table_checks_its_keys() {
        let mut def = LevelCurve {
            name: Name::from_static("damage"),
            interpolation: Interpolation::Linear,
            keys: vec![(10.0, 100.0), (1.0, 10.0)],
        };
        let errors = CurveRegistry::build(&[def.clone()])
            .unwrap_err()
            .to_string();
        assert_eq!(errors, "a curve whose levels don't rise (in `damage`)");
        def.keys.clear();
        assert!(CurveRegistry::build(&[def]).is_err());
    }

    #[test]
    fn an_ease_is_written_by_name() {
        use serde_test::{Token, assert_tokens};

        assert_tokens(
            &Interpolation::Ease(EaseFunction::SineInOut),
            &[
                Token::NewtypeVariant {
                    name: "Interpolation",
                    variant: "Ease",
                },
                Token::UnitVariant {
                    name: "EaseFunction",
                    variant: "SineInOut",
                },
            ],
        );
    }
}
