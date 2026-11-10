//! Kinds: what content defines, and each core kind keys and ids are typed by.
//!
//! Tags, curves, attributes, effects and abilities, here so one part names
//! another's without depending on it, as an effect names the ability that
//! applied it.

/// A kind of thing mettle's content defines: what its keys are typed by.
///
/// What a [`Key`](super::Key) names, and an [`Id`](super::Id) numbers, each
/// an empty enum: [`AnyTag`], [`AnyCurve`], [`AnyAttribute`], [`AnyEffect`]
/// and [`AnyAbility`] here; `AnyExecution`, `AnyCalculation`, `AnyRoute` and
/// `AnyContextData` in `bevy_mettle_effect`; `AnyBehaviour` in
/// `bevy_mettle_ability`. A `Key<AnyEffect>` can't stand in for a
/// `Key<AnyAbility>`.
///
/// ```
/// use bevy_mettle_core::registry::{AnyEffect, Key};
///
/// let burning: Key<AnyEffect> = Key::from_static("burning");
/// assert_eq!(format!("{burning:?}"), r#"effect("burning")"#);
/// ```
pub trait Kind: 'static {
    /// What one is called in messages: `effect`, `ability`.
    const NAME: &'static str;
}

/// Mettle's crates derive their kinds; games don't define their own.
#[doc(hidden)]
pub use bevy_mettle_macros::Kind;

/// A type that names something content defines: its kind and its key.
///
/// Implemented by the derives (`#[derive(Effect)]`, `#[derive(Ability)]`,
/// `#[derive(Behaviour)]`, ...) with the name they give the type, for
/// [`Key::of`](super::Key::of). A type is of one kind: an ability and its
/// effect are two types.
#[diagnostic::on_unimplemented(
    message = "`{Self}` doesn't name anything content defines",
    label = "not keyed",
    note = "derive what it is: `#[derive(Effect)]`, `#[derive(Ability)]`, ..."
)]
pub trait Keyed {
    /// What it is: `AnyEffect`, `AnyAbility`.
    type Kind: Kind;

    /// Its name, as content writes it and its key holds it: `"burning"`.
    const KEY: &'static str;
}

/// The tag kind: what a [`TagId`](super::TagId) numbers, as the
/// [`TagRegistry`](crate::tag::TagRegistry) holds them.
///
/// Tags themselves are named by [`TagKey`](crate::tag::TagKey), by type or by
/// name.
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyTag {}

/// The curve kind: what a `Key<AnyCurve>` names and a
/// [`CurveId`](super::CurveId) numbers.
///
/// A curve's key in content is a `Key<AnyCurve>`, until it's resolved to a
/// [`CurveId`](super::CurveId).
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyCurve {}

/// The attribute kind: what data names, and an
/// [`AttributeId`](super::AttributeId) numbers.
///
/// An attribute's key in data is a `Key<AnyAttribute>`, until it's resolved
/// to an [`AttributeId`](super::AttributeId).
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyAttribute {}

/// The effect kind: what content names, and an [`EffectId`](super::EffectId)
/// numbers.
///
/// An effect's key in content is a `Key<AnyEffect>`, until it's resolved to
/// an [`EffectId`](super::EffectId).
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyEffect {}

/// The ability kind: what content names, and an
/// [`AbilityId`](super::AbilityId) numbers.
///
/// An ability's key in content is a `Key<AnyAbility>`, until it's resolved
/// to an [`AbilityId`](super::AbilityId).
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyAbility {}
