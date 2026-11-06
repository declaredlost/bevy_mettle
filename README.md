<div align="center">

# bevy_mettle

**A gameplay framework for Bevy in the spirit of Unreal's Gameplay Ability System.**

[![Build](https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_mettle/build.yml?branch=main&label=build&style=flat-square)](https://github.com/declaredlost/bevy_mettle/actions/workflows/build.yml)
[![Security](https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_mettle/security.yml?branch=main&label=security&style=flat-square)](https://github.com/declaredlost/bevy_mettle/actions/workflows/security.yml)

[**github.com/declaredlost**](https://github.com/declaredlost)

</div>

Attributes, effects and abilities, data first like GAS: attributes are the
game's own types, each a marker with its rules derived; content (gameplay
tags, curves, effects, abilities) is data, named by tags and ids, and checked
as registries are built from it. Prediction is GAS-style, with an adapter for
[lightyear](https://github.com/cBournhonesque/lightyear).

```rust,ignore
use bevy_mettle::prelude::*;

#[derive(Attribute)]
#[attribute(base = 50.0, min = 0.0, max = MaxLife)]
pub struct Life;

#[derive(Attribute)]
#[attribute(base = 50.0)]
pub struct MaxLife;

app.add_attribute::<Life>()
    .add_attribute::<MaxLife>()
    .add_plugins((EffectPlugin::default(), AbilityPlugin));

commands.spawn((Base::<Life>::default(), Base::<MaxLife>::default()));
commands.entity(hero).apply_effect(EffectSpec::new("burning").param("param.damage", 12.0));
```

> [!WARNING]
> **Early development.** The API may change at any time.

## Crates

| Crate | Description |
|---|---|
| [`bevy_mettle`](crates/bevy_mettle) | The facade over the crates below, each behind a feature |
| [`bevy_mettle_core`](crates/bevy_mettle_core) | Gameplay tags and owned tags, curve tables and the condition language |
| [`bevy_mettle_attribute`](crates/bevy_mettle_attribute) | Attributes: marker types, `Base` and `Current`, modifiers, reads in context |
| [`bevy_mettle_effect`](crates/bevy_mettle_effect) | Effects: instant, timed, infinite, periodic; stacking, granted tags, immunity |
| [`bevy_mettle_ability`](crates/bevy_mettle_ability) | Abilities: granted, activated, paid for, cooled down; behaviours in code |
| [`bevy_mettle_lightyear`](crates/bevy_mettle_lightyear) | `bevy_mettle` over lightyear: replication, ability requests and replies, checked |
| [`bevy_mettle_macros`](crates/bevy_mettle_macros) | `bevy_mettle`'s derives: `IdTarget` and `Attribute` |

## Requirements

- **Rust + Cargo**: 1.99+, Edition 2024
- **Bevy**: 0.20; **lightyear**: 0.31, for `bevy_mettle_lightyear`

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines, and
[CHANGELOG.md](CHANGELOG.md) for release notes.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

