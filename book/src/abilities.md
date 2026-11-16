# Defining abilities

An ability is something an actor can do: the mage's fireball. It's granted to
the actor, activated, paid for, cooled down, and ended. What the ability is,
what it costs and what blocks it are its definition. What it actually does is
a *behaviour*: a component of yours that your systems run while the ability
is active.

## The fireball

In GAS an ability is a class with properties; in mettle it's a type of
yours. Derive [`Ability`] on a unit struct, and its keys on `#[ability(..)]`
define it:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Component, Default, Behaviour)] struct CastFireball;
# #[derive(Tag)] struct Ability;
# #[derive(Tag)] #[tag(parent = Ability)] struct Spell;
# #[derive(Tag)] #[tag(parent = Spell)] struct Fire;
# #[derive(Tag)] #[tag(parent = Ability)] struct Channel;
# #[derive(Tag)] struct State;
# #[derive(Tag)] #[tag(parent = State)] struct Silenced;
# #[derive(Tag)] #[tag(parent = State)] struct Stunned;
# #[derive(Tag)] #[tag(parent = State)] struct Casting;
# #[derive(Tag)] struct Event;
# #[derive(Tag)] #[tag(parent = Event)] struct Parried;
#[derive(Ability)]
#[ability(behaviour::<CastFireball>, tag = Fire)]
#[ability(cost = Cost::of::<Mana>(20.0), cooldown = 2.0)]
#[ability(require = Condition::all([!Condition::owner_has(Silenced), !Condition::owner_has(Stunned)]))]
#[ability(ends = Channel, blocks = Spell, owner_tag = Casting)]
#[ability(trigger = Trigger::Event(Parried.into()))]
struct Fireball;

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_attribute::<Mana>()
    .add_behaviour::<CastFireball>()
#   .add_tag::<Fire>().add_tag::<Channel>()
#   .add_tag::<Silenced>().add_tag::<Stunned>().add_tag::<Casting>()
#   .add_tag::<Parried>()
    .add_ability::<Fireball>();
app.update();

let registry = app.world().resource::<AbilityRegistry>();
let id = registry.id_of::<Fireball>().unwrap();
assert_eq!(registry.id("fireball"), Some(id));
```

Each key is the [`AbilityData`] builder method of the same name:
`cooldown = 2.0` is `.cooldown(2.0)`, `behaviour::<CastFireball>` is
`.behaviour::<CastFireball>()`, and `cost = Cost::of::<Mana>(20.0)` is
`.cost(Cost::of::<Mana>(20.0))`. A key that isn't a method doesn't compile.
Keys can be given more than once, `cost` for an ability paid in two
attributes, and spread over several `#[ability(..)]` attributes.

Its key is its type's name in `snake_case`, `"fireball"`, unless `name` says
otherwise: `#[ability(name = "fireball.charged")]`. Add it to the app with
`add_ability::<Fireball>()`, or several at once with
`add_abilities::<(Fireball, Frostbolt)>()`. Code then grants and activates it
by the type: `grant_ability(Fireball)`, or `grant_ability(Fireball.level(2.0))`
at a level, and `activate_ability(Fireball)`, as
[Granting and activating](activating.md) shows. A misspelled type doesn't
compile.

Only `behaviour` is needed. Everything else has a default: no tags, no cost,
no cooldown, nothing blocking it, `Commit::OnActivate` and
`NetExecution::Predicted`.

## Long definitions

A definition that reads better as code than as keys goes in a function,
named with `define`. It's handed the ability, named by its key, and fills it
in with the same builders:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
# #[derive(Component, Default, Behaviour)] struct CastProjectile;
# #[derive(Tag)] struct State;
# #[derive(Tag)] #[tag(parent = State)] struct Stunned;
#[derive(Ability)]
#[ability(define = blood_bolt)]
struct BloodBolt;

