//! Curves: values by key, defined in code, tuned in data, and numbered.
//!
//! Sampled as Bevy's curves.

use std::any::TypeId;
use std::sync::Arc;

use bevy_app::App;
#[doc(no_inline)]
pub use bevy_curve::EaseFunction;
use bevy_curve::{
    ConstantCurve, Curve as BevyCurve, Interval, UnevenSampleAutoCurve, UnevenSampleCurve,
};
use bevy_ecs::resource::Resource;
use bevy_platform::collections::HashMap;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

use crate::error::{Error, Errors};
use crate::registry::{AnyCurve, CurveId, Definitions, Key, Keyed, Kind, Registry};

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

/// A curve's definition: a value by key, from keys `(at, value)` rising.
///
/// What an effect's magnitude scales by, read at the spec's level as Unreal's
/// `ScalableFloat` is, or at what else the magnitude says: an attribute, for
/// diminishing returns. Named by a `Key<AnyCurve>`.
///
/// Defined by a [`ScalingCurve`] type, whose derive's keys are its builder
/// methods; content may define it again by its key, to tune it, as
/// Unreal's curve tables are:
///
/// ```
/// # use bevy_mettle_core::prelude::*;
/// # use bevy_curve::EaseFunction;
/// let damage = CurveData::new("fireball_damage")
///     .keys([(1.0, 10.0), (10.0, 85.0), (20.0, 240.0)])
///     .ease(EaseFunction::QuadraticIn);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurveData {
    /// Its name.
    pub name: Key<AnyCurve>,
    /// How it goes between keys: linearly, unless it says.
    #[serde(default)]
    pub interpolation: Interpolation,
    /// Its keys, `(at, value)`, what they're at rising: a level, or what else
    /// it's read at. Before the first and past the last, it's as at that one.
    pub keys: Vec<(f32, f32)>,
}

impl CurveData {
    /// A curve called `name`, linear, with no keys yet.
    #[must_use]
    pub fn new(name: impl Into<Key<AnyCurve>>) -> Self {
        Self {
            name: name.into(),
            interpolation: Interpolation::Linear,
            keys: Vec::new(),
        }
    }

    /// With the key `value` at `at`, after those it has.
    #[must_use]
    pub fn key(mut self, at: f32, value: f32) -> Self {
        self.keys.push((at, value));
        self
    }

    /// With the keys `keys`, `(at, value)`, after those it has.
    #[must_use]
    pub fn keys(mut self, keys: impl IntoIterator<Item = (f32, f32)>) -> Self {
        self.keys.extend(keys);
        self
    }

    /// Holding each key's value until the next.
    #[must_use]
    pub fn constant(mut self) -> Self {
        self.interpolation = Interpolation::Constant;
        self
    }

    /// Going from each key to the next as `ease` goes from 0 to 1.
    #[must_use]
    pub fn ease(mut self, ease: EaseFunction) -> Self {
        self.interpolation = Interpolation::Ease(ease);
        self
    }
}

/// A curve in code: a marker type that defines itself, by its derive's keys.
///
/// Derive it, each key on `#[curve(..)]` a [`CurveData`] builder method of
/// the same name, and add it to the app; a magnitude scales by it as
/// `Magnitude::scaled::<FireballDamage>(10.0)`:
///
/// ```ignore
/// #[derive(ScalingCurve)]
/// #[curve(keys = [(1.0, 10.0), (10.0, 85.0)], ease = EaseFunction::QuadraticIn)]
/// pub struct FireballDamage;
///
/// app.add_curve::<FireballDamage>();
/// ```
///
/// | Key | Meaning |
/// |---|---|
/// | `name = ".."` | Its key: its type's name in `snake_case`, unless it says |
/// | `define = function` | `fn(CurveData) -> CurveData`, defining all of it |
/// | anything else | the [`CurveData`] method of that name |
///
/// Content defining a curve with its key replaces it: what tuning is.
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't a curve",
    label = "not a curve",
    note = "derive it: `#[derive(ScalingCurve)]`"
)]
pub trait ScalingCurve: Keyed<Kind = AnyCurve> + Send + Sync + 'static {
    /// Its definition: `curve`, called by its key, filled in.
    fn define(curve: CurveData) -> CurveData;
}

pub use bevy_mettle_macros::ScalingCurve;

/// Adding the curves code defines to an app, for magnitudes to scale by.
pub trait CurveApp {
    /// Adds the curve `C`, defined by its type: content may tune it.
    fn add_curve<C: ScalingCurve>(&mut self) -> &mut Self;
}

impl CurveApp for App {
    fn add_curve<C: ScalingCurve>(&mut self) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<CurveRegistry>()
            .add::<C>();
        self
    }
}

/// A curve, built: one of Bevy's, over everything it's read at.
type Built = dyn BevyCurve<f32> + Send + Sync;

