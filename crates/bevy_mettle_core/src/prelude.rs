//! What using tags, curves and conditions takes, in one glob import.
//!
//! `use bevy_mettle_core::prelude::*`.
//!
//! Weighing a condition in a context of your own takes
//! [`Context`](crate::condition::Context) too. Names, keys and registries are
//! reached by [`registry`](crate::registry); errors are [`Error`](crate::Error)
//! and [`Errors`](crate::Errors).

#[doc(no_inline)]
pub use crate::condition::{Condition, ConditionOver};
#[doc(no_inline)]
pub use crate::curve::{CurveId, CurveRegistry, Interpolation, LevelCurve};
#[doc(no_inline)]
pub use crate::net::NetRole;
#[doc(no_inline)]
pub use crate::tag::{
    OwnedTags, Root, Tag, TagAdded, TagApp, TagCounts, TagId, TagKey, TagRegistry, TagRemoved,
    Tags, owns,
};
