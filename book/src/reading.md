# Reading in context

`Current<A>` is an attribute with nothing in play. A fireball hitting the
mage is something in play: it's fire, it has a source, it flew some way.
Some modifiers count only for reads like that. This chapter reads
attributes in a context.

## Kinds of a value

The mage resists fire better than cold. That's not two attributes. It's one,
`Resistance`, with modifiers that count only for reads tagged with a damage
type:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(max = MaxResistance)] struct Resistance;
# #[derive(Attribute)] #[attribute(base = 0.75)] struct MaxResistance;
# let mut app = App::new();
# app.add_plugins(MettlePlugins.set(AttributePlugin::new(Update)))
#     .add_attribute::<Resistance>()
#     .add_attribute::<MaxResistance>();
# let mage = app
#     .world_mut()
#     .spawn((Base::<Resistance>::default(), Base::<MaxResistance>::default()))
#     .id();
// A ring: +90% elemental resistance, +5% maximum fire resistance.
app.world_mut().spawn((
    ModifierOf(mage),
    Modifiers(vec![
        Modifier::flat::<Resistance>(0.9).read_tagged("damage.elemental"),
        Modifier::flat::<MaxResistance>(0.05).read_tagged("damage.elemental.fire"),
    ]),
));
app.update();

let world = app.world();
let fire = AttributeContext::new().tag("damage.elemental.fire");
let cold = AttributeContext::new().tag("damage.elemental.cold");
let resists = |context: &AttributeContext| world.value_at(mage, &Resistance::ID, context).unwrap();
assert!((resists(&fire) - 0.8).abs() < 1e-4); // capped at 75% + 5%
assert!((resists(&cold) - 0.75).abs() < 1e-4); // capped at 75%
assert_eq!(resists(&AttributeContext::new()), 0.0); // nothing in play
```

Tags are a tree, so the modifier for `damage.elemental` counts for a fire
hit: a read tagged `damage.elemental.fire` is tagged `damage.elemental` too.
Limits are read in the same context, so fire resistance is capped by
maximum fire resistance. A mod adds a damage type by adding its tag. Unreal's
Gameplay Ability System does the same: modifiers with tag requirements,
weighed as the attribute is read.

## The context

[`AttributeContext`] says what's in play. Start from
`AttributeContext::new()`, nothing, and add to it:

| Builder | What the read has |
|---|---|
| `tags(&tags)`, `tag("..")` | The read's own tags: the hit's, the ability's |
| `target_tags(&owned)`, `target_tag("..")` | What the target owns |
| `source_tags(&owned)`, `source_tag("..")` | What the source (the caster, the attacker) owns |
| `fact("fact.flown", 7.0)` | A number named by a tag. Supplying it again replaces it |
| `source(entity)` | The source involved: its local modifiers count |

The owner's own tags are always in play: they're the actor's `OwnedTags`.

These are what a modifier's [condition](conditions.md) is weighed in.
`ReadTagged` reads the read's tags, `TargetHas` the target's, `SourceHas`
the source's, and `FactRamp` and the other fact terms read the facts. A
fact the read didn't supply isn't there, so a read with no hit gets no hit
bonuses.

`tags`, `target_tags` and `source_tags` borrow what they're given. A read per
hit copies nothing.

## In a system

[`AttributeReader`] is a system parameter that reads in context:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 10.0)] struct SpellPower;
# #[derive(Component)] struct Mage;
# #[derive(Resource, Default)] struct Dealt(f32);
fn fireball_damage(
    mages: Query<Entity, With<Mage>>,
    attributes: AttributeReader,
    mut dealt: ResMut<Dealt>,
) {
    for mage in &mages {
        let hit = AttributeContext::new()
            .tag("ability.spell.fire")
            .fact("fact.flown", 7.0);
        if let Some(power) = attributes.value::<SpellPower>(mage, &hit) {
            dealt.0 = power;
        }
    }
}
# let mut app = App::new();
# app.add_plugins(MettlePlugins.set(AttributePlugin::new(Update)))
#     .add_attribute::<SpellPower>()
#     .init_resource::<Dealt>()
#     .add_systems(Update, fireball_damage.after(AttributeSystems));
# app.world_mut().spawn((
#     Mage,
#     Base::<SpellPower>::default(),
#     Modifiers(vec![
#         Modifier::additive::<SpellPower>(0.5).read_tagged("ability.spell.fire"),
#         Modifier::flat::<SpellPower>(10.0).condition(Condition::fact_ramp("fact.flown", 2.0, 12.0)),
#     ]),
# ));
# app.update();
# // (10 + 10 × half) × 1.5: the ramp is half-way at 7.
# assert!((app.world().resource::<Dealt>().0 - 22.5).abs() < 1e-4);
```

`value` is `None` if the actor doesn't have the attribute. With nothing in
play, read `Current<A>` instead: it's already worked out.

The reader also has `value_at`, which names the attribute by id rather than
by type, `breakdown` and `breakdown_at` (below), and `owned`, an actor's
owned tags.

## On the world

[`AttributeWorld`] does the same on a `World`, for exclusive systems, effect
code and tests. It names attributes by id: `value_at`, `breakdown_at` and
`base_at`.

