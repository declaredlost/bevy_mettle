//! What using tags, curves and conditions takes:
//! `use bevy_mettle_core::prelude::*`.
//!
//! Weighing a condition in a context of your own takes
//! [`Context`](crate::condition::Context) too.

#[doc(no_inline)]
pub use crate::condition::Condition;
#[doc(no_inline)]
pub use crate::curve::{CurveTable, Interpolation, LevelCurve};
#[doc(no_inline)]
pub use crate::tag::{OwnedTags, Tag, TagAdded, TagCounts, TagRemoved, TagTree, Tags, owns};
#[doc(no_inline)]
pub use crate::{Id, IdTarget, MettleError, MettleErrors};
