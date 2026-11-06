//! What an attribute is: a marker type, its rules on it.

use bevy_mettle_core::{Id, IdTarget};
use serde::{Deserialize, Serialize};

/// An attribute: a number an actor has (life, damage, a resistance), as a
/// marker type its rules are on. Derive it:
///
/// ```ignore
/// #[derive(Attribute)]
/// #[attribute(base = 50.0, min = 0.0, max = MaxLife)]
/// pub struct Life;
/// ```
///
/// | Setting | Meaning | Unless it says |
/// |---|---|---|
/// | `base = 50.0` | What its base starts at | `0.0` |
/// | `min = 0.0`, `min = Other` | The least it can be: a value, or another attribute's current value, read in the same context | none |
/// | `max = ..` | The most it can be, as `min` | none |
/// | `rounding = Down`, `Up`, `Nearest` | A whole number, rounded so | none |
/// | `name = ".."` | The name data names it by | its type's, in `snake_case` |
///
/// An attribute that differs by kind (a resistance per damage type) is one
/// attribute, its modifiers counting for reads tagged so:
/// `Modifier::flat::<Resistance>(0.2).read_tagged("damage.elemental")`.
///
/// Or implement it, for rules the derive can't say: its rules are
/// constants.
///
/// ```
/// use bevy_mettle_attribute::{Attribute, Limit};
///
/// struct MaxLife;
/// impl Attribute for MaxLife {
///     const BASE: f32 = 50.0;
///     const NAME: &'static str = "max_life";
/// }
///
/// struct Life;
/// impl Attribute for Life {
///     const BASE: f32 = 50.0;
///     const MAX: Option<Limit> = Some(Limit::attribute::<MaxLife>());
///     const MIN: Option<Limit> = Some(Limit::Value(0.0));
///     const NAME: &'static str = "life";
/// }
///
/// assert_eq!(Life::ID.as_str(), "life");
/// ```
pub trait Attribute: Send + Sync + 'static {
    /// The name data names it by: `"life"`.
    const NAME: &'static str;

    /// Its id: its name.
    const ID: Id<AnyAttribute> = Id::from_static(Self::NAME);

    /// What its base starts at.
    const BASE: f32 = 0.0;

    /// The least it can be.
    const MIN: Option<Limit> = None;

    /// The most it can be.
    const MAX: Option<Limit> = None;

    /// How it's rounded, if it's a whole number.
    const ROUNDING: Option<Rounding> = None;
}

/// Any attribute: what attributes are named by, as an `Id<AnyAttribute>`,
/// in data and in modifiers. Each attribute is a type of its own; this is
/// what they share.
#[derive(IdTarget)]
#[id_target(name = "attribute")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyAttribute {}

/// What an attribute is kept within.
#[derive(Debug, Clone, PartialEq)]
pub enum Limit {
    /// This value.
    Value(f32),
    /// This attribute's current value, as the actor has it, read in the
    /// same context.
    Attribute(Id<AnyAttribute>),
}

impl Limit {
    /// `A`'s current value.
    #[must_use]
    pub const fn attribute<A: Attribute>() -> Self {
        Self::Attribute(A::ID)
    }
}

/// How a whole-number attribute is rounded.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize
)]
pub enum Rounding {
    /// Towards negative infinity: 2.9 is 2.
    #[default]
    Down,
    /// Towards positive infinity: 2.1 is 3.
    Up,
    /// To the nearest, halves away from 0: 2.5 is 3.
    Nearest,
}

impl Rounding {
    pub(crate) fn round(self, value: f32) -> f32 {
        match self {
            Self::Down => value.floor(),
            Self::Up => value.ceil(),
            Self::Nearest => value.round(),
        }
    }
}
