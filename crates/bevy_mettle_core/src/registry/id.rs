//! What content is numbered by: each content kind, and its id.
//!
//! Tags, curves, attributes, effects and abilities, here so one part names
//! another's without depending on it, as an effect names the ability that
//! applied it.

use super::{Key, Kind};

/// The tag kind: what a [`TagId`] numbers, as the
/// [`TagRegistry`](crate::tag::TagRegistry) holds them.
///
/// Tags themselves are named by [`TagKey`](crate::tag::TagKey), by type or by
/// name.
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyTag {}

/// A tag's number in the [`TagRegistry`](crate::tag::TagRegistry): what the
/// network sends for it.
///
/// The same on a server and its clients, as they check the same tags. A
/// [`TagKey`](crate::tag::TagKey) is a tag as this process knows it; a
/// `TagId` is its place in the registry, shared.
pub type TagId = Key<AnyTag>;

/// The curve kind: what a `Name<AnyCurve>` names and a [`CurveId`] numbers.
///
/// A curve's name in content is a `Name<AnyCurve>`, until it's resolved to a
/// [`CurveId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyCurve {}

/// A curve's number in the [`CurveRegistry`](crate::curve::CurveRegistry):
/// what magnitudes scale by.
///
/// Magnitudes name a curve by number once content is checked.
pub type CurveId = Key<AnyCurve>;

/// The attribute kind: what data names, and an [`AttributeId`] numbers.
///
/// An attribute's name in data is a `Name<AnyAttribute>`, until it's resolved
/// to an [`AttributeId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyAttribute {}

/// An attribute's number in its registry, given as it's added to an app.
///
/// As Bevy's `ComponentId` is to a component: what a modifier, an effect or a
/// replica knows it by.
pub type AttributeId = Key<AnyAttribute>;

/// The effect kind: what content names, and an [`EffectId`] numbers.
///
/// An effect's name in content is a `Name<AnyEffect>`, until it's resolved to
/// an [`EffectId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyEffect {}

/// An effect's number in its registry, as the app knows it.
///
/// What an active effect, a replica, a chain or an event knows it by, as Bevy
/// knows a component by its `ComponentId`.
///
/// Numbered in order of name as content is checked, so the same content is
/// numbered the same on a server and its clients; a reload keeps the
/// numbers effects had.
pub type EffectId = Key<AnyEffect>;

/// The ability kind: what content names, and an [`AbilityId`] numbers.
///
/// An ability's name in content is a `Name<AnyAbility>`, until it's resolved
/// to an [`AbilityId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyAbility {}

/// An ability's number in its registry, as the app knows it.
///
/// What a granted ability, a request, a replica, an event or an effect's
/// context knows it by.
///
/// Numbered in order of name as content is checked, so the same content is
/// numbered the same on a server and its clients; a reload keeps the
/// numbers abilities had.
pub type AbilityId = Key<AnyAbility>;
