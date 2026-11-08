# Defining abilities

An ability is something an actor can do: the mage's fireball. It's granted to
the actor, activated, paid for, cooled down, and ended. As with effects, the
rules are data: what the ability is, what it costs, and what blocks it. What
it actually does is code, a *behaviour*: a component of yours that your
systems run while the ability is active.

## The fireball

In data, the fireball looks like this:

```ron
(
    id: "fireball",
    tags: ["ability.spell.fire"],
    behaviour: "cast_fireball",
    cost: "fireball.cost",
    cooldown: "fireball.cooldown",
    require: Not(OwnerHas("state.silenced")),
    blocked_by: ["state.stunned"],
    cancels: ["ability.channel"],
    blocks: ["ability.spell"],
    grants: ["state.casting"],
    triggers: [Event("event.parried")],
    commit: OnActivate,
)
```

Only `id` and `behaviour` are needed. Everything else has a default: no
tags, no cost, no cooldown, nothing blocking it, and `commit: OnActivate`.

The same ability, written in code with [`Ability`]'s builders:

```rust
# use bevy_mettle::prelude::*;
let fireball = Ability::new("fireball", "cast_fireball")
    .tag("ability.spell.fire")
    .cost("fireball.cost")
    .cooldown("fireball.cooldown")
    .blocked_by("state.stunned")
    .cancels("ability.channel")
    .grant("state.casting")
    .trigger(Trigger::Event(Id::from_static("event.parried")));
assert_eq!(fireball.commit, Commit::OnActivate);
```

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

A tag in `blocked_by`, `cancels` or `blocks` matches the tags under it too.
`blocks: ["ability.spell"]` keeps every spell from activating, fire or not.

`require` is weighed in the ability's own tags and its owner's. `ReadTagged`
asks the ability's tags. `OwnerHas`, `TargetHas` and `SourceHas` all ask the
owner, since an ability is its own source and target. It has no facts, so a
fact term never holds.

## Behaviours

A behaviour is a component with [`#[derive(Behaviour)]`][`Behaviour`], added
to the app with
[`add_behaviour`](https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/behaviour/trait.BehaviourApp.html#tymethod.add_behaviour).
Data names it by its name: its type's in `snake_case`, unless it says.

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
assert!(app.world().resource::<Behaviours>().contains(&Id::from_static("cast_fireball")));
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
let cost = Effect::instant("fireball.cost")
    .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0));
let cooldown = Effect::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball");
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

let mut app = App::new();
app.add_plugins(MettlePlugins)
    .add_attribute::<Mana>()
    .add_behaviour::<CastFireball>();

let tags: Vec<Tag> = [
    "state", "state.stunned", "state.casting",
    "cooldown", "cooldown.fireball",
    "ability", "ability.spell", "ability.spell.fire",
]
.into_iter()
.map(|id| Tag { id: Id::from_static(id) })
.collect();

let effects = vec![
    Effect::instant("fireball.cost")
        .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
    Effect::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
];

let fireball = Ability::new("fireball", "cast_fireball")
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
    .push(Ability::new("firebal", "cast_firebal").cooldown("fireball.cost"));
let errors = broken.check(app.world()).unwrap_err();
assert_eq!(errors.0.len(), 2);

app.insert_resource(content);
```

Change the content later, a hot reload, and every granted ability takes its
new definition. One whose ability is gone is taken away from its actor.

Mettle doesn't load anything. Your game reads its files and hands over the
content. [Loading data](loading-data.md) shows how.

## Pitfalls

- **Behaviours first.** Content is checked against the behaviours the app
  added, so add them all before you insert it.
- **A cooldown is a tag.** Two abilities whose cooldowns grant the same tag
  share a cooldown. That's useful for a shared "global cooldown", and a bug
  otherwise.

[`Ability`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/data/struct.Ability.html
[`Behaviour`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/behaviour/trait.Behaviour.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`AbilityRegistry`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilityRegistry.html
[`ActiveAbility`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.ActiveAbility.html
