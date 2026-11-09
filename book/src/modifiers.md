# Modifiers

A modifier changes one attribute: +5 spell power from a ring, +20% mana
from a passive, a fire resistance an effect grants. Modifiers sit on
sources (an item, an effect, the actor itself) and count for one actor.
Mettle folds them into the attribute's current value.

Code writes a modifier by its attribute's type, as a [`ModifierSpec`], and
gives an entity its modifiers with `modifiers(..)`, on `EntityCommands` or
`EntityWorldMut`. The mage's ring:

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
    .spawn(ModifierOf(mage))
    .modifiers([
        ModifierSpec::flat::<SpellPower>(5.0),
        ModifierSpec::additive::<Mana>(0.2),
    ])
    .id();
# app.update();
# let world = app.world();
assert_eq!(world.get::<Current<SpellPower>>(mage).unwrap().get(), 15.0);
let mana = world.get::<Current<Mana>>(mage).unwrap().get();
assert!((mana - 120.0).abs() < 1e-4);
# let _ = ring;
```

As it's put on, each becomes a [`Modifier`], which names its attribute by
number, an [`AttributeId`], as Bevy names a component by its `ComponentId`.
What's kept, folded and replicated is the number. With one in hand
(`world.attribute_id::<SpellPower>()`, or `attributes.id_of::<SpellPower>()` from
the [`AttributeRegistry`]), make a `Modifier` directly:
`Modifier::flat(power, 5.0)`.

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
    .spawn(Base::<SpellPower>::default())
    .modifiers([
        ModifierSpec::flat::<SpellPower>(5.0),
        ModifierSpec::additive::<SpellPower>(0.2),
        ModifierSpec::additive::<SpellPower>(0.3),
        ModifierSpec::multiplicative::<SpellPower>(0.2),
        ModifierSpec::multiplicative::<SpellPower>(0.3),
        ModifierSpec::divide::<SpellPower>(1.0),
        ModifierSpec::unscaled::<SpellPower>(2.0),
    ])
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
app.world_mut()
    .spawn(ModifierOf(mage))
    .modifiers([ModifierSpec::overriding::<SpellPower>(5.0)]);
app.update();
app.world_mut()
    .spawn(ModifierOf(mage))
    .modifiers([ModifierSpec::overriding::<SpellPower>(50.0)]);
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
#[derive(Tag)]
struct State;
#[derive(Tag)]
#[tag(parent = State)]
struct Casting; // "state.casting"

let mage = app
    .world_mut()
    .spawn((Base::<SpellPower>::default(), Base::<Mana>::new(60.0)))
    .modifiers([
        // +1 for each whole 10 mana: +6.
        ModifierSpec::flat::<SpellPower>(1.0).per::<Mana>(10.0),
        // +20% while casting.
        ModifierSpec::additive::<SpellPower>(0.2).owner_has(Casting),
    ])
    .id();
app.update();
assert_eq!(app.world().get::<Current<SpellPower>>(mage).unwrap().get(), 16.0);

app.world_mut()
    .get_mut::<OwnedTags>(mage)
    .unwrap()
    .add(Casting);
app.update();
let power = app.world().get::<Current<SpellPower>>(mage).unwrap().get();
assert!((power - 19.2).abs() < 1e-4);
```

`Base` and `Override` count only in full. One whose condition weighs less
than 1 doesn't count at all. They can't be per anything.

A modifier whose numbers can't be (a value that isn't finite, an attribute
the app doesn't have, a `per` of 0, a `Base` per something) is skipped with
a warning. Data is checked earlier: [`Modifier::check`] checks a modifier
against the attributes the app added and the tag registry, and effects' are
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

`Modifiers` are for sources made in code. What data writes, an item's stats
say, is an infinite [effect](effects.md), whose modifiers name attributes by
name: checking the content resolves them to numbers. Applied as the item is
equipped and taken off as it's unequipped, it does what a source does, as GAS
equips items.

## Over a network

On a server, a modifier source the game makes in code, an item or a passive,
has its `Modifiers` there and nowhere else. To show them on a client, opt in
with a [`ModifiersReplica`]: put one on the source, empty, and replicate it.

