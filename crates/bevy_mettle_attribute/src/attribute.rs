//! What an attribute is: a marker type, its rules on it.

use bevy_mettle_core::registry::{AnyAttribute, Keyed};

use crate::registry::{Limit, OnLimit, Rounding};

/// An attribute: a number an actor has, as a marker type its rules are on.
///
/// Life, damage, a resistance. Derive it:
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
/// | `on_limit = Clamp`, `Ratio` | What its base does as a limit that's another attribute moves: kept within it, or kept at the same fraction of its maximum | `Clamp` |
/// | `name = ".."` | The name data names it by | its type's, in `snake_case` |
///
/// An attribute that differs by kind (a resistance per damage type) is one
/// attribute, its modifiers counting for reads tagged so:
/// `ModifierSpec::flat::<Resistance>(0.2).read_tagged("damage.elemental")`.
///
/// Or implement it, for rules the derive can't say: its rules are constants,
/// and its name is its [`Keyed`] key, as an `AnyAttribute`.
///
/// ```
/// use bevy_mettle_attribute::Attribute;
/// use bevy_mettle_attribute::registry::Limit;
/// use bevy_mettle_core::registry::{AnyAttribute, Key, Keyed};
///
/// struct MaxLife;
/// impl Keyed for MaxLife {
///     type Kind = AnyAttribute;
///
///     const KEY: &'static str = "max_life";
/// }
/// impl Attribute for MaxLife {
///     const BASE: f32 = 50.0;
/// }
///
/// struct Life;
/// impl Keyed for Life {
///     type Kind = AnyAttribute;
///
///     const KEY: &'static str = "life";
/// }
/// impl Attribute for Life {
///     const BASE: f32 = 50.0;
///     const MAX: Option<Limit> = Some(Limit::attribute::<MaxLife>());
///     const MIN: Option<Limit> = Some(Limit::Value(0.0));
/// }
///
/// assert_eq!(Key::of::<Life>().as_str(), "life");
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't an attribute",
    label = "not an attribute",
    note = "a limit is a number written out, or a type with `#[derive(Attribute)]`"
)]
pub trait Attribute: Keyed<Kind = AnyAttribute> + Send + Sync + 'static {
    /// What its base starts at.
    const BASE: f32 = 0.0;

    /// The least it can be.
    const MIN: Option<Limit> = None;

    /// The most it can be.
    const MAX: Option<Limit> = None;

    /// How it's rounded, if it's a whole number.
    const ROUNDING: Option<Rounding> = None;

    /// What its base does as a limit that's another attribute moves.
    const ON_LIMIT: OnLimit = OnLimit::Clamp;
}
