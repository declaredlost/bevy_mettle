//! Curve tables: values by level, from data, sampled as Bevy's curves.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use bevy_curve::{
    ConstantCurve, Curve as BevyCurve, EaseFunction, Interval, UnevenSampleAutoCurve,
    UnevenSampleCurve,
};
use serde::{Deserialize, Serialize};

use crate::{Id, IdTarget, MettleError, MettleErrors};

/// How a curve goes between its keys.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum Interpolation {
    /// Each key's value until the next.
    Constant,
    /// A straight line from each key to the next.
    #[default]
    Linear,
    /// From each key to the next as one of Bevy's easing functions goes
    /// from 0 to 1: `Ease(SineInOut)`, `Ease(ExponentialIn)`.
    Ease(EaseFunction),
}

/// A curve: a value by level, its keys `(level, value)`, levels rising.
/// What an effect's magnitude scales by, as Unreal's `ScalableFloat` does;
/// defined in data, and named by an `Id<LevelCurve>`.
///
/// ```ron
/// [(id: "fireball.damage", interpolation: Ease(QuadraticIn), keys: [(1, 10), (10, 85), (20, 240)])]
/// ```
#[derive(IdTarget)]
#[id_target(name = "curve")]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelCurve {
    /// Its id.
    pub id: Id<LevelCurve>,
    /// How it goes between keys: linearly, unless it says.
    #[serde(default)]
    pub interpolation: Interpolation,
    /// Its keys, `(level, value)`, levels rising. Before the first and
    /// past the last, it's as at that one.
    pub keys: Vec<(f32, f32)>,
}

/// A curve, built: one of Bevy's, over every level.
type Built = dyn BevyCurve<f32> + Send + Sync;

impl LevelCurve {
    /// It as one of Bevy's curves, its keys checked: kept as at its ends
    /// beyond them.
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

/// The curves there are, checked (each with keys, all finite, levels
/// rising) and built as Bevy's curves. Cheap to clone; empty by default.
#[derive(Clone, Default)]
pub struct CurveTable(Arc<HashMap<Id<LevelCurve>, Arc<Built>>>);

impl fmt::Debug for CurveTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut curves: Vec<&Id<LevelCurve>> = self.0.keys().collect();
        curves.sort();
        f.debug_tuple("CurveTable").field(&curves).finish()
    }
}

impl CurveTable {
    /// Checks and builds `definitions`.
    ///
    /// # Errors
    ///
    /// Every curve defined twice, or whose keys can't be.
    pub fn build<'a>(
        definitions: impl IntoIterator<Item = &'a LevelCurve>,
    ) -> Result<Self, MettleErrors> {
        let mut errors = Vec::new();
        let mut curves = HashMap::new();
        for def in definitions {
            match def.build() {
                Ok(curve) => {
                    if curves.insert(def.id.clone(), curve).is_some() {
                        errors.push(MettleError::Duplicate {
                            kind: LevelCurve::NAME,
                            id: def.id.to_string(),
                        });
                    }
                }
                Err(what) => errors.push(MettleError::Invalid {
                    what: format!("a curve {what}"),
                    within: Some(def.id.to_string()),
                }),
            }
        }
        if errors.is_empty() {
            Ok(Self(Arc::new(curves)))
        } else {
            Err(MettleErrors(errors))
        }
    }

    /// `curve`'s value at `level`, if there's such a curve.
    #[must_use]
    pub fn at(&self, curve: &Id<LevelCurve>, level: f32) -> Option<f32> {
        self.0.get(curve).map(|built| built.sample_clamped(level))
    }

    /// `curve`, as one of Bevy's curves: for graphs, tooling, and Bevy's
    /// curve adaptors.
    #[must_use]
    pub fn curve(&self, curve: &Id<LevelCurve>) -> Option<&(dyn BevyCurve<f32> + Send + Sync)> {
        self.0.get(curve).map(AsRef::as_ref)
    }

    /// `curve`, if it's defined, named within `within` if it's named in
    /// something.
    ///
    /// # Errors
    ///
    /// If it isn't.
    pub fn known(&self, curve: &Id<LevelCurve>, within: Option<&str>) -> Result<(), MettleError> {
        if self.0.contains_key(curve) {
            Ok(())
        } else {
            Err(MettleError::unknown::<LevelCurve>(curve.as_str(), within))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(interpolation: Interpolation) -> CurveTable {
        CurveTable::build(&[LevelCurve {
            id: Id::from_static("damage"),
            interpolation,
            keys: vec![(1.0, 10.0), (10.0, 100.0), (20.0, 300.0)],
        }])
        .unwrap()
    }

    fn at(table: &CurveTable, level: f32) -> f32 {
        table.at(&Id::from_static("damage"), level).unwrap()
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
        let table = CurveTable::build(&[LevelCurve {
            id: Id::from_static("flat"),
            interpolation: Interpolation::Linear,
            keys: vec![(1.0, 7.0)],
        }])
        .unwrap();
        assert_eq!(table.at(&Id::from_static("flat"), 50.0), Some(7.0));
    }

    #[test]
    fn a_curve_table_checks_its_keys() {
        let mut def = LevelCurve {
            id: Id::from_static("damage"),
            interpolation: Interpolation::Linear,
            keys: vec![(10.0, 100.0), (1.0, 10.0)],
        };
        let errors = CurveTable::build(&[def.clone()]).unwrap_err().to_string();
        assert_eq!(errors, "a curve whose levels don't rise (in `damage`)");
        def.keys.clear();
        assert!(CurveTable::build(&[def]).is_err());
    }

    #[test]
    fn an_ease_reads_from_data_by_name() {
        let def: LevelCurve = ron::from_str(
            r#"(id: "damage", interpolation: Ease(SineInOut), keys: [(1, 10), (10, 85)])"#,
        )
        .unwrap();
        assert_eq!(
            def.interpolation,
            Interpolation::Ease(EaseFunction::SineInOut)
        );
        assert!(CurveTable::build(&[def]).is_ok());
    }
}
