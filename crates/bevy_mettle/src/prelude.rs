//! What using `bevy_mettle` takes: every enabled crate's prelude.

#[cfg(feature = "ability")]
pub use bevy_mettle_ability::prelude::*;
#[cfg(feature = "attribute")]
pub use bevy_mettle_attribute::prelude::*;
pub use bevy_mettle_core::prelude::*;
#[cfg(feature = "effect")]
pub use bevy_mettle_effect::prelude::*;
