//! A curve's `name` is a string: its key, as data names it.

use bevy_mettle::prelude::*;

#[derive(ScalingCurve)]
#[curve(name = fireball_damage, keys = [(1.0, 10.0), (10.0, 85.0)])]
struct FireballDamage;

fn main() {}