- **On a server**, mettle keeps the replica in step with the source's
  `Modifiers`, and the actor it's `ModifierOf`: as either changes, as the
  replica's put on, and as the source is taken off its actor.
- **On a client**, mettle gives the source the `Modifiers` and the
  `ModifierOf` the replica says, as it arrives or changes. The actor is an
  entity: replication maps it into the client's world.

A replica holds numbers only. A [`Modifier`] names its attribute by its
[`AttributeId`], which agrees on both sides as long as they add the same
attributes in the same order. Its condition's tags are [`TagKey`]s, this
process's alone, so the replica holds a `Modifier<TagId>`: each tag by its
number in the [tag registry](tags.md#the-tag-registry), which agrees
wherever the same content is loaded. A modifier whose condition names a tag
the registry hasn't can't be sent; it's left out, with a warning.

Here the replica goes from a server's world to a client's by hand, as the
game's replication would carry it:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 10.0)] struct SpellPower;
#[derive(Tag)]
struct State;
#[derive(Tag)]
#[tag(parent = State)]
struct Casting;

fn app(role: NetRole) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, MettlePlugins.set(AttributePlugin::new(Update))))
        .add_attribute::<SpellPower>()
        .add_tag::<Casting>()
        .insert_resource(role)
        .insert_resource(MettleContent::default());
    app
}
let mut server = app(NetRole::Server);
let mut client = app(NetRole::Client);

// The server's ring, opted in.
let mage = server.world_mut().spawn(Base::<SpellPower>::default()).id();
let ring = server
    .world_mut()
    .spawn((ModifierOf(mage), ModifiersReplica::default()))
    .modifiers([ModifierSpec::flat::<SpellPower>(5.0).owner_has(Casting)])
    .id();
server.update();

// What's sent: the tag by its number.
let replica = server.world().get::<ModifiersReplica>(ring).unwrap().clone();
let casting = server.world().resource::<TagRegistry>().id(Casting).unwrap();
assert_eq!(replica.of, Some(mage));
assert_eq!(replica.modifiers[0].condition, Some(ConditionOver::OwnerHas(casting)));

// The client's copies, as they arrive: its mage, as replication maps it.
let mage = client.world_mut().spawn(Base::<SpellPower>::default()).id();
let ring = client.world_mut().spawn(ModifiersReplica { of: Some(mage), ..replica }).id();
client.update();
assert_eq!(client.world().get::<ModifierOf>(ring), Some(&ModifierOf(mage)));
client.world_mut().get_mut::<OwnedTags>(mage).unwrap().add(Casting);
client.update();
assert_eq!(client.world().get::<Current<SpellPower>>(mage).unwrap().get(), 15.0);
```

Effects don't need it: a client makes an active effect's modifiers from the
effect's own replica ([Prediction](prediction.md)). With the lightyear
adapter, `ModifiersReplica` is replicated for you, and `ModifierOf` isn't:
the replica carries it. The game still puts the replica on each source it
replicates ([Over lightyear](lightyear.md)).

## Pitfalls

- **Writing a fraction as a percent.** `ModifierSpec::additive::<Mana>(20.0)`
  is +2000%. Write `0.2`.
- **Expecting two overrides to combine.** Only the last applied counts.
- **A modifier on the wrong entity.** A source needs `ModifierOf`. Without
  it, its `Modifiers` count for the source itself, if it has attributes.
- **A source replicated without its replica.** Replicating a source's
  `ModifierOf` alone shows nothing on a client: its `Modifiers` aren't
  replicated. Put a `ModifiersReplica` on it.
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
[`Condition`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/condition/type.Condition.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
[`TagAdded`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagAdded.html
[`TagRemoved`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRemoved.html
[`AttributeSystems`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.AttributeSystems.html
[`AttributeId`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/struct.AttributeId.html
[`AttributeRegistry`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/struct.AttributeRegistry.html
[`ModifierSpec`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.ModifierSpec.html
[`Modifier`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Modifier.html
[`ModifiersReplica`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.ModifiersReplica.html
[`TagKey`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagKey.html
