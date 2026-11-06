# bevy_mettle_core

What `bevy_mettle`'s crates share: gameplay tags and the tags an actor
owns, curve tables, and the condition language. Its only ECS is the
owned tags (`bevy_ecs`, no app). Mettle is a gameplay framework
in the spirit of Unreal's Gameplay Ability System: attributes are the
game's own types, and everything else (tags, curves, effects, abilities)
is data, named by tags and ids.

At the root: [`Id<T>`](Id), what everything defined in data is named by;
[`IdTarget`](trait@IdTarget), what it can name (`#[derive(IdTarget)]`); and [`MettleError`] and
[`MettleErrors`], what can be wrong with data.

| Module | What's in it |
|---|---|
| [`tag`] | Gameplay tags: `Tags`, `TagCounts`, the `TagTree`; an actor's `OwnedTags` and the events of them changing |
| [`curve`] | Curve tables: `LevelCurve`, the `CurveTable` |
| [`condition`] | The condition language: `Condition`, and the `Context` it's weighed in |

[`prelude`] brings the lot in at once.

## Tags

A tag is a label in a tree, defined in data, its id its path
(`damage.elemental.fire`), so a mod can add its own. What has a tag has
every tag above it: a rule on `damage.elemental` holds for fire.

- [`TagTree`]: the tags there are, from [`Tag`]s in however many files,
  checked: each a path whose parent is a tag of its own.
- [`Tags`]: a set, what an effect, an ability or a hit is labelled with;
  holds every tag above the ones it's given, written to data as given.
- [`TagCounts`]: tags held by count, what an actor owns while its states
  hold: each source (an effect, the game marking it moving) adds one and
  takes its one away.
- [`OwnedTags`]: on an actor, its states, by count: what the game marks
  it with (`state.moving`) and what effects grant it (`immune.stun`).
  [`TagAdded`] and [`TagRemoved`] tell them coming and going, as
  [`publish_tags`] runs (`bevy_mettle_attribute` schedules it), and
  [`owns`] is a guard on them, for `bevy_arbor`. Mettle's other crates
  share it: modifiers count while their owner owns a tag, effects grant
  them, abilities are blocked by them.

```ron
[(id: "damage"), (id: "damage.elemental"), (id: "damage.elemental.fire")]
```

## Curves

[`CurveTable`]: values by level, from [`LevelCurve`]s in data, checked (keys
finite, levels rising) and built as Bevy's curves (`bevy_curve`). Between
keys, `Constant`, `Linear`, or `Ease(..)` with any of Bevy's easing
functions; past the ends, as at the nearer one. What effects scale by, as
Unreal's curve tables are. [`CurveTable::curve`] hands back the curve
itself, for graphs and Bevy's curve adaptors.

```ron
[(id: "fireball.damage", interpolation: Ease(QuadraticIn), keys: [(1, 10), (10, 85), (20, 240)])]
```

Mettle doesn't load anything: the game reads its files however it likes,
and hands the definitions to [`TagTree::build`], [`CurveTable::build`] and
the like.

## Condition

[`Condition`] is the condition language: an expression over the read's tags,
the tags its owner, target and source own, and the facts the read
supplies (numbers named by tags: `fact.flown`), weighed from 0 to 1 in a
[`Context`].

| Term | Weight |
|---|---|
| `ReadTagged(tag)` | 1 if the read (the hit, the ability) has it, or one under it |
| `OwnerHas(tag)` | 1 if the owner owns it, or one under it |
| `TargetHas(tag)` | 1 if the target owns it, or one under it |
| `SourceHas(tag)` | 1 if the source (the caster, the attacker) owns it, or one under it |
| `FactSet(fact)` | 1 if the fact is above 0 |
| `FactAbove(fact: .., value: ..)`, `FactBelow(fact: .., value: ..)` | 1 if the fact compares so |
| `FactRamp(fact: .., full: .., none: ..)` | 1 at `full`, 0 at `none`, in proportion between |
| `FactEvery(fact: .., n: ..)` | 1 if the fact, a count, is a whole multiple of `n` |
| `All([..])`, `Any([..])`, `Not(..)` | the product, the greatest, one less it |

```ron
All([
    ReadTagged("delivery.attack"),
    FactRamp(fact: "fact.flown", full: 2, none: 8),
    Not(TargetHas("creature.undead")),
])
```

A fact the read didn't supply isn't there: a read with no hit gets no hit
bonuses. What a weight in part means is up to what reads it; a
requirement asks for it to hold in full ([`Condition::holds`]).
[`Condition::check`] checks its tags against a [`TagTree`] and its numbers.

[`TagTree`]: tag::TagTree
[`TagTree::build`]: tag::TagTree::build
[`Tag`]: tag::Tag
[`Tags`]: tag::Tags
[`TagCounts`]: tag::TagCounts
[`OwnedTags`]: tag::OwnedTags
[`TagAdded`]: tag::TagAdded
[`TagRemoved`]: tag::TagRemoved
[`publish_tags`]: tag::publish_tags
[`owns`]: tag::owns
[`CurveTable`]: curve::CurveTable
[`CurveTable::build`]: curve::CurveTable::build
[`CurveTable::curve`]: curve::CurveTable::curve
[`LevelCurve`]: curve::LevelCurve
[`Condition`]: condition::Condition
[`Condition::check`]: condition::Condition::check
[`Condition::holds`]: condition::Condition::holds
[`Context`]: condition::Context
