//! What an id can name.

/// What an [`Id`](super::Id) can name: a tag, a curve, or anything in data.
///
/// A curve, an effect, an ability, or anything else a game
/// defines in data and names by id. Derive it, named in messages as the type is
/// in `snake_case` unless `#[id_target(name = "..")]` says otherwise:
///
/// ```
/// use bevy_mettle_core::{Id, IdTarget};
///
/// #[derive(IdTarget)]
/// struct Item {
///     id: Id<Item>,
/// }
///
/// let sword: Id<Item> = Id::from_static("sword");
/// assert_eq!(format!("{sword:?}"), r#"item("sword")"#);
/// ```
pub trait IdTarget: 'static {
    /// What one is called in messages: `tag`, `item`.
    const NAME: &'static str;
}
