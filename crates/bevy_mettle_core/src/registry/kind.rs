//! Kinds: what content defines, and each core kind keys and ids are typed by.
//!
//! Tags, curves, attributes, effects and abilities, here so one part names
//! another's without depending on it, as an effect names the ability that
//! applied it.

/// A kind of thing content defines and names: an effect, an ability, a curve.
///
/// A tag is a kind too, as is anything else a game defines.
///
/// What a [`Key`](super::Key) names, and an [`Id`](super::Id) numbers.
/// Usually an empty enum, `AnyEffect`. Derive it, named in messages as the
/// type is in `snake_case`, without a leading `Any`, unless
/// `#[kind(name = "..")]` says otherwise:
///
/// ```
/// use bevy_mettle_core::registry::{Key, Kind};
///
/// #[derive(Kind)]
/// enum AnyItem {}
///
/// let sword: Key<AnyItem> = Key::from_static("sword");
/// assert_eq!(format!("{sword:?}"), r#"item("sword")"#);
/// ```
pub trait Kind: 'static {
    /// What one is called in messages: `effect`, `item`.
    const NAME: &'static str;
}

pub use bevy_mettle_macros::Kind;

/// A type that names something of kind `K`: its key, without constructing it.
///
/// Implemented by the derives (`#[derive(Effect)]`, `#[derive(Ability)]`,
/// `#[derive(Behaviour)]`, ...) with the name they give the type, for
/// [`Key::of`](super::Key::of). One type can be keyed as more than one kind:
/// an ability and its effect, say.
#[diagnostic::on_unimplemented(
    message = "`{Self}` doesn't name a {K}",
    label = "not keyed as this kind",
    note = "derive what it is: `#[derive(Effect)]`, `#[derive(Ability)]`, ..."
)]
pub trait Keyed<K: Kind> {
    /// Its name as a `K`, as content writes it and its key holds it:
    /// `"burning"`.
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
