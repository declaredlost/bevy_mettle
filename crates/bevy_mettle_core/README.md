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
curve tables, and the condition language.

## Overview

- An [`Id<T>`] names what's defined in data: a tag, a curve, an effect. What
  it can name derives [`IdTarget`].
- A [`TagTree`] is the gameplay tags there are, from [`Tag`]s in data, each a
  path such as `damage.elemental.fire`. What has a tag has every tag above
  it: a rule on `damage.elemental` holds for fire.
- [`Tags`] is a set of them, what an effect or an ability is labelled with;
  [`OwnedTags`] is what an actor owns by count, as its states come and go,
  told by [`TagAdded`] and [`TagRemoved`].
- A [`CurveTable`] is values by level, from [`LevelCurve`]s in data, built as
  Bevy's curves: what effects scale by.
- A [`Condition`] is an expression over a read's tags, what its owner,
  target and source own, and the facts it supplies, weighed from 0 to 1.
- Building any of them checks it, and returns every [`MettleError`] at once.

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

// The tags there are, read from data.
let tags: Vec<Tag> = ron::from_str(
    r#"[(id: "damage"), (id: "damage.elemental"), (id: "damage.elemental.fire")]"#,
)
.unwrap();
let tree = TagTree::build(&tags).unwrap();
assert!(tree.contains(&Id::new("damage.elemental.fire")));

// A fire hit is elemental too.
let mut hit = Tags::new();
hit.insert(Id::new("damage.elemental.fire"));
assert!(hit.contains(&Id::new("damage.elemental")));

// Damage by level: 10 at level 1, 110 at level 11, linearly between.
let curves = CurveTable::build(&[LevelCurve {
    id: Id::new("fireball.damage"),
    interpolation: Interpolation::Linear,
    keys: vec![(1.0, 10.0), (11.0, 110.0)],
}])
.unwrap();
assert_eq!(curves.at(&Id::new("fireball.damage"), 6.0), Some(60.0));

// A condition, checked against the tree.
let elemental = Condition::read_tagged("damage.elemental");
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

[`Id<T>`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/struct.Id.html
[`IdTarget`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/trait.IdTarget.html
[`TagTree`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagTree.html
[`Tag`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.Tag.html
[`Tags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.Tags.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
[`TagAdded`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagAdded.html
[`TagRemoved`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRemoved.html
[`CurveTable`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveTable.html
[`LevelCurve`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.LevelCurve.html
[`Condition`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/condition/enum.Condition.html
[`MettleError`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/enum.MettleError.html
