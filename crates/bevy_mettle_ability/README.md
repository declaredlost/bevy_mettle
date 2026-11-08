# bevy_mettle_ability

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]

**Part of [bevy_mettle](https://github.com/declaredlost/bevy_mettle), a gameplay framework for Bevy.**

[action-badge]: https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_mettle/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/declaredlost/bevy_mettle/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/bevy_mettle_ability.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/bevy_mettle_ability
[docs-badge]: https://img.shields.io/docsrs/bevy_mettle_ability?logo=Docs.rs&style=flat-square
[docs-url]: https://docs.rs/bevy_mettle_ability

Abilities for Bevy, as in Unreal's Gameplay Ability System: what an actor
can do, granted to it, activated, paid for, cooled down and ended.

## Overview

- An [`Ability`] is data, named by an id: the behaviour that runs it, its
  cost and cooldown (effects), what it needs and what blocks it, what it
  cancels and blocks while active, the tags it grants, and what triggers
  it.
- A behaviour is a component of yours, `#[derive(Behaviour)]`, added with
  [`add_behaviour`]: put on the ability's entity as it activates, for your
  systems to run, and taken off as it ends.
- Its input is what the player chose, an aim or a target: given with
  `activate_ability_with`, or while it runs with `set_input`, checked by a
  system of yours before it's taken, and sent to the server with a
  prediction.
- An [`AbilityRegistry`] is built from them, checked against the tag tree,
  the effects and the behaviours: every error at once. A [`Grant`] says
  which abilities an effect grants while it lasts: gear's skills.
- `grant_ability` and `activate_ability` grant and activate it on an actor,
  checking it's granted, not cooling down, affordable, not blocked and its
  requirement holds. [`AbilityActivated`], [`AbilityFailed`] (why) and
  [`AbilityEnded`] are triggered on the actor.
- A [`GameplayEvent`] sent to an actor activates what it triggers.
- Over a network, a client predicts its own abilities and asks the server,
  which decides and replies.

## Installation

Most games depend on `bevy_mettle`, where abilities are a default feature
and this crate is `bevy_mettle::ability`:

```toml
[dependencies]
bevy_mettle = "0.1"
```

Or depend on this crate directly:

```toml
[dependencies]
bevy_mettle_ability = "0.1"
```

## Usage

```rust,no_run
use bevy::prelude::*;
use bevy_mettle_ability::prelude::*;
use bevy_mettle_attribute::prelude::*;
use bevy_mettle_core::prelude::*;
use bevy_mettle_effect::prelude::*;

#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0)]
struct Mana;

/// The fireball being cast: half a second, then it ends.
#[derive(Component, Default, Behaviour)]
struct CastFireball {
    timer: f32,
}

fn load(
    mut commands: Commands,
    attributes: Res<AttributeRegistry>,
    code: Res<EffectCode>,
    behaviours: Res<Behaviours>,
) {
    let tags: Vec<Tag> = ["cooldown", "cooldown.fireball"]
        .into_iter()
        .map(|id| Tag { id: Id::new(id) })
        .collect();
    let tags = TagTree::build(&tags).unwrap();
    let effects = [
        Effect::instant("fireball.cost")
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
        Effect::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
    ];
    let effects = EffectRegistry::check(
        &effects,
        EffectChecks {
            tags: &tags,
            curves: &CurveTable::default(),
            attributes: &attributes,
            code: &code,
        },
    )
    .unwrap();
    let fireball = Ability::new("fireball", "cast_fireball")
        .cost("fireball.cost")
        .cooldown("fireball.cooldown");
    let abilities = AbilityRegistry::check(
        &[fireball],
        &[],
        AbilityChecks {
            tags: &tags,
            effects: &effects,
            behaviours: &behaviours,
        },
    )
    .unwrap();
    commands.insert_resource(effects);
    commands.insert_resource(abilities);
}

fn spawn(mut commands: Commands) {
    commands
        .spawn(Base::<Mana>::default())
        .grant_ability("fireball", 1.0)
        .activate_ability("fireball");
}

/// Runs each cast until it ends.
fn cast(mut casts: Query<(Entity, &mut CastFireball)>, time: Res<Time>, mut commands: Commands) {
    for (ability, mut cast) in &mut casts {
        cast.timer += time.delta_secs();
        if cast.timer > 0.5 {
            commands.entity(ability).end_ability();
        }
    }
}

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, EffectPlugin::default(), AbilityPlugin))
        .add_attribute::<Mana>()
        .add_behaviour::<CastFireball>()
        .add_systems(Startup, (load, spawn).chain())
        .add_systems(Update, cast)
        .run();
}
```

Abilities are kept in [`AbilitySystems`], in `PreUpdate`: over a network, a
transport hands its messages over before it.

## Learn more

- The book's [Defining abilities](https://declaredlost.github.io/bevy_mettle/abilities.html),
  [Granting and activating](https://declaredlost.github.io/bevy_mettle/activating.html),
  [Input and targets](https://declaredlost.github.io/bevy_mettle/input.html)
  and [Grants and triggers](https://declaredlost.github.io/bevy_mettle/grants-triggers.html)
  pages, and [Prediction](https://declaredlost.github.io/bevy_mettle/prediction.html)
  for the network
- The [API documentation](https://docs.rs/bevy_mettle_ability)

## Contributing

See [CONTRIBUTING.md](https://github.com/declaredlost/bevy_mettle/blob/main/CONTRIBUTING.md)
for development setup and guidelines.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-MIT) at your option.

[`Ability`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/data/struct.Ability.html
[`add_behaviour`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/data/trait.BehaviourApp.html#tymethod.add_behaviour
[`AbilityRegistry`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilityRegistry.html
[`Grant`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/data/struct.Grant.html
[`AbilityActivated`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.AbilityActivated.html
[`AbilityFailed`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.AbilityFailed.html
[`AbilityEnded`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.AbilityEnded.html
[`GameplayEvent`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.GameplayEvent.html
[`AbilitySystems`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilitySystems.html
