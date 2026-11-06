#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod ability;
mod behaviour;
mod commands;
mod engine;
mod events;
mod granted;
mod net;
mod plugin;
pub mod prelude;
mod registry;

pub use self::ability::{Ability, Commit, Grant, Trigger};
pub use self::behaviour::{Behaviour, BehaviourApp, Behaviours};
pub use self::commands::{AbilityCommands, AbilityWorld};
pub use self::events::{AbilityActivated, AbilityEnded, AbilityFailed, Failure, GameplayEvent};
pub use self::granted::{Abilities, AbilityOf, ActiveAbility, GrantedAbility};
pub use self::net::{AbilityReplica, AbilityReply, AbilityRequest};
pub use self::plugin::AbilityPlugin;
pub use self::registry::AbilityRegistry;
