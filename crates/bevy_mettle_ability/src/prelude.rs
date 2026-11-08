//! What using abilities takes: `use bevy_mettle_ability::prelude::*`.
//!
//! Effects, attributes, and tags, curves and conditions are their own
//! crates', each in its own prelude.

#[doc(no_inline)]
pub use crate::active::{
    Abilities, AbilityOf, ActiveAbility, GrantedAbilities, GrantedAbility, GrantedBy,
};
#[doc(no_inline)]
pub use crate::behaviour::{Behaviour, BehaviourApp, Behaviours, InputCheck, NoInput};
#[doc(no_inline)]
pub use crate::data::{Ability, Commit, Grant, Trigger};
#[doc(no_inline)]
pub use crate::events::{AbilityActivated, AbilityEnded, AbilityFailed, Failure, GameplayEvent};
#[doc(no_inline)]
pub use crate::net::end::{AbilityCancelRequest, AbilityEnd};
#[doc(no_inline)]
pub use crate::net::input::AbilityInput;
#[doc(no_inline)]
pub use crate::net::replica::AbilityReplica;
#[doc(no_inline)]
pub use crate::net::request::{AbilityReply, AbilityRequest, AbilityRequestWith};
#[doc(no_inline)]
pub use crate::{
    AbilityChecks, AbilityCommands, AbilityPlugin, AbilityReader, AbilityRegistry, AbilitySystems,
    AbilityWorld,
};
