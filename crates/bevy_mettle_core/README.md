# bevy_mettle_core

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]

**Part of [bevy_mettle](https://github.com/declaredlost/bevy_mettle), a gameplay framework for Bevy.**

[action-badge]: https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_mettle/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/declaredlost/bevy_mettle/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/bevy_mettle_core.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/bevy_mettle_core
[docs-badge]: https://img.shields.io/docsrs/bevy_mettle_core?logo=Docs.rs&style=flat-square
[docs-url]: https://docs.rs/bevy_mettle_core

What bevy_mettle's parts share: gameplay tags and the tags an actor owns,
curves, the condition language, and the registries everything is numbered in.

## Overview

- A [`Key<K>`] names what content defines: a tag, a curve, an effect, of a
  [`Kind`] such as `AnyEffect`. Checked, it's known by its number, an
  [`Id<K>`], in a [`Registry`]: entries by number, numbers by name and by
  type, as Bevy keeps components.
- A [`TagRegistry`] is the gameplay tags there are, from [`Tag`]s in data, each a
  path such as `damage.elemental.fire`. What has a tag has every tag above
  it: a rule on `damage.elemental` holds for fire.
- [`Tags`] is a set of them, what an effect or an ability is labelled with;
  [`OwnedTags`] is what an actor owns by count, as its states come and go,
  told by [`TagAdded`] and [`TagRemoved`].
- A [`CurveRegistry`] is values by level, from [`CurveData`]s in data, built as
  Bevy's curves: what effects scale by.
- A [`Condition`] is an expression over a read's tags, what its owner,
  target and source own, and the facts it supplies, weighed from 0 to 1.
- Building any of them checks it, and returns every [`Error`] at once.

Its only ECS is the owned tags, with `bevy_ecs` and no app.

## Installation

Most games depend on `bevy_mettle`, where this crate is always there, as
`bevy_mettle::core`:

```toml
[dependencies]
bevy_mettle = "0.1"
```

Or depend on this crate directly, for tags, curves and conditions alone:

```toml
[dependencies]
bevy_mettle_core = "0.1"
```

## Usage

```rust
use bevy_mettle_core::prelude::*;

// Tags code names, as types: a misspelled one doesn't compile.
#[derive(Tag)]
struct Damage;
#[derive(Tag)]
#[tag(parent = Damage)]
struct Elemental;
#[derive(Tag)]
#[tag(parent = Elemental)]
struct Fire;

// The tags there are: those content names, with those code names.
let content = ["damage", "damage.elemental", "damage.cold"].map(TagKey::named);
let tree = TagRegistry::build(content.into_iter().chain([Fire.into()])).unwrap();
assert!(tree.contains(Fire));
assert!(tree.contains("damage.cold"));

// A fire hit is elemental too.
let mut hit = Tags::new();
hit.insert(Fire);
assert!(hit.contains(Elemental));

// Damage by level: 10 at level 1, 110 at level 11, linearly between.
let curves = CurveRegistry::build(&[CurveData {
    name: Key::new("fireball.damage"),
    interpolation: Interpolation::Linear,
    keys: vec![(1.0, 10.0), (11.0, 110.0)],
}])
.unwrap();
let damage = curves.id("fireball.damage").unwrap();
assert_eq!(curves.at(damage, 6.0), Some(60.0));

// A condition, checked against the tree.
let elemental = Condition::read_tagged(Elemental);
elemental.check(&tree, None).unwrap();
```

## Learn more

- The book's [Gameplay tags](https://declaredlost.github.io/bevy_mettle/tags.html),
  [Curves](https://declaredlost.github.io/bevy_mettle/curves.html) and
  [Conditions](https://declaredlost.github.io/bevy_mettle/conditions.html) pages
- The [API documentation](https://docs.rs/bevy_mettle_core)

## Contributing

See [CONTRIBUTING.md](https://github.com/declaredlost/bevy_mettle/blob/main/CONTRIBUTING.md)
for development setup and guidelines.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-MIT) at your option.

[`Key<K>`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/struct.Key.html
[`Id<K>`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/struct.Id.html
[`Registry`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/struct.Registry.html
[`Kind`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/trait.Kind.html
[`TagRegistry`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRegistry.html
[`Tag`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/trait.Tag.html
[`Tags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.Tags.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
[`TagAdded`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagAdded.html
[`TagRemoved`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRemoved.html
[`CurveRegistry`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html
[`CurveData`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveData.html
[`Condition`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/condition/type.Condition.html
[`Error`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/enum.Error.html