fn blood_bolt(ability: AbilityData) -> AbilityData {
    ability
        .behaviour::<CastProjectile>()
        .cost(Cost::of::<Mana>(10.0))
        .cost(Cost::of::<Life>(5.0))
        .cooldown(1.5)
        .end_if(Condition::owner_has(Stunned))
}
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .add_attribute::<Mana>().add_attribute::<Life>()
#     .add_behaviour::<CastProjectile>().add_tag::<Stunned>()
#     .add_ability::<BloodBolt>();
# app.update();
# assert!(app.world().resource::<AbilityRegistry>().id_of::<BloodBolt>().is_some());
```

`define` defines all of it: an ability with `define` has no other keys.

## The keys

| Key | What it does |
|---|---|
| `behaviour::<B>` | The component in code that runs it, added with `add_behaviour`. |
| `tag` | What the ability is. Blocking, ending and activating by tag match on these. |
| `cost` | What it pays as it commits, a [`Cost`]: an amount of one of its owner's attributes. It can't activate if paying would take an attribute below 0. |
| `cooldown` | How many seconds it cools down for once it commits. It can't activate while it does. |
| `cooldown_tag` | The tag its owner holds while it cools down: `cooldown.<its key>`, unless it says. |
| `require` | A [condition](conditions.md) that has to hold for it to activate. |
| `end_if` | A condition that interrupts it: while it's active, it's cancelled as soon as this holds. See [Granting and activating](activating.md#interrupted). |
| `ends` | Active abilities tagged any of these are cancelled as it activates. |
| `blocks` | Other abilities tagged any of these can't activate while it's active. |
| `owner_tag` | A tag its owner holds while it's active. |
| `cue` | A cue held on its owner while it's active, such as a cast's glow. See [Cues](cues.md). |
| `trigger` | What activates it besides being asked. See [Grants and triggers](grants-triggers.md). |
| `granted_by::<E>` | A lasting effect that grants it while it lasts. See [Grants and triggers](grants-triggers.md). |
| `commit` | When its cost and cooldown are applied: `Commit::OnActivate` or `Commit::Manual`. |
| `net` | How it runs over a network: `Predicted` (the default), `ServerOnly`, `ServerInitiated` or `LocalOnly`. See [Prediction](prediction.md#execution-policies). |

A tag in `ends` or `blocks` matches the tags under it too.
`blocks = Spell` keeps every spell from activating, fire or not.

`require` is weighed in the ability's own tags and its owner's. `ReadTagged`
asks the ability's tags. `OwnerHas`, `TargetHas` and `SourceHas` all ask the
owner, since an ability is its own source and target. It has no facts, so a
fact term never holds. An ability kept from activating while its owner owns
a tag requires its absence: `require = !Condition::owner_has(Stunned)`.

## Behaviours

A behaviour is a component with [`#[derive(Behaviour)]`][`Behaviour`], added
to the app with
[`add_behaviour`](https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/behaviour/trait.BehaviourApp.html#tymethod.add_behaviour).
Content names it by its name, a `Key<AnyBehaviour>`: its type's in
`snake_case`, unless it says. Adding it gives it its number in the
`BehaviourRegistry`, as attributes get theirs.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# use bevy_mettle::ability::behaviour::BehaviourRegistry;
#[derive(Component, Default, Behaviour)]
struct CastFireball {
    timer: f32,
}

let mut app = App::new();
app.add_plugins(MettlePlugins)
    .add_behaviour::<CastFireball>();
let behaviours = app.world().resource::<BehaviourRegistry>();
let id = behaviours.id_of::<CastFireball>().unwrap();
assert_eq!(behaviours.id("cast_fireball"), Some(id));
assert_eq!(behaviours.name(id), Some("cast_fireball"));
```

When the fireball activates, `CastFireball::default()` is put on the
ability's entity. When it ends, it's taken off. Your systems find it there,
next to [`ActiveAbility`]. [Granting and activating](activating.md) shows such
a system.

The component has to be `Default`, since mettle makes a fresh one each time.
Its fields are the cast's run state, such as `timer`. Several abilities can
share one behaviour: a `CastProjectile` behaviour can run the fireball and
the frostbolt alike, telling them apart by the ability.

Its settings go on `#[behaviour(..)]`:

| Setting | Meaning | Unless it says |
|---|---|---|
| `name = ".."` | The name content names it by | its type's, in `snake_case` |
| `input = Type` | What its abilities are activated with: an aim, a target | none |

What a player chooses, such as where the fireball is aimed, isn't run state:
it's the behaviour's *input*. [Input and targets](input.md) covers it.

## Cost and cooldown

An ability says what it costs and how long it cools down, and mettle makes
the effects that apply them, as GAS has a cost and a cooldown effect on each
ability:

- **The cost**, `<its key>.cost`, is an instant effect taking each
  [`Cost`] off its attribute's base. Before the ability activates, its
  changes are worked out one after another on the owner's values. If any
  attribute would end below 0, the ability fails with `Failure::Cost`. A
  mage with 15 mana can't cast a fireball that costs 20.
- **The cooldown**, `<its key>.cooldown`, is a lasting effect granting its
  owner the cooldown tag: `cooldown.<its key>`, so `cooldown.fireball`. While
  the owner owns it, the ability fails with `Failure::Cooldown`. Mettle
  defines those tags itself.

Both are applied to the owner at the ability's level, with the owner as their
source, under the activation (see [Prediction](prediction.md)). A cost or a
cooldown is a [magnitude](effects.md), so one scaled by a curve reads it at
the ability's level: `cost = Cost::of::<Mana>(Magnitude::scaled::<FireballCost>(1.0))`.

Abilities with the same `cooldown_tag` share a cooldown: a "global cooldown"
every spell starts and waits for. The tag is one of yours, so add it to the
app as any other:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Component, Default, Behaviour)] struct CastProjectile;
#[derive(Tag)]
struct Global;

