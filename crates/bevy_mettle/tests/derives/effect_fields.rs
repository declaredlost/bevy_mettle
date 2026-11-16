//! An effect is a marker: what changes as it's applied goes on its spec's
//! context, not on the type.

use bevy_mettle::prelude::*;

#[derive(Effect)]
struct Burn {
    amount: f32,
}

fn main() {}
