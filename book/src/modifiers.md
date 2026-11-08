# Modifiers

A modifier changes one attribute: +5 spell power from a ring, +20% mana
from a passive, a fire resistance an effect grants. Modifiers sit on
sources (an item, an effect, the actor itself) and count for one actor.
Mettle folds them into the attribute's current value.

The mage's ring:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 10.0)] struct SpellPower;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# let mut app = App::new();
# app.add_plugins(MettlePlugins.set(AttributePlugin::new(Update)))
#     .add_attribute::<SpellPower>()
#     .add_attribute::<Mana>();
# let world = app.world_mut();
let mage = world
    .spawn((Base::<SpellPower>::default(), Base::<Mana>::default()))
    .id();
let ring = world
    .spawn((
        ModifierOf(mage),
        Modifiers(vec![
            Modifier::flat::<SpellPower>(5.0),
            Modifier::additive::<Mana>(0.2),
        ]),
    ))
    .id();
# app.update();
# let world = app.world();
assert_eq!(world.get::<Current<SpellPower>>(mage).unwrap().get(), 15.0);
let mana = world.get::<Current<Mana>>(mage).unwrap().get();
assert!((mana - 120.0).abs() < 1e-4);
# let _ = ring;
```

## Sources

[`Modifiers`] is a list of modifiers on whatever holds them. On a source,
they count for the actor its [`ModifierOf`] names. On the actor itself,
they count for itself: a passive, a class bonus.

Equipping the ring is adding `ModifierOf(mage)` to it. Taking the ring off
(removing `ModifierOf`) or despawning it takes its modifiers away. The
actor's [`ModifierSources`] lists its sources. Mettle keeps it; don't set it
by hand.

Effects are sources too. A lasting effect carries its modifiers this way
while it counts. [Applying effects](applying-effects.md) shows that.

## How they add up

An attribute is

```text
((base + Σ flat) × (1 + Σ additive) × Π (1 + multiplicative) / (1 + Σ divide)) + Σ unscaled
```

unless something overrides it. Then it's rounded, if it rounds, and kept
within its limits. The limits always win.

| Operation | Builder | What it does |
|---|---|---|
| `Base` | `Modifier::base` | Replaces the base. The one applied last counts |
| `Flat` | `Modifier::flat` | Added to the base |
| `Additive` | `Modifier::additive` | A fraction, summed with the others of its kind: +20% and +30% are +50% ("increased") |
| `Multiplicative` | `Modifier::multiplicative` | A fraction, multiplied in on its own: +20% and +30% are +56% ("more") |
| `Divide` | `Modifier::divide` | A fraction, summed, then divided by: +100% halves it, and more never reaches 0 (cooldown recovery) |
| `Unscaled` | `Modifier::unscaled` | Added after everything else, scaled by nothing |
| `Override` | `Modifier::overriding` | The value, whatever else there is. The one applied last counts |

Fractions are written as fractions: `0.2` is +20%, `-0.2` is -20%. Amounts
(`Base`, `Flat`, `Unscaled`, `Override`) are written as amounts.

Here they are all at once on the mage's spell power:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 10.0)] struct SpellPower;
# let mut app = App::new();
# app.add_plugins(MettlePlugins.set(AttributePlugin::new(Update)))
#     .add_attribute::<SpellPower>();
# let world = app.world_mut();
let mage = world
    .spawn((
        Base::<SpellPower>::default(),
        Modifiers(vec![
            Modifier::flat::<SpellPower>(5.0),
            Modifier::additive::<SpellPower>(0.2),
            Modifier::additive::<SpellPower>(0.3),
            Modifier::multiplicative::<SpellPower>(0.2),
            Modifier::multiplicative::<SpellPower>(0.3),
            Modifier::divide::<SpellPower>(1.0),
            Modifier::unscaled::<SpellPower>(2.0),
        ]),
    ))
    .id();
# app.update();
// ((10 + 5) × 1.5 × 1.2 × 1.3 / 2) + 2
let power = app.world().get::<Current<SpellPower>>(mage).unwrap().get();
assert!((power - 19.55).abs() < 1e-4);
```

Two edges are clamped. Additive below -100% leaves nothing, not less than
nothing. A divide of -99% or below divides by 0.01, so a hundred times at
most.

### Which base or override counts

Of all the `Base` and `Override` modifiers on an attribute, only the one
applied last counts:

- a source's modifiers come in the order its `ModifierOf` was last set: an
  effect applied, an item equipped;
- the actor's own modifiers come as its `Modifiers` was added;
- within one source, the later in its list wins;
- sources applied in the same tick are ordered by entity.

