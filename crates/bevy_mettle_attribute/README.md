# bevy_mettle_attribute

`bevy_mettle`'s attributes, as in Unreal's Gameplay Ability System: each
attribute a marker type, its rules derived on it; each actor's a base
(what the game and effects set) and a current value (with modifiers),
as components.

```rust
use bevy::prelude::*;
use bevy_mettle_attribute::prelude::*;

#[derive(Attribute)]
#[attribute(base = 50.0, min = 0.0, max = MaxLife)]
pub struct Life;

#[derive(Attribute)]
#[attribute(base = 50.0)]
pub struct MaxLife;

#[derive(Attribute)]
#[attribute(max = MaxResistance)]
pub struct Resistance;

#[derive(Attribute)]
#[attribute(base = 0.75)]
pub struct MaxResistance;

fn build(app: &mut App) {
    app.add_attribute::<Life>()
        .add_attribute::<MaxLife>()
        .add_attribute::<Resistance>()
        .add_attribute::<MaxResistance>();
}

fn spawn(mut commands: Commands) {
    let hero = commands
        .spawn((
            Base::<Life>::default(),
            Base::<MaxLife>::default(),
            Base::<Resistance>::default(),
            Base::<MaxResistance>::default(),
        ))
        .id();
    // A ring it wears:
    commands.spawn((
        ModifierOf(hero),
        Modifiers(vec![Modifier::flat::<Resistance>(0.2).read_tagged("damage.elemental")]),
    ));
}

fn health_bars(lives: Query<&Current<Life>, Changed<Current<Life>>>) {
    for life in &lives {
        let _ = life.get();
    }
}
```

At the root: [`Attribute`] (and its derive), [`AttributePlugin`],
[`AttributeSystems`], [`AttributeApp`], [`Base`] and [`Current`]. The rest is
in three modules, and [`prelude`] brings in what using attributes takes:

| Module | What's in it |
|---|---|
| [`modifier`] | `Modifier`, `Modifiers` and the sources they sit on, `Operation`, `Per` |
| [`read`] | Reads in context: `AttributeContext`, `AttributeReader`, `AttributeWorld`, `Breakdown`, guards |
| [`registry`] | The untyped layer: `AttributeRegistry`, `AnyAttribute`, `Limit`, `Rounding`, `AttributeState`, `AttributeChanged` |

## Attributes

An attribute is a marker type: [`#[derive(Attribute)]`](Attribute), its
rules on `#[attribute(..)]`.

| Setting | Meaning | Unless it says |
|---|---|---|
| `base = 50.0` | What its base starts at | `0.0` |
| `min = 0.0`, `min = Other` | The least it can be: a value, or another attribute's current value | none |
| `max = ..` | The most it can be, as `min` | none |
| `rounding = Down`, `Up`, `Nearest` | A whole number, rounded so | none |
| `name = ".."` | The name data names it by | its type's, in `snake_case` |

## Kinds of a value

An attribute that differs by kind, a resistance per damage type, is one
attribute: its modifiers count for reads tagged so, and a hit reads it
tagged with its damage type. Tags are a tree, so a modifier for
`damage.elemental` counts for a fire hit; a limit is read in the same
context, so fire resistance is capped by maximum fire resistance. A mod
adds a damage type by adding its tag. This is Unreal's way: modifiers
with tag requirements, weighed as the attribute is evaluated.

```rust,ignore
// +20% elemental resistance, +5% maximum fire resistance:
Modifier::flat::<Resistance>(0.2).read_tagged("damage.elemental");
Modifier::flat::<MaxResistance>(0.05).read_tagged("damage.elemental.fire");
// A fire hit:
let fire = AttributeContext::new().tag("damage.elemental.fire");
let resisted = attributes.value::<Resistance>(target, &fire);
```

[`add_attribute`](AttributeApp::add_attribute) adds one to the app: data
can name it, and actors with its [`Base`] have its [`Current`] kept.

## Base and Current

