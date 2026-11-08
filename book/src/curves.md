# Curves

A curve gives a value by level. The mage's fireball hits harder as it levels
up: 10 damage at level 1, 85 at level 10, 240 at level 20. Effects scale
their numbers by curves, as Unreal's curve tables do.

## Defining a curve

A curve is data, a [`LevelCurve`]: an id, how it goes between its keys, and
its keys as `(level, value)` pairs, levels rising:

```ron
[
    (id: "fireball.damage", interpolation: Ease(QuadraticIn), keys: [(1, 10), (10, 85), (20, 240)]),
    (id: "burning.damage", keys: [(1, 4), (20, 40)]),
]
```

Between keys, a curve goes one of three ways:

| `interpolation` | Between two keys |
|---|---|
| `Linear` | A straight line from one to the next. The default. |
| `Constant` | Each key's value, held until the next. |
| `Ease(..)` | One of Bevy's easing functions, such as `Ease(SineInOut)` or `Ease(ExponentialIn)`, from one key to the next. |

Before the first key and past the last, a curve stays at that key's value. A
curve with one key is that value at every level.

## The curve table

In a game, the curves go in [`MettleContent`]'s `curves`, and mettle builds
the table as the content is loaded, keeping it as a resource,
`Res<CurveTable>`. [`CurveTable::build`] is that step on its own: it checks
each curve and builds it as one of Bevy's curves:

```rust
# use bevy_mettle::prelude::*;
let file = r#"[
    (id: "fireball.damage", interpolation: Ease(QuadraticIn), keys: [(1, 10), (10, 85), (20, 240)]),
    (id: "burning.damage", keys: [(1, 4), (20, 40)]),
]"#;
let definitions: Vec<LevelCurve> = ron::from_str(file).unwrap();
let curves = CurveTable::build(&definitions).unwrap();

let burning = Id::from_static("burning.damage");
assert_eq!(curves.at(&burning, 1.0), Some(4.0));
assert_eq!(curves.at(&burning, 50.0), Some(40.0)); // Past the last key.
assert_eq!(curves.at(&Id::from_static("frost.damage"), 1.0), None);
```

`build` reports every mistake together:

- **A curve defined twice.**
- **A curve with no keys.**
- **A key that isn't finite**, such as `NaN`.
- **Levels that don't rise.** Each key's level has to be above the one
  before it.

Effects name curves by id. As content is loaded, every curve an effect names
is checked against the table. [Defining
effects](effects.md) shows a magnitude scaled by a curve.

## As one of Bevy's curves

[`CurveTable::curve`] hands back the built curve itself, as Bevy's
[`Curve<f32>`]. Use it to draw a graph in your tooling, or with Bevy's curve
adaptors:

```rust
# use bevy_mettle::prelude::*;
# let curves = CurveTable::build(&[LevelCurve {
#     id: Id::from_static("burning.damage"),
#     interpolation: Interpolation::Linear,
#     keys: vec![(1.0, 4.0), (20.0, 40.0)],
# }])
# .unwrap();
let burning = curves.curve(&Id::from_static("burning.damage")).unwrap();
let graph: Vec<f32> = (1..=20).map(|level| burning.sample_clamped(level as f32)).collect();
assert_eq!(graph.len(), 20);
```

## Pitfalls

- **Levels are numbers, not steps.** A level can be 2.5. Each curve goes
  between its keys as it says, so a `Constant` curve is the way to get steps.
- **Easing goes from one key to the next.** `Ease(QuadraticIn)` starts slow
  after every key, not once across the whole curve.

[`LevelCurve`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.LevelCurve.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`CurveTable::build`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveTable.html#method.build
[`CurveTable::curve`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveTable.html#method.curve
[`Curve<f32>`]: https://docs.rs/bevy/latest/bevy/curve/trait.Curve.html