On the server this order is exact, and the server decides. A client shows
what the server serves ([Prediction](prediction.md)).

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 10.0)] struct SpellPower;
# let mut app = App::new();
# app.add_plugins(MettlePlugins.set(AttributePlugin::new(Update)))
#     .add_attribute::<SpellPower>();
let mage = app.world_mut().spawn(Base::<SpellPower>::default()).id();
app.world_mut().spawn((
    ModifierOf(mage),
    Modifiers(vec![Modifier::overriding::<SpellPower>(5.0)]),
));
app.update();
app.world_mut().spawn((
    ModifierOf(mage),
    Modifiers(vec![Modifier::overriding::<SpellPower>(50.0)]),
));
app.update();
assert_eq!(app.world().get::<Current<SpellPower>>(mage).unwrap().get(), 50.0);
```

## When a modifier counts

A plain modifier always counts. Three builders narrow that.

**A condition.** [`Modifier::condition`] takes any [`Condition`]. The modifier
counts while it holds, and as much as it does: a condition weighing 0.5
counts half the amount. [`Modifier::read_tagged`] and
[`Modifier::owner_has`] are the common ones. Each condition you add must
hold as well: they're combined with "and".

**Per another attribute.** [`Modifier::per`] counts once for each whole
amount of another attribute, read in the same context. +1 spell power per
10 mana gives +6 at 60 mana. A multiplicative one is multiplied in each
time: +10% twice is +21%.

**Local.** [`Modifier::local`] counts only for reads that name its source:
a staff's own spell power, not every spell's. [Reading in
context](reading.md#local-modifiers) shows how a read names one.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 10.0)] struct SpellPower;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# let mut app = App::new();
# app.add_plugins(MettlePlugins.set(AttributePlugin::new(Update)))
#     .add_attribute::<SpellPower>()
#     .add_attribute::<Mana>();
let mage = app
    .world_mut()
    .spawn((
        Base::<SpellPower>::default(),
        Base::<Mana>::new(60.0),
        Modifiers(vec![
            // +1 for each whole 10 mana: +6.
            Modifier::flat::<SpellPower>(1.0).per::<Mana>(10.0),
            // +20% while casting.
            Modifier::additive::<SpellPower>(0.2).owner_has("state.casting"),
        ]),
    ))
    .id();
app.update();
assert_eq!(app.world().get::<Current<SpellPower>>(mage).unwrap().get(), 16.0);

app.world_mut()
    .get_mut::<OwnedTags>(mage)
    .unwrap()
    .add(&Id::new("state.casting"));
app.update();
let power = app.world().get::<Current<SpellPower>>(mage).unwrap().get();
assert!((power - 19.2).abs() < 1e-4);
```

`Base` and `Override` count only in full. One whose condition weighs less
than 1 doesn't count at all. They can't be per anything.

A modifier whose numbers can't be (a value that isn't finite, an attribute
the app doesn't have, a `per` of 0, a `Base` per something) is skipped with
a warning. Data is checked earlier: [`Modifier::check`] checks a modifier
against the attributes the app added and the tag tree, and effects' are
checked as content is loaded.

## Owned tags

`owner_has` reads the tags the actor owns: its [`OwnedTags`], a component
from `bevy_mettle_core`. Effects and abilities share them. The game marks
the mage `state.casting` while it casts. Effects grant tags, such as
`state.burning`, while they last. A tag counts for every tag above it, so
`owner_has("state")` holds while the mage owns `state.casting`.

Mettle tells owned tags coming and going with [`TagAdded`] and
[`TagRemoved`] in [`AttributeSystems`], before values are worked out.
[Gameplay tags](tags.md) covers them.

## In data

In data a modifier names its attribute by name. Fields that aren't there are
left out:

```ron
(attribute: "spell_power", op: Flat, value: 5.0)
(attribute: "resistance", op: Flat, value: 0.1, condition: ReadTagged("damage.elemental"))
(attribute: "spell_power", op: Flat, value: 1.0, per: (attribute: "mana", every: 10.0))
(attribute: "spell_power", op: Flat, value: 100.0, local: true)
```

A misspelled field is an error. Networking writes every field, in order, in
its binary format.

## Pitfalls

- **Writing a fraction as a percent.** `Modifier::additive::<Mana>(20.0)`
  is +2000%. Write `0.2`.
- **Expecting two overrides to combine.** Only the last applied counts.
- **A modifier on the wrong entity.** A source needs `ModifierOf`. Without
  it, its `Modifiers` count for the source itself, if it has attributes.
- **A limit that reads itself.** If an attribute's limit or `per` leads back
  to itself, the loop is cut where it closes, so the value stays finite.

[`Modifiers`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Modifiers.html
[`ModifierOf`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.ModifierOf.html
[`ModifierSources`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.ModifierSources.html
[`Modifier::condition`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Modifier.html#method.condition
[`Modifier::read_tagged`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Modifier.html#method.read_tagged
[`Modifier::owner_has`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Modifier.html#method.owner_has
[`Modifier::per`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Modifier.html#method.per
[`Modifier::local`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Modifier.html#method.local
[`Modifier::check`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Modifier.html#method.check
[`Condition`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/condition/enum.Condition.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
[`TagAdded`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagAdded.html
[`TagRemoved`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRemoved.html
[`AttributeSystems`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.AttributeSystems.html
