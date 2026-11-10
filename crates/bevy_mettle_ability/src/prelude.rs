//! What using abilities takes: their components, events, commands and data.
//!
//! `use bevy_mettle_ability::prelude::*`.
//!
//! Effects, attributes, and tags, curves and conditions are their own
//! crates', each in its own prelude.

#[doc(no_inline)]
pub use bevy_mettle_core::registry::{AbilityId, AnyAbility};

#[doc(no_inline)]
pub use crate::active::{
    Abilities, AbilityOf, ActiveAbility, GrantedAbilities, GrantedAbility, GrantedBy,
};
#[doc(no_inline)]
pub use crate::behaviour::{Behaviour, BehaviourApp, BehaviourRegistry, InputCheck, NoInput};
#[doc(no_inline)]
pub use crate::data::{AbilityData, Commit, Grant, NetExecution, Trigger};
#[doc(no_inline)]
pub use crate::events::{
    AbilityActivated, AbilityEnded, AbilityFailed, Activated, Failure, GameplayEvent,
};
#[doc(no_inline)]
pub use crate::net::AbilityInput;
#[doc(no_inline)]
pub use crate::net::AbilityReplica;
#[doc(no_inline)]
pub use crate::net::{AbilityCancelRequest, AbilityEnd};
#[doc(no_inline)]
pub use crate::net::{AbilityReply, AbilityRequest, PredictionTimeout};
#[doc(no_inline)]
pub use crate::{
    Ability, AbilityApp, AbilityCommands, AbilityPlugin, AbilityRegistry, AbilityScope,
    AbilitySpec, AbilitySystems, AbilityWorld,
};
