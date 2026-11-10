# Defining abilities

An ability is something an actor can do: the mage's fireball. It's granted to
the actor, activated, paid for, cooled down, and ended. As with effects, the
rules are data: what the ability is, what it costs, and what blocks it. What
it actually does is code, a *behaviour*: a component of yours that your
systems run while the ability is active.

## The fireball

An ability is content, an [`AbilityData`], named by a string as effects are.
Here is the fireball, built with its builders:

```rust
# use bevy_mettle::prelude::*;
let fireball = AbilityData::new("fireball", "cast_fireball")
    .tag("ability.spell.fire")
    .cost("fireball.cost")
    .cooldown("fireball.cooldown")
    .require(!Condition::owner_has("state.silenced"))
    .blocked_by("state.stunned")
    .cancels("ability.channel")
    .blocks("ability.spell")
    .grant("state.casting")
    .trigger(Trigger::Event(TagKey::named("event.parried")));
assert_eq!(fireball.commit, Commit::OnActivate);
```

Only `name` and `behaviour` are needed. Everything else has a default: no
tags, no cost, no cooldown, nothing blocking it, and `Commit::OnActivate`.
`AbilityData` is a serde type, so the same abilities can come from files in
any serde format: a struct with the fields below, its tags, effects and
behaviour written as their names, strings, and fields left out taking their
defaults.

## Naming abilities in code

Content names abilities by string, a `Key<AnyAbility>`. Code names an ability by its type, as it
does an [effect](effects.md#naming-effects-in-code) or a
[tag](tags.md#tags-in-code). Derive [`Ability`] on a unit struct, and add it
to the app:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Ability)]
struct Fireball;                   // "fireball"

#[derive(Ability)]
#[ability(name = "fireball.charged")]
struct ChargedFireball;            // "fireball.charged"

let mut app = App::new();
app.add_plugins(MettlePlugins)
    .add_ability::<Fireball>()
    .add_ability::<ChargedFireball>();
