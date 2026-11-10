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

- An [`EffectData`] is content, named by its name: its duration, period,
  modifiers, executions in code, the tags it grants, what it needs to apply
  or to count, what it removes or keeps off, how it stacks, and the effects
  it chains into.
- An [`EffectRegistry`] is built from them, checked against the tags, the
  curves, the attributes the app added and its executions, calculations and
  routes: every error at once. Each effect is numbered, an `EffectId`, and a
  reload keeps the numbers. Insert a new one and active effects take their
  new definitions.
- A [`Magnitude`] is a small expression: values, the level, params, curves,
  attributes and calculations in code, summed, multiplied and clamped.
  Checked, its names are numbers and its constants folded.
- An attribute it reads is captured as the spec is made (a projectile
  carries its caster's spell power, [`Snapshots`]) or as it's applied, or
  read live, a lasting effect following it as it changes. Executions and
  calculations declare theirs on their derive.
- Code names an effect by an [`Effect`] type, which is its spec: its level,
  its params and its source (`Burning.level(3.0)`), an [`EffectSpec`], and
  its context: what struck, where, and a game's own reflected types.
  `apply_effect` checks it, then executes it, stacks it, or spawns it as an
  entity [`EffectOf`] its target.
- While it counts, a lasting effect's modifiers count for its target and
  the target owns what it grants; while inhibited, neither, and its time
  stands still.
- Routes are amounts no actor holds, such as incoming damage: dealt by
  effects, then routed by one router per route into the attributes actors
  have, through shields into life, the same on a server and a predicting
  client.
- [`EffectApplied`], [`EffectStacked`], [`EffectBlocked`],
  [`EffectExecuted`] and [`EffectRemoved`] are triggered on the target, for
  abilities and the game. [`EffectExecuted`] says what changed, on the
  target and its source, and what each route was dealt.
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

/// Burning, as code names it: content defines it as `"burning"`.
#[derive(Effect)]
struct Burning;

/// Burning: 5 fire damage every half second, for 4 seconds.
fn load(
    mut commands: Commands,
    effects: Res<EffectRegistry>,
    attributes: Res<AttributeRegistry>,
    executions: Res<ExecutionRegistry>,
    calculations: Res<CalculationRegistry>,
    routes: Res<RouteRegistry>,
) {
    let tags: Vec<TagKey> = ["state", "state.burning"]
        .into_iter()
        .map(TagKey::named)
        .collect();
    let burning = EffectData::lasting("burning", 4.0)
        .period(0.5, true)
        .modifier(EffectModifier::new::<Life>(Operation::Flat, -5.0))
        .grant("state.burning");
    let checks = EffectChecks {
        tags: &TagRegistry::build(tags).unwrap(),
        curves: &CurveRegistry::default(),
        attributes: &attributes,
        executions: &executions,
        calculations: &calculations,
        routes: &routes,
    };
    // Checked from the app's registry, which knows `Burning` is code's.
    commands.insert_resource(effects.recheck(&[burning], checks).unwrap());
}

#[derive(Component)]
struct Mage;

fn spawn(mut commands: Commands) {
    commands.spawn((Mage, Base::<Life>::default()));
}

/// Sets every mage alight.
fn ignite(mages: Query<Entity, Added<Mage>>, mut commands: Commands) {
    for mage in &mages {
        commands.entity(mage).apply_effect(Burning);
    }
}

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, EffectPlugin))
        .add_attribute::<Life>()
        .add_effect::<Burning>()
        .add_systems(Startup, (load, spawn))
        .add_systems(Update, ignite)
        .run();
}
```

Effects tick in mettle's schedule, [`Mettle`], in [`MettleSet::Effects`],
before attributes fold what they changed in the same step. It runs each
fixed step, in `FixedPostUpdate` unless [`MettleCorePlugin::new`] says
otherwise. Cues play in `PostUpdate`.

## Learn more

- The book's [Defining effects](https://declaredlost.github.io/bevy_mettle/effects.html),
  [Applying effects](https://declaredlost.github.io/bevy_mettle/applying-effects.html),
  [Stacking](https://declaredlost.github.io/bevy_mettle/stacking.html) and
  [Effects in code](https://declaredlost.github.io/bevy_mettle/effect-code.html)
  pages, [Cues](https://declaredlost.github.io/bevy_mettle/cues.html), and [Prediction](https://declaredlost.github.io/bevy_mettle/prediction.html)
  for the network
- The [API documentation](https://docs.rs/bevy_mettle_effect)

## Contributing

See [CONTRIBUTING.md](https://github.com/declaredlost/bevy_mettle/blob/main/CONTRIBUTING.md)
for development setup and guidelines.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-MIT) at your option.

[`Effect`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.Effect.html
[`EffectRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html
[`EffectData`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/struct.EffectData.html
[`Magnitude`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html
[`Snapshots`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.Snapshots.html
[`EffectSpec`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectSpec.html
[`EffectOf`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/active/struct.EffectOf.html
[`EffectApplied`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectApplied.html
[`EffectStacked`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectStacked.html
[`EffectBlocked`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectBlocked.html
[`EffectExecuted`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectExecuted.html
[`EffectRemoved`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectRemoved.html
[`Mettle`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/struct.Mettle.html
[`MettleSet::Effects`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/enum.MettleSet.html#variant.Effects
[`MettleCorePlugin::new`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/struct.MettleCorePlugin.html#method.new