#[derive(Ability)]
#[ability(behaviour::<CastProjectile>, cooldown = 1.0, cooldown_tag = Global)]
struct Fireball;

#[derive(Ability)]
#[ability(behaviour::<CastProjectile>, cooldown = 1.0, cooldown_tag = Global)]
struct Frostbolt;

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_behaviour::<CastProjectile>()
    .add_tag::<Global>()
    .add_abilities::<(Fireball, Frostbolt)>();
app.update();

let world = app.world_mut();
let mage = world.spawn(OwnedTags::default()).id();
world.grant_ability(mage, Fireball).unwrap();
world.grant_ability(mage, Frostbolt).unwrap();
world.activate_ability(mage, Fireball).unwrap();
assert_eq!(world.activate_ability(mage, Frostbolt), Err(Failure::Cooldown));
```

## When it commits

*Committing* is paying the cost and starting the cooldown.

- `Commit::OnActivate`, the default, commits as the ability activates.
- `Commit::Manual` leaves it to the behaviour. A spell with a cast time
  commits when the cast finishes, with `commit_ability`. If the owner can't
  pay then, the ability is cancelled. Until it commits, the cooldown hasn't
  started, so cancelling a cast costs nothing.

The cost and the cooldown commit apart too, with `commit_cost` and
`commit_cooldown`: a charged shot starts its cooldown as it's pressed and
pays as it's let go. Each commits once an activation; committing again does
nothing.

## Checking them

The [`AbilityRegistry`] is built as the app starts, in `PreStartup`, from the
ability types the app added and those content adds. Mettle checks every
ability against the tags, the effects and the behaviours there are, and
reports everything wrong at once:

- an ability with no behaviour;
- a tag, effect or behaviour named that doesn't exist;
- an ability granted by an effect that doesn't exist, or doesn't last;
- triggers on an ability whose behaviour takes input;
- a cost or a cooldown on an ability that's `LocalOnly`;
- an ability content defines twice, or that code already defines.

Abilities are checked once the effects are, so an ability's errors show once
its effects' are fixed.

Content adds abilities by keys of its own, as a mod or a server does: an
[`AbilityData`] for each, with the same builders and the same fields. It's a
serde type, so content written in any serde format reads the same, its tags,
effects and behaviours written as their names:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Component, Default, Behaviour)] struct CastProjectile;
# #[derive(Ability)] #[ability(behaviour::<CastProjectile>)] struct Fireball;
let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_attribute::<Mana>()
    .add_behaviour::<CastProjectile>()
    .add_ability::<Fireball>();

let content = MettleContent {
    abilities: vec![
        AbilityData::new("frostbolt")
            .behaviour::<CastProjectile>()
            .cost(Cost::of::<Mana>(15.0)),
    ],
    ..default()
};
content.check(app.world()).unwrap();

// A typo in the behaviour, and an ability code already defines:
let mut broken = content.clone();
broken.abilities.push(AbilityData::new("firebal"));
broken.abilities.push(AbilityData::new("fireball").behaviour::<CastProjectile>());
let errors = broken.check(app.world()).unwrap_err();
assert_eq!(errors.0.len(), 2);

app.insert_resource(content);
app.update();
let registry = app.world().resource::<AbilityRegistry>();
assert!(registry.id("frostbolt").is_some());
```

As the registry is built, each ability is numbered: an [`AbilityId`], in
order of name, as effects are. Granted abilities, events and the network go
by the number; the name is for content, debug output and what a client shows.
[`AbilityRegistry`] goes between them, as every registry does: `get(id)`,
`id("fireball")`, `id_of::<Fireball>()`, `name(id)` and `iter()`.

The registry is built again as content changes, and, with the `hotpatching`
feature, as code is hot patched, each type defining itself again. Every
granted ability takes its new definition. Abilities keep their numbers, and
new ones are numbered after them. One whose ability is gone is taken away
from its actor. [Adding and tuning content](content.md) shows how content is
handed over.

## Pitfalls

- **Add code before the app starts.** The registry is built in `PreStartup`
  from the behaviours, effects and abilities the app added. Add them all as
  the app is built.
- **A cooldown is a tag.** Abilities with the same `cooldown_tag` share a
  cooldown. That's what a global cooldown wants, and a bug otherwise.
- **The made effects are named after the ability.** `fireball.cost` and
  `fireball.cooldown` are mettle's; an effect of yours with one of those keys
  collides with them.

[`AbilityData`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/data/struct.AbilityData.html
[`Ability`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/trait.Ability.html
[`AbilityId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.AbilityId.html
[`Behaviour`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/behaviour/trait.Behaviour.html
[`Cost`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/data/struct.Cost.html
[`AbilityRegistry`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilityRegistry.html
[`ActiveAbility`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.ActiveAbility.html
