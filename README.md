<div align="center">

# bevy_mettle

**A gameplay framework for Bevy in the spirit of Unreal's Gameplay Ability System.**

Attributes, effects and abilities, with content as data and prediction over
the network.

[![Build](https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_mettle/build.yml?branch=main&label=build&style=flat-square)](https://github.com/declaredlost/bevy_mettle/actions/workflows/build.yml)
[![Security](https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_mettle/security.yml?branch=main&label=security&style=flat-square)](https://github.com/declaredlost/bevy_mettle/actions/workflows/security.yml)
[![Crates.io](https://img.shields.io/crates/v/bevy_mettle.svg?style=flat-square)](https://crates.io/crates/bevy_mettle)
[![Docs](https://img.shields.io/docsrs/bevy_mettle?style=flat-square)](https://docs.rs/bevy_mettle)

[**The bevy_mettle book**](https://declaredlost.github.io/bevy_mettle/) · [**docs.rs/bevy_mettle**](https://docs.rs/bevy_mettle) · [**crates.io/crates/bevy_mettle**](https://crates.io/crates/bevy_mettle)

</div>

Attributes are your game's own types, each a marker with its rules derived,
kept on each actor as a `Base` and a `Current`. Effects change them, grant
tags and stack; abilities are what an actor can do, paid for with effects.
Everything but attributes is data, as in GAS: gameplay tags, curves,
effects and abilities are named by tags and ids, handed over as one
`MettleContent` resource, and checked as a whole: change it and it's
reloaded, or rejected with every error while the content before stays.
Mettle doesn't load anything: your game reads its files. A client predicts its own abilities
and the server decides, with an adapter for
[lightyear](https://github.com/cBournhonesque/lightyear).

> [!WARNING]
> **Early development.** The API may change at any time.

## Features

- **Attributes are your types**: `#[derive(Attribute)]` with a base, limits
  (numbers or other attributes) and rounding; `Current<Life>` changing is
  `Changed<Current<Life>>`.
- **Modifiers**: flat, increased, more, divided, unscaled, base and override,
  from items, effects or the actor itself, each counting while a condition
  holds, per so much of another attribute, or only for its own source's
  reads.
- **Reads in context**: one attribute per kind of value, a resistance read
  tagged with a hit's damage type, with a breakdown of where it came from.
- **Effects**: instant, timed, infinite or periodic, scaled by curves and
  levels; stacking, granting tags, immunity, requirements to apply and to
  count, executions in code, and chained effects.
- **Abilities**: granted, activated, paid for, cooled down and ended, by
  hand or by triggers; what blocks and cancels what is data, what one does
  is your component and systems.
- **Input and targets**: an aim, a target or a charge, given as an ability
  activates or while it runs, checked by your systems, predicted on the
  client and checked again on the server.
- **Gameplay tags**: a tree of labels, owned by actors by count, told as they
  come and go.
- **Cues**: what's shown of effects and abilities, lasting ones as entities
  to hang your effects on, one-shots with what they dealt, each run by the
  handler for its most specific tag.
- **Content as data**: one resource of definitions, checked as a whole and
  reloaded as it changes; your game reads the files, mettle checks them.
- **Prediction**: a client activates at once and the server confirms or
  rejects, with effects and attributes shown as the server serves them;
  effects go to their owner alone, and everyone else sees the cues.

## Requirements

- **Rust + Cargo**: 1.99+, Edition 2024
- **Bevy**: 0.20; **lightyear**: 0.31, for the `lightyear` feature

## Quick start

Add bevy_mettle:

```toml
[dependencies]
bevy_mettle = "0.1"
```

A mage whose fireball costs mana:

```rust,no_run
use bevy::prelude::*;
use bevy_mettle::prelude::*;

#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0)]
struct Mana;

/// What the fireball does while it's cast.
#[derive(Component, Default, Behaviour)]
struct CastFireball;

/// The game's content: in a game, read from its files.
fn content() -> MettleContent {
    MettleContent {
        effects: vec![Effect::instant("fireball.cost")
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0))],
        abilities: vec![Ability::new("fireball", "cast_fireball").cost("fireball.cost")],
        ..default()
    }
}

fn spawn(mut commands: Commands) {
    commands
        .spawn(Base::<Mana>::default())
        .grant_ability("fireball", 1.0)
        .activate_ability("fireball");
}

/// Runs each cast: here, it ends at once.
fn cast(casts: Query<Entity, Added<CastFireball>>, mut commands: Commands) {
    for cast in &casts {
        commands.entity(cast).end_ability();
    }
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MettlePlugins))
        .add_attribute::<Mana>()
        .add_behaviour::<CastFireball>()
        .insert_resource(content())
        .add_systems(Startup, spawn)
        .add_systems(Update, cast)
        .run();
}
```

[The bevy_mettle book](https://declaredlost.github.io/bevy_mettle/) is the guide: attributes and modifiers,
effects, abilities, tags and prediction, one fire mage built up chapter by
chapter. See its [Installation](https://declaredlost.github.io/bevy_mettle/installation.html) and
[Feature flags](https://declaredlost.github.io/bevy_mettle/features.html) pages for configuration.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines, and
[CHANGELOG.md](CHANGELOG.md) for release notes.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