It also changes a base the way an instant effect does. `apply_to_base`
changes it by an operation and a value, then rounds it and keeps it within
its limits. `base_after` says what that would leave, without changing
anything:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
# let mut app = App::new();
# app.add_plugins(MettlePlugins.set(AttributePlugin::new(Update)))
#     .add_attribute::<Life>()
#     .add_attribute::<MaxLife>();
# let mage = app
#     .world_mut()
#     .spawn((Base::<Life>::default(), Base::<MaxLife>::default()))
#     .id();
# app.update();
let world = app.world_mut();
// A fireball for 130: life stops at its minimum, 0.
assert_eq!(
    world.base_after(mage, &Life::ID, 100.0, Operation::Flat, -130.0),
    Some(0.0)
);
world.apply_to_base(mage, &Life::ID, Operation::Flat, -30.0);
assert_eq!(world.base_at(mage, &Life::ID), Some(70.0));
```

Limits are read as the bases are now, even if they changed since attributes
were last kept.

## Breakdowns

A [`Breakdown`] is an attribute's modifiers summed but not yet applied: the
base, Σ flat, Σ additive, the product of the multipliers, Σ divide, Σ
unscaled, and the override if there is one. `total()` applies them with the
formula from [Modifiers](modifiers.md#how-they-add-up).

Use it for a tooltip, or to combine an attacker's and a target's modifiers
into one hit:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 10.0)] struct SpellPower;
# let mut app = App::new();
# app.add_plugins(MettlePlugins.set(AttributePlugin::new(Update)))
#     .add_attribute::<SpellPower>();
# let mage = app
#     .world_mut()
#     .spawn((
#         Base::<SpellPower>::default(),
#         Modifiers(vec![
#             Modifier::flat::<SpellPower>(5.0),
#             Modifier::additive::<SpellPower>(0.2),
#         ]),
#     ))
#     .id();
# app.update();
let parts = app
    .world()
    .breakdown_at(mage, &SpellPower::ID, &AttributeContext::new())
    .unwrap();
assert_eq!((parts.base, parts.flat), (10.0, 5.0));
assert!((parts.total() - 18.0).abs() < 1e-4);
```

## Local modifiers

A [local](modifiers.md#when-a-modifier-counts) modifier counts only for
reads naming its source. The mage's staff adds 100 spell power to its own
spells, not to every spell:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 10.0)] struct SpellPower;
# let mut app = App::new();
# app.add_plugins(MettlePlugins.set(AttributePlugin::new(Update)))
#     .add_attribute::<SpellPower>();
# let mage = app.world_mut().spawn(Base::<SpellPower>::default()).id();
let staff = app
    .world_mut()
    .spawn((
        ModifierOf(mage),
        Modifiers(vec![Modifier::flat::<SpellPower>(100.0).local()]),
    ))
    .id();
app.update();
let world = app.world();
let with_staff = AttributeContext::new().source(staff);
assert_eq!(world.value_at(mage, &SpellPower::ID, &with_staff), Some(110.0));
assert_eq!(world.get::<Current<SpellPower>>(mage).unwrap().get(), 10.0);
```

## Guards

Two guards read attributes with nothing in play:
[`attribute_above`] and [`attribute_below`]. They're read-only systems
taking an agent and a value, the shape `bevy_arbor`'s behavior trees take
as conditions: `when(attribute_below::<Life>, 20.0) => task(Flee)`. An
agent without the attribute is neither above nor below. The guard on owned
tags, `owns`, is in `bevy_mettle_core` ([Gameplay tags](tags.md)).

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# use bevy_mettle::attribute::read::{attribute_above, attribute_below};
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
# let mut app = App::new();
# app.add_plugins(MettlePlugins.set(AttributePlugin::new(Update)))
#     .add_attribute::<Life>()
#     .add_attribute::<MaxLife>();
# let mage = app
#     .world_mut()
#     .spawn((Base::<Life>::new(15.0), Base::<MaxLife>::default()))
#     .id();
# app.update();
let world = app.world_mut();
assert!(world.run_system_cached_with(attribute_below::<Life>, (mage, 20.0)).unwrap());
assert!(!world.run_system_cached_with(attribute_above::<Life>, (mage, 20.0)).unwrap());
```

## On a client

On a client, values with nothing in play are the ones the server serves. A
read in context is worked out on the client, from the modifiers replicated
to it. So it's advisory, as in Unreal: the server's read is the one that
decides. See [Prediction](prediction.md).

## Pitfalls

- **Reading `Current` for a hit.** `Current` leaves out every modifier with
  a condition on the read. Read the hit in its context.
- **A context per frame per actor.** A context is cheap, but reading every
  attribute in context every frame isn't. Read in context where something
  happens: a hit, a cast.
- **A fact not supplied.** A condition on a fact the read doesn't supply
  doesn't hold. Supply every fact your modifiers name.

[`AttributeContext`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/read/struct.AttributeContext.html
[`AttributeReader`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/read/struct.AttributeReader.html
[`AttributeWorld`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/read/trait.AttributeWorld.html
[`Breakdown`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/read/struct.Breakdown.html
[`attribute_above`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/read/fn.attribute_above.html
[`attribute_below`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/read/fn.attribute_below.html
