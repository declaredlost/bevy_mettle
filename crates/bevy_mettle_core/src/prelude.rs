//! What using tags, curves and conditions takes:
//! `use bevy_mettle_core::prelude::*`.
//!
//! Weighing a condition in a context of your own takes
//! [`Context`](crate::condition::Context) too.

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
#[doc(no_inline)]
pub use crate::{Id, IdTarget, MettleError, MettleErrors};