```

Its name is its type's in `snake_case`, unless `name` says otherwise. Code
then grants and activates it by the type: `grant_ability(Fireball)`, or
`grant_ability(Fireball.level(2.0))` at a level, and
`activate_ability(Fireball)`, as [Granting and activating](activating.md)
shows. A misspelled type doesn't compile.

`add_ability` is a promise that content defines the ability: content that
doesn't is rejected with "code names ability `fireball`, which content
doesn't define". Abilities only content names, such as those an effect
grants, needn't be types.

## The fields

| Field | What it does |
|---|---|
| `tags` | What the ability is. Blocking, cancelling and activating by tag match on these. |
| `behaviour` | The component in code that runs it, added with `add_behaviour`. |
| `cost` | An instant effect it pays as it commits. It can't activate if paying would take an attribute below 0. |
| `cooldown` | A lasting effect applied as it commits. It can't activate while its owner owns a tag that effect grants. |
| `require` | A [condition](conditions.md) that has to hold for it to activate. |
| `cancel_if` | A condition that interrupts it: while it's active, it's cancelled as soon as this holds. See [Granting and activating](activating.md#interrupted). |
| `blocked_by` | It can't activate while its owner owns any of these tags. |
| `cancels` | Active abilities tagged any of these are cancelled as it activates. |
| `blocks` | Other abilities tagged any of these can't activate while it's active. |
| `grants` | Tags its owner owns while it's active. |
| `cues` | Cues held on its owner while it's active, such as a cast's glow. See [Cues](cues.md). |
| `triggers` | What activates it besides being asked. See [Grants and triggers](grants-triggers.md). |
| `commit` | When its cost and cooldown are applied: `OnActivate` or `Manual`. |
| `net` | How it runs over a network: `Predicted` (the default), `ServerOnly`, `ServerInitiated` or `LocalOnly`. See [Prediction](prediction.md#execution-policies). |

A tag in `blocked_by`, `cancels` or `blocks` matches the tags under it too.
`.blocks("ability.spell")` keeps every spell from activating, fire or not.

`require` is weighed in the ability's own tags and its owner's. `ReadTagged`
asks the ability's tags. `OwnerHas`, `TargetHas` and `SourceHas` all ask the
owner, since an ability is its own source and target. It has no facts, so a
fact term never holds.

## Behaviours

A behaviour is a component with [`#[derive(Behaviour)]`][`Behaviour`], added
to the app with
[`add_behaviour`](https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/behaviour/trait.BehaviourApp.html#tymethod.add_behaviour).
Data names it by its name, a `Key<AnyBehaviour>`: its type's in
`snake_case`, unless it says. Adding it gives it its number in the
[`BehaviourRegistry`], as attributes get theirs.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
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
share one behaviour: a `cast_projectile` behaviour can run the fireball and
the frostbolt alike, telling them apart by the ability.

Its settings go on `#[behaviour(..)]`:

| Setting | Meaning | Unless it says |
|---|---|---|
| `name = ".."` | The name data names it by | its type's, in `snake_case` |
| `input = Type` | What its abilities are activated with: an aim, a target | none |

What a player chooses, such as where the fireball is aimed, isn't run state:
it's the behaviour's *input*. [Input and targets](input.md) covers it.

## Cost and cooldown

The fireball's cost and cooldown are effects:

```rust
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Mana;
# use bevy_mettle::prelude::*;
let cost = EffectData::instant("fireball.cost")
    .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0));
let cooldown = EffectData::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball");
# let _ = (cost, cooldown);
```

- **The cost** has to be instant. Before the ability activates, its changes
  are worked out one after another on the owner's values. If any attribute
  would end below 0, the ability fails with `Failure::Cost`. A mage with 15
  mana can't cast a fireball that costs 20.
- **The cooldown** has to last and grant at least one tag. While the owner
  owns any tag it grants, the ability fails with `Failure::Cooldown`. The
  cooldown's tag is how mettle knows it's still cooling down, so a cooldown
  that grants nothing would never cool down at all.

Both are applied to the owner at the ability's level, with the owner as their
source, under the activation (see [Prediction](prediction.md)). A cost or
cooldown scaled by a curve reads it at the ability's level.

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

Abilities reach mettle in [`MettleContent`], with the effects and tags they
name. As it's inserted, mettle checks every ability against the tags, the
effects and the behaviours there are, and every grant (see
[Grants and triggers](grants-triggers.md)), and turns them into the
[`AbilityRegistry`]. It reports everything wrong at once:

- an ability defined twice;
- an ability code names with `add_ability` that content doesn't define;
- a tag, effect or behaviour named that doesn't exist;
- a cost that isn't instant;
- a cooldown that doesn't last, or grants no tag;
- a grant of an effect that doesn't exist or doesn't last, or of an ability
  that doesn't exist.

Abilities are checked once the effects they name are, so an ability's errors
show once its effects' are fixed.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0)]
struct Mana;

#[derive(Component, Default, Behaviour)]
struct CastFireball;

#[derive(Ability)]
struct Fireball;

let mut app = App::new();
app.add_plugins(MettlePlugins)
    .add_attribute::<Mana>()
    .add_behaviour::<CastFireball>()
    .add_ability::<Fireball>();

let tags: Vec<TagKey> = [
    "state", "state.stunned", "state.casting",
    "cooldown", "cooldown.fireball",
    "ability", "ability.spell", "ability.spell.fire",
]
.into_iter()
.map(TagKey::named)
.collect();

let effects = vec![
    EffectData::instant("fireball.cost")
        .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
    EffectData::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
];

let fireball = AbilityData::new("fireball", "cast_fireball")
    .tag("ability.spell.fire")
    .cost("fireball.cost")
    .cooldown("fireball.cooldown")
    .blocked_by("state.stunned")
    .grant("state.casting");

let content = MettleContent {
    tags,
    effects,
    abilities: vec![fireball],
    ..default()
};
content.check(app.world()).unwrap();

// A typo in the behaviour, and a cooldown that's instant:
let mut broken = content.clone();
broken
    .abilities
    .push(AbilityData::new("firebal", "cast_firebal").cooldown("fireball.cost"));
let errors = broken.check(app.world()).unwrap_err();
assert_eq!(errors.0.len(), 2);

// Content without the fireball code names:
let mut missing = content.clone();
missing.abilities.clear();
let errors = missing.check(app.world()).unwrap_err();
assert_eq!(
    errors.to_string(),
    "code names ability `fireball`, which content doesn't define"
);

app.insert_resource(content);
app.update();

// Checked, the fireball has its number.
let registry = app.world().resource::<AbilityRegistry>();
let id = registry.id_of::<Fireball>().unwrap();
assert_eq!(registry.id("fireball"), Some(id));
assert_eq!(registry.name(id), Some("fireball"));
```

As content is checked, each ability is numbered: an [`AbilityId`], in order
of name, as effects are. Granted abilities, events and the network go by the
number; the name is for content, debug output and what a client shows.
[`AbilityRegistry`] goes between them, as every registry does: `get(id)`,
`id("fireball")`, `id_of::<Fireball>()`, `name(id)` and `iter()`.

Change the content later, a hot reload, and every granted ability takes its
new definition. Abilities keep their numbers, and new ones are numbered after
them. One whose ability is gone is taken away from its actor.

Mettle doesn't load anything. Your game reads its files and hands over the
content: see [Loading data](loading-data.md).

## Pitfalls

- **Behaviours first.** Content is checked against the behaviours the app
  added, so add them all before you insert it. The same goes for the
  abilities and effects code names with `add_ability` and `add_effect`.
- **A cooldown is a tag.** Two abilities whose cooldowns grant the same tag
  share a cooldown. That's useful for a shared "global cooldown", and a bug
  otherwise.

[`AbilityData`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/data/struct.AbilityData.html
[`Ability`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/trait.Ability.html
[`AbilityId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.AbilityId.html
[`Behaviour`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/behaviour/trait.Behaviour.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`AbilityRegistry`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilityRegistry.html
[`BehaviourRegistry`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/behaviour/struct.BehaviourRegistry.html
[`ActiveAbility`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.ActiveAbility.html
