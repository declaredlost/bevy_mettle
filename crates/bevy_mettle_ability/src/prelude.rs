//! What using abilities takes: defining, granting and activating them.
//!
//! Their behaviours and events too.
//!
//! `use bevy_mettle_ability::prelude::*`.
//!
//! Effects, attributes, and tags, curves and conditions are their own
//! crates', each in its own prelude. What content is checked by (registries
//! and scopes), and what's sent over a network, are reached by their
//! modules.

#[doc(no_inline)]
pub use crate::active::{
    Abilities, AbilityOf, ActiveAbility, GrantedAbilities, GrantedAbility, GrantedBy,
};
#[doc(no_inline)]
pub use crate::behaviour::{Behaviour, BehaviourApp, InputCheck, NoInput};
#[doc(no_inline)]
pub use crate::data::{AbilityData, Commit, Cost, NetExecution, Trigger};
#[doc(no_inline)]
pub use crate::events::{
    AbilityActivated, AbilityEnded, AbilityFailed, Activated, Failure, GameplayEvent,
};
#[doc(no_inline)]
pub use crate::net::PredictionTimeout;
#[doc(no_inline)]
pub use crate::{
    Ability, AbilityApp, AbilityCommands, AbilityPlugin, AbilityRegistry, AbilitySpec,
    AbilitySystems, AbilityWorld,
};
