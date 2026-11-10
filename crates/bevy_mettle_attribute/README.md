# bevy_mettle_attribute

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]

**Part of [bevy_mettle](https://github.com/declaredlost/bevy_mettle), a gameplay framework for Bevy.**

[action-badge]: https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_mettle/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/declaredlost/bevy_mettle/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/bevy_mettle_attribute.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/bevy_mettle_attribute
[docs-badge]: https://img.shields.io/docsrs/bevy_mettle_attribute?logo=Docs.rs&style=flat-square
[docs-url]: https://docs.rs/bevy_mettle_attribute

Attributes for Bevy, as in Unreal's Gameplay Ability System: each a marker
type with its rules derived, each actor's kept as a base and a current
value, with modifiers.

## Overview

- An attribute is a marker type with [`#[derive(Attribute)]`][`Attribute`]:
  its base, its limits (numbers, or other attributes) and its rounding.
  [`add_attribute`] adds it to the app.
- An actor has an attribute when it has its [`Base`], the value the game
  and effects set. Its [`Current`] is the value with every modifier counted,
  kept within its limits, and written only when it changes;
  [`AttributeChanged`] is triggered as it does.
- [`Modifiers`] sit on whatever holds them, an item, an effect or the actor,
  counting for the actor they're [`ModifierOf`]: flat, increased, more,
  divided, unscaled, base or override, each counting while a condition
  holds, per so much of another attribute, or only for its own source.
- An [`AttributeReader`] reads values in context: a fire hit's resistance,
  counting modifiers for `damage.elemental`, with a breakdown of where it
  came from.
- Over a network, an actor shows the values its server serves
  ([`Served`]), but its own while it predicts.

## Installation

Most games depend on `bevy_mettle`, where attributes are a default feature
and this crate is `bevy_mettle::attribute`:

```toml
[dependencies]
bevy_mettle = "0.1"
```

Or depend on this crate directly to use attributes alone:

```toml
[dependencies]
bevy_mettle_attribute = "0.1"
```

## Usage

```rust,no_run
use bevy::prelude::*;
use bevy_mettle_attribute::prelude::*;
use bevy_mettle_core::prelude::*;

#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0, max = MaxLife)]
struct Life;

#[derive(Attribute)]
#[attribute(base = 100.0)]
struct MaxLife;

#[derive(Attribute)]
#[attribute(max = MaxResistance)]
struct Resistance;

#[derive(Attribute)]
#[attribute(base = 0.75)]
struct MaxResistance;

#[derive(Tag)]
struct Damage;

#[derive(Tag)]
#[tag(parent = Damage)]
struct Elemental;

#[derive(Tag)]
#[tag(parent = Elemental)]
struct Fire;

fn spawn(mut commands: Commands) {
    let mage = commands
        .spawn((
            Base::<Life>::default(),
            Base::<MaxLife>::default(),
            Base::<Resistance>::default(),
            Base::<MaxResistance>::default(),
        ))
        .id();
    // A ring it wears: +20% resistance to elemental damage.
    commands
        .spawn(ModifierOf(mage))
        .modifiers([ModifierSpec::flat::<Resistance>(0.2).read_tagged(Elemental)]);
}

/// What a fire hit is resisted by.
fn resist(mages: Query<Entity, With<Base<Life>>>, attributes: AttributeReader) {
    let fire = AttributeContext::new().tag(Fire);
    for mage in &mages {
        let _resisted = attributes.value::<Resistance>(mage, &fire);
    }
}

fn health_bars(lives: Query<&Current<Life>, Changed<Current<Life>>>) {
    for life in &lives {
        info!("life: {}", life.get());
    }
}

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, AttributePlugin))
        .add_attribute::<Life>()
        .add_attribute::<MaxLife>()
        .add_attribute::<Resistance>()
        .add_attribute::<MaxResistance>()
        .add_systems(Startup, spawn)
        .add_systems(FixedUpdate, resist)
        .add_systems(FixedPostUpdate, health_bars.after(MettleSystems))
        .run();
}
```

Attributes are folded in mettle's schedule, [`Mettle`], in
[`MettleSet::Attributes`], and published in [`MettleSet::Publish`]. It runs
each fixed step, in `FixedPostUpdate` unless [`MettleCorePlugin::new`] says
otherwise. Order systems that read `Current` after [`MettleSystems`].

## Learn more

- The book's [Your first attributes](https://declaredlost.github.io/bevy_mettle/first-attributes.html),
  [Attributes](https://declaredlost.github.io/bevy_mettle/attributes.html),
  [Modifiers](https://declaredlost.github.io/bevy_mettle/modifiers.html) and
  [Reading in context](https://declaredlost.github.io/bevy_mettle/reading.html)
  pages, and [Prediction](https://declaredlost.github.io/bevy_mettle/prediction.html)
  for served values
- The [API documentation](https://docs.rs/bevy_mettle_attribute)

## Contributing

See [CONTRIBUTING.md](https://github.com/declaredlost/bevy_mettle/blob/main/CONTRIBUTING.md)
for development setup and guidelines.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-MIT) at your option.

[`Attribute`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/trait.Attribute.html
[`add_attribute`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/trait.AttributeApp.html#tymethod.add_attribute
[`Base`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.Base.html
[`Current`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.Current.html
[`AttributeChanged`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/struct.AttributeChanged.html
[`Modifiers`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Modifiers.html
[`ModifierOf`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.ModifierOf.html
[`AttributeReader`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/read/struct.AttributeReader.html
[`Served`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/authority/struct.Served.html
[`Mettle`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/struct.Mettle.html
[`MettleSet::Attributes`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/enum.MettleSet.html#variant.Attributes
[`MettleSet::Publish`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/enum.MettleSet.html#variant.Publish
[`MettleCorePlugin::new`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/struct.MettleCorePlugin.html#method.new
[`MettleSystems`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/struct.MettleSystems.html
