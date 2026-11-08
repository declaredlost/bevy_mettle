# bevy_mettle_effect

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]

**Part of [bevy_mettle](https://github.com/declaredlost/bevy_mettle), a gameplay framework for Bevy.**

[action-badge]: https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_mettle/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/declaredlost/bevy_mettle/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/bevy_mettle_effect.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/bevy_mettle_effect
[docs-badge]: https://img.shields.io/docsrs/bevy_mettle_effect?logo=Docs.rs&style=flat-square
[docs-url]: https://docs.rs/bevy_mettle_effect

Effects for Bevy, as in Unreal's Gameplay Ability System: what changes
attributes and grants tags, at once, for a while, for good, or every so
often.

## Overview

- An [`Effect`] is data, named by an id: its duration, period, modifiers,
  executions in code, the tags it grants, what it needs to apply or to
  count, what it removes or keeps off, how it stacks, and the effects it
  chains into.
- An [`EffectRegistry`] is built from them, checked against the tag tree,
  the curves, the attributes the app added and your [`EffectCode`]: every
  error at once. Insert a new one and active effects take their new
  definitions.
- An [`EffectSpec`] is what's applied: the effect, its level, its params
  and its source. `apply_effect` checks it, then executes it, stacks it, or
  spawns it as an entity [`EffectOf`] its target.
- While it counts, a lasting effect's modifiers count for its target and
  the target owns what it grants; while inhibited, neither, and its time
  stands still.
- [`EffectApplied`], [`EffectStacked`], [`EffectBlocked`],
  [`EffectExecuted`] and [`EffectRemoved`] are triggered on the target, for
  abilities and the game.
- Cues are what's shown of effects and abilities: lasting ones as entities
  on the actor while they hold, one-shots as they execute, run by the
  handler for their most specific tag, and replicated to every client that
  sees the actor.
- Over a network, a server keeps each effect's replica to send, and a
  client shows them, and predicts its own.

## Installation

Most games depend on `bevy_mettle`, where effects are a default feature and
this crate is `bevy_mettle::effect`:

```toml
[dependencies]
bevy_mettle = "0.1"
```

Or depend on this crate directly, for attributes and effects without
abilities:

```toml
[dependencies]
bevy_mettle_effect = "0.1"
```

## Usage

```rust,no_run
use bevy::prelude::*;
use bevy_mettle_attribute::prelude::*;
use bevy_mettle_core::prelude::*;
use bevy_mettle_effect::prelude::*;

#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0)]
struct Life;

/// Burning: 5 fire damage every half second, for 4 seconds.
fn load(mut commands: Commands, attributes: Res<AttributeRegistry>, code: Res<EffectCode>) {
    let tags: Vec<Tag> = ["state", "state.burning"]
        .into_iter()
        .map(|id| Tag { id: Id::new(id) })
        .collect();
    let burning = Effect::lasting("burning", 4.0)
        .period(0.5, true)
        .modifier(EffectModifier::new::<Life>(Operation::Flat, -5.0))
        .grant("state.burning");
    let checks = EffectChecks {
        tags: &TagTree::build(&tags).unwrap(),
        curves: &CurveTable::default(),
        attributes: &attributes,
        code: &code,
    };
    commands.insert_resource(EffectRegistry::check(&[burning], checks).unwrap());
}

#[derive(Component)]
struct Mage;

fn spawn(mut commands: Commands) {
    commands.spawn((Mage, Base::<Life>::default()));
}

/// Sets every mage alight.
fn ignite(mages: Query<Entity, Added<Mage>>, mut commands: Commands) {
    for mage in &mages {
        commands.entity(mage).apply_effect(EffectSpec::new("burning"));
    }
}

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, EffectPlugin::default()))
        .add_attribute::<Life>()
        .add_systems(Startup, (load, spawn))
        .add_systems(Update, ignite)
        .run();
}
```

Effects tick in [`EffectSystems`], in `FixedUpdate` unless
[`EffectPlugin::new`] says otherwise.

## Learn more

- The book's [Defining effects](https://declaredlost.github.io/bevy_mettle/effects.html),
  [Applying effects](https://declaredlost.github.io/bevy_mettle/applying-effects.html),
  [Stacking](https://declaredlost.github.io/bevy_mettle/stacking.html) and
  [Effects in code](https://declaredlost.github.io/bevy_mettle/effect-code.html)
  pages, [Cues](https://declaredlost.github.io/bevy_mettle/cues.html),, and [Prediction](https://declaredlost.github.io/bevy_mettle/prediction.html)
  for the network
- The [API documentation](https://docs.rs/bevy_mettle_effect)

## Contributing

See [CONTRIBUTING.md](https://github.com/declaredlost/bevy_mettle/blob/main/CONTRIBUTING.md)
for development setup and guidelines.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-MIT) at your option.

[`Effect`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/struct.Effect.html
[`EffectRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html
[`EffectCode`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/struct.EffectCode.html
[`EffectSpec`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectSpec.html
[`EffectOf`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/active/struct.EffectOf.html
[`EffectApplied`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectApplied.html
[`EffectStacked`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectStacked.html
[`EffectBlocked`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectBlocked.html
[`EffectExecuted`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectExecuted.html
[`EffectRemoved`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectRemoved.html
[`EffectSystems`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectSystems.html
[`EffectPlugin::new`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectPlugin.html#method.new
