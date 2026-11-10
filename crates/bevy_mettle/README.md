# bevy_mettle

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]
[![Book][book-badge]][book-url]

[action-badge]: https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_mettle/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/declaredlost/bevy_mettle/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/bevy_mettle.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/bevy_mettle
[docs-badge]: https://img.shields.io/docsrs/bevy_mettle?logo=Docs.rs&style=flat-square
[docs-url]: https://docs.rs/bevy_mettle
[book-badge]: https://img.shields.io/badge/book-guide-blue?logo=mdbook&style=flat-square
[book-url]: https://declaredlost.github.io/bevy_mettle/

A gameplay framework for [Bevy](https://bevy.org) in the spirit of Unreal's
Gameplay Ability System: attributes, effects and abilities.

## Overview

- **Attributes are your types**: `Life`, `Mana` and `Resistance` are marker
  types with their rules derived, and each actor keeps a `Base` and a
  `Current` of each as components.
- **Modifiers**: flat, increased, more, overrides and the rest, from items,
  effects or the actor itself, counting only in context where they say so:
  resistance against fire.
- **Effects**: instant, timed, infinite or periodic, their numbers small
  expressions of levels, curves and attributes, captured or read live;
  stacking, granting tags, needing conditions to apply or to count, and
  chaining into each other.
- **Abilities**: granted, activated, paid for, cooled down and ended; what
  one does is a component of yours, run by your systems, with input such as
  an aim or a target, checked on the server.
- **Gameplay tags**: a tree of labels for what effects and abilities are,
  and what an actor is going through.
- **Cues**: what's shown of effects and abilities, as entities while they
  last and one-shots as they execute, routed to the most specific handler,
  and replicated to every client that sees the actor.
- **Data first**: tags, curves, effects and abilities are data, handed over
  as one `MettleContent` resource and checked as a whole; change it and it's
  reloaded, or rejected with every error while the content before stays.
  Mettle doesn't load files: your game reads them.
- **Prediction**: a client predicts its own abilities and the server
  decides, with an adapter for lightyear.

## Installation

```toml
[dependencies]
bevy_mettle = "0.1"
```

Applications depend on `bevy_mettle`. Each part is a crate of its own,
re-exported here behind a feature: `bevy_mettle::attribute`, `effect`,
`ability` and `lightyear`, with `bevy_mettle::core` always there. The
prelude brings in what using the enabled parts takes, and `MettlePlugins`
adds their plugins.

`attribute`, `effect` and `ability` are enabled by default. The rest are
listed on the book's [Feature
flags](https://declaredlost.github.io/bevy_mettle/features.html) page.

## Usage

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

/// The fireball, as code names it: content defines it as `"fireball"`.
#[derive(Ability)]
struct Fireball;

/// The game's content: in a game, read from its files.
fn content() -> MettleContent {
    MettleContent {
        effects: vec![EffectData::instant("fireball.cost")
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0))],
        abilities: vec![AbilityData::new("fireball", "cast_fireball").cost("fireball.cost")],
        ..default()
    }
}

fn spawn(mut commands: Commands) {
    commands
        .spawn(Base::<Mana>::default())
        .grant_ability(Fireball)
        .activate_ability(Fireball);
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
        .add_ability::<Fireball>()
        .insert_resource(content())
        .add_systems(Startup, spawn)
        .add_systems(Update, cast)
        .run();
}
```

Content inserted as the app is built is checked and loaded in `PreStartup`;
changed later, it's loaded again in the next `PreUpdate`, or rejected with
every error while the content before stays. Effects and attributes run in
mettle's own schedule, `Mettle`, each fixed step in `FixedPostUpdate`:
effects ticked, attributes folded, live reads settled, values published.
Abilities handle network messages in `PreUpdate`. Systems that read
attributes as they stand go after `MettleSystems`.

## Learn more

[The bevy_mettle book](https://declaredlost.github.io/bevy_mettle/) explains every part:

- **Getting started**: [installation](https://declaredlost.github.io/bevy_mettle/installation.html),
  [your first attributes](https://declaredlost.github.io/bevy_mettle/first-attributes.html), and
  [how it fits together](https://declaredlost.github.io/bevy_mettle/concepts.html)
- **Tags, curves and conditions**: [gameplay tags](https://declaredlost.github.io/bevy_mettle/tags.html),
  [curves](https://declaredlost.github.io/bevy_mettle/curves.html), and
  [conditions](https://declaredlost.github.io/bevy_mettle/conditions.html)
- **Attributes**: [attributes](https://declaredlost.github.io/bevy_mettle/attributes.html),
  [modifiers](https://declaredlost.github.io/bevy_mettle/modifiers.html), and
  [reading in context](https://declaredlost.github.io/bevy_mettle/reading.html)
- **Effects**: [defining effects](https://declaredlost.github.io/bevy_mettle/effects.html),
  [applying them](https://declaredlost.github.io/bevy_mettle/applying-effects.html),
  [stacking](https://declaredlost.github.io/bevy_mettle/stacking.html), and
  [effects in code](https://declaredlost.github.io/bevy_mettle/effect-code.html)
- **Abilities**: [defining abilities](https://declaredlost.github.io/bevy_mettle/abilities.html),
  [granting and activating](https://declaredlost.github.io/bevy_mettle/activating.html),
  [input and targets](https://declaredlost.github.io/bevy_mettle/input.html),
  [cues](https://declaredlost.github.io/bevy_mettle/cues.html), and
  [grants and triggers](https://declaredlost.github.io/bevy_mettle/grants-triggers.html)
- **Multiplayer**: [prediction](https://declaredlost.github.io/bevy_mettle/prediction.html), and
  [over lightyear](https://declaredlost.github.io/bevy_mettle/lightyear.html)
- **Putting it together**: [loading data](https://declaredlost.github.io/bevy_mettle/loading-data.html),
  and [testing](https://declaredlost.github.io/bevy_mettle/testing.html)

The API reference is on [docs.rs](https://docs.rs/bevy_mettle).

## Contributing

See [CONTRIBUTING.md](https://github.com/declaredlost/bevy_mettle/blob/main/CONTRIBUTING.md)
for development setup and guidelines.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-MIT) at your option.
