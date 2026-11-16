//! `define` defines all of an effect: its other keys go in the function.

use bevy_mettle::prelude::*;

fn burn(effect: EffectData) -> EffectData {
    effect.lasting(4.0)
}

#[derive(Effect)]
#[effect(define = burn, period = 0.5)]
struct Burn;

fn main() {}