- [`Base<A>`]: the base, the game's and effects' to set (what's left of
  life, a level gained). Having it is having the attribute; it brings
  `Current<A>`. Taking it off takes the attribute away, `Current<A>` too.
- [`Current<A>`]: the value with every modifier that counts with nothing
  in play, kept within its limits; the base until it's first worked out.
  Written only when it changes, so `Changed<Current<Life>>` is life
  changing.
- [`AttributeChanged<A>`](registry::AttributeChanged): an entity event as its current value changes
  (what it was, what it is): `On<AttributeChanged<Life>>`. Its first value
  worked out isn't a change; what only some reads see (a fire hit's
  resistance) isn't told.

Two attribute types can't share a name: adding the second panics. The
components are reflected and registered, for inspectors and scenes.

## Modifiers

An attribute is

```text
((base + Σ flat) × (1 + Σ additive) × Π (1 + multiplicative) / (1 + Σ divide)) + Σ unscaled
```

or its override, then rounded and kept within its limits (the limits
win). Of `Base`s and `Override`s, the last counts: in the actor's own
modifiers, then each source's, in the order the sources came. That order
is the world's own: a client can have replicated sources in another
order than the server, so don't have sources' overrides compete where
clients have to agree.

| Op | What it does |
|---|---|
| `Base` | Replaces the base; the last counts |
| `Flat` | Added to the base |
| `Additive` | A fraction, summed with the others of its kind: +20% and +30% are +50% ("increased") |
| `Multiplicative` | A fraction, multiplied in on its own: +20% and +30% are +56% ("more") |
| `Divide` | A fraction, summed, then divided by: +100% halves it, more never reaches 0 (cooldown recovery); -99% or less divides by 0.01 |
| `Unscaled` | Added after everything, scaled by nothing |
| `Override` | The value, whatever else there is; the last counts |

`Modifier::flat::<Damage>(5.0)`. A modifier can count while a `Condition`
holds, and as much as it does (`read_tagged`, `owner_has`);
`per::<Strength>(10.0)`, once for each whole 10 (a multiplicative one
multiplied in each time; a `Base` or `Override` can't be per anything);
or `local()`, only for reads naming its source. One whose numbers can't
be, its condition's too, is skipped with a warning.
In data, by name:

```ron
(attribute: "resistance", op: Flat, value: 0.1, condition: ReadTagged("damage.elemental"))
```

[`Modifiers`] sit on whatever holds them (an item, an active effect, the
actor itself), counting for the actor they're [`ModifierOf`].

## Owned tags

A modifier can count while its owner owns a tag (`owner_has`): the
actor's [`OwnedTags`](bevy_mettle_core::tag::OwnedTags), which are
core's, since effects and abilities share them. The attribute plugin
tells them coming and going (`TagAdded`, `TagRemoved`) in
[`AttributeSystems`], before values are worked out.

## Reading in context

[`AttributeReader`] reads in an [`AttributeContext`](read::AttributeContext): the read's tags (a
hit's), the target's owned tags (`target_tags`, `target_tag`), the
source's (`source_tags`, `source_tag`), the facts it supplies (numbers
named by tags: `fact.flown`), and the source involved (`source`).

```rust,ignore
fn strike(attributes: AttributeReader) {
    let context = AttributeContext::new().tag("delivery.attack").fact("fact.flown", 7.0);
    let damage = attributes.value::<Damage>(caster, &context);
    let fire = AttributeContext::new().tag("damage.elemental.fire");
    let resisted = attributes.value::<Resistance>(target, &fire);
    let parts = attributes.breakdown::<Damage>(caster, &context);
}
```

[`AttributeWorld`] does the same on the world, and changes a base as an
instant effect does, kept within its limits.

## Guards

[`read`]: `attribute_above::<A>` and `attribute_below::<A>`, for
`bevy_arbor`; the one on owned tags, `owns`, is core's.

[`AttributeReader`]: read::AttributeReader
[`AttributeWorld`]: read::AttributeWorld
[`ModifierOf`]: modifier::ModifierOf
[`Modifiers`]: modifier::Modifiers
