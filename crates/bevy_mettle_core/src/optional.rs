//! Optional fields written as their value, not wrapped in `Some`, and left out
//! when there's nothing. For a field with
//! `#[serde(default, with = "optional", skip_serializing_if =
//! "Option::is_none")]`. Shared by the crates whose data has optional fields.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Writes what's there (a field with nothing is skipped).
pub fn serialize<T: Serialize, S: Serializer>(
    value: &Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => value.serialize(serializer),
        None => serializer.serialize_none(),
    }
}

/// Reads what's there (a field left out is `None`, by its default).
pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}