impl CurveData {
    /// It as one of Bevy's curves, its keys checked: kept as at its ends beyond
    /// them.
    fn build(&self) -> Result<Arc<Built>, &'static str> {
        let keys = &self.keys;
        if keys.is_empty() {
            return Err("with no keys");
        }
        if keys
            .iter()
            .any(|(at, value)| !at.is_finite() || !value.is_finite())
        {
            return Err("with a key that isn't finite");
        }
        if keys.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
            return Err("whose keys don't rise");
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

/// The curves there are, checked, built as Bevy's curves, and numbered.
///
/// Each with keys, all finite, rising, and numbered, a [`CurveId`], in
/// order of name; [`rebuild`](Self::rebuild) keeps the numbers curves had.
/// Read as a [`Registry`] of each curve, built: `get(id)`, `id(name)`,
/// `id_of::<C>()`, `name(id)`, `iter()`. Cheap to clone; empty by default.
/// A resource: the curves code defines, and as content tunes them.
#[derive(Resource, Debug, Clone, Default)]
pub struct CurveRegistry {
    registry: Registry<AnyCurve, Arc<Built>>,
    definitions: Definitions<AnyCurve, CurveData>,
}

impl CurveRegistry {
    /// Checks and builds `content`, numbered in order of name: knowing no
    /// curve types code defines. For tools and tests; an app's content goes
    /// through [`rebuild`](Self::rebuild).
    ///
    /// # Errors
    ///
    /// Every curve defined twice, or whose keys can't be.
    pub fn build<'a>(content: impl IntoIterator<Item = &'a CurveData>) -> Result<Self, Errors> {
        Self::default().rebuild(content)
    }

    /// The curves code defines here, each tuned by `content`'s curve of its
    /// key if there's one, and `content`'s others, checked and built as
    /// [`build`](Self::build) does: each curve here keeping its number, new
    /// ones numbered after the last, in order of name. What a load, a
    /// reload or a hot patch does.
    ///
    /// # Errors
    ///
    /// As `build`'s.
    pub fn rebuild<'a>(
        &self,
        content: impl IntoIterator<Item = &'a CurveData>,
    ) -> Result<Self, Errors> {
        let mut errors = Vec::new();
        let mut built: HashMap<SmolStr, Arc<Built>> = HashMap::new();
        let mut tuned: HashMap<SmolStr, bool> = HashMap::new();
        for def in content {
            if tuned
                .insert(SmolStr::new(def.name.as_str()), true)
                .is_some()
            {
                errors.push(Error::Duplicate {
                    kind: AnyCurve::NAME,
                    name: def.name.to_string(),
                });
                continue;
            }
            insert(def, &mut built, &mut errors);
        }
        for def in self.definitions.definitions(CurveData::new) {
            if !tuned.contains_key(def.name.as_str()) {
                insert(&def, &mut built, &mut errors);
            }
        }
        if !errors.is_empty() {
            return Err(Errors(errors));
        }
        let mut names: Vec<SmolStr> = built.keys().cloned().collect();
        names.sort_unstable();
        let mut registry = self.registry.renumbered(names);
        for (name, curve) in built {
            if let Some(id) = registry.id(&name) {
                registry.set(id, curve);
            }
        }
        Ok(Self {
            registry,
            definitions: self.definitions.clone(),
        })
    }

    /// Adds the curve `C` code defines: built as content's checked.
    pub fn add<C: ScalingCurve>(&mut self) {
        self.definitions.add::<C>(C::define);
        self.registry.name_type(TypeId::of::<C>(), C::KEY);
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
        self.registry.get(curve).map(AsRef::as_ref)
    }
}

/// Read as the [`Registry`] of each curve, built, by number.
impl std::ops::Deref for CurveRegistry {
    type Target = Registry<AnyCurve, Arc<Built>>;

    fn deref(&self) -> &Self::Target {
        &self.registry
    }
}

/// `def` built into `built`, or what's wrong with it noted.
fn insert(def: &CurveData, built: &mut HashMap<SmolStr, Arc<Built>>, errors: &mut Vec<Error>) {
    match def.build() {
        Ok(curve) => {
            built.insert(SmolStr::new(def.name.as_str()), curve);
        }
        Err(what) => errors.push(Error::invalid(
            format!("a curve {what}"),
            Some(def.name.as_str()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(interpolation: Interpolation) -> CurveRegistry {
        CurveRegistry::build(&[CurveData {
            name: Key::from_static("damage"),
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
        let table = CurveRegistry::build(&[CurveData {
            name: Key::from_static("flat"),
            interpolation: Interpolation::Linear,
            keys: vec![(1.0, 7.0)],
        }])
        .unwrap();
        assert_eq!(table.at(table.id("flat").unwrap(), 50.0), Some(7.0));
    }

    fn curve(name: &'static str) -> CurveData {
        CurveData {
            name: Key::from_static(name),
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
        let mut def = CurveData {
            name: Key::from_static("damage"),
            interpolation: Interpolation::Linear,
            keys: vec![(10.0, 100.0), (1.0, 10.0)],
        };
        let errors = CurveRegistry::build(&[def.clone()])
            .unwrap_err()
            .to_string();
        assert_eq!(errors, "a curve whose keys don't rise (in `damage`)");
        def.keys.clear();
        assert!(CurveRegistry::build(&[def]).is_err());
    }

    struct Damage;
    impl Keyed for Damage {
        type Kind = AnyCurve;

        const KEY: &'static str = "damage";
    }
    impl ScalingCurve for Damage {
        fn define(curve: CurveData) -> CurveData {
            curve.keys([(1.0, 10.0), (10.0, 100.0)])
        }
    }

    #[test]
    fn content_tunes_a_curve_code_defines() {
        let mut code = CurveRegistry::default();
        code.add::<Damage>();
        let defined = code.rebuild(&[]).unwrap();
        let damage = defined.id_of::<Damage>().unwrap();
        assert_eq!(defined.at(damage, 10.0), Some(100.0));
        let tuned = code
            .rebuild(&[CurveData::new("damage").key(1.0, 5.0)])
            .unwrap();
        assert_eq!(tuned.id_of::<Damage>(), Some(damage), "keeps its number");
        assert_eq!(tuned.at(damage, 10.0), Some(5.0), "as content says");
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
