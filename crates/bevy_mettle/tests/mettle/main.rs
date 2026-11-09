//! `bevy_mettle`'s tests: attributes folded and read, when modifiers count,
//! what's told as they change, effects, guards, and what replication needs.
#![cfg(all(feature = "attribute", feature = "effect", feature = "ability"))]

mod abilities;
mod common;
mod conditions;
mod content;
mod cues;
mod effects;
mod events;
mod folding;
mod guards;
mod limits;
mod network;
mod plugins;
mod prediction;
mod reading;
mod routes;
mod tags;
