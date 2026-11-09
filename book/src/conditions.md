# Conditions

A [`Condition`] says when something counts, and how much. The mage's ring
adds fire resistance only against fire. Burning stops hurting while the mage
is wet. The fireball can't be cast while the mage is stunned. Each of these
is a condition.

A condition is weighed from 0, not at all, to 1, in full. Most terms either
hold or don't. One, `FactRamp`, can hold in part, and so can what's built
from it.

## Writing one

A condition is data: a tree of terms, each naming tags. Here is one built
from its variants:

```rust
# use bevy_mettle::prelude::*;
let falloff = Condition::All(vec![
    Condition::ReadTagged("damage.elemental.fire".into()),
    Condition::FactRamp { fact: "fact.flown".into(), full: 2.0, none: 8.0 },
    Condition::Not(Box::new(Condition::TargetHas("immune.fire".into()))),
]);
# let _ = falloff;
```

That reads: a fire hit, at full strength up to 2 tiles flown and fading to
nothing at 8, against a target that isn't immune to fire.

These are the terms:

| Term | Weight |
|---|---|
| `ReadTagged(tag)` | 1 if the read has the tag, or one under it: the hit's tags, the ability's |
| `OwnerHas(tag)` | 1 if the owner owns the tag, or one under it |
| `TargetHas(tag)` | 1 if the target owns the tag, or one under it |
| `SourceHas(tag)` | 1 if the source, such as the caster or the attacker, owns the tag, or one under it |
| `FactSet(fact)` | 1 if the fact is above 0 |
| `FactAbove { fact, value }` | 1 if the fact is above `value` |
| `FactBelow { fact, value }` | 1 if the fact is below `value` |
| `FactRamp { fact, full, none }` | 1 at `full`, 0 at `none`, in proportion between, and as at the nearer end beyond |
| `FactEvery { fact, n }` | 1 if the fact, a count of 1 or more, is a whole multiple of `n`: every sixth hit |
| `All(..)` | The product of the weights: every one holds |
| `Any(..)` | The greatest of the weights: any one holds |
| `Not(..)` | One less the weight: it doesn't hold |

A ramp can go either way. With `full` below `none`, it fades as the fact
grows: a point-blank bonus. With `full` above `none`, it builds up: a bonus
for a far shot.

In code, the same conditions come from builders. `!` makes a `Not`, and
`and` adds a term to an `All`:

```rust
# use bevy_mettle::prelude::*;
let falloff = Condition::read_tagged("damage.elemental.fire")
    .and(Condition::fact_ramp("fact.flown", 2.0, 8.0))
    .and(!Condition::target_has("immune.fire"));

let written = Condition::All(vec![
    Condition::ReadTagged("damage.elemental.fire".into()),
    Condition::FactRamp { fact: "fact.flown".into(), full: 2.0, none: 8.0 },
    Condition::Not(Box::new(Condition::TargetHas("immune.fire".into()))),
]);
assert_eq!(falloff, written);
```

`Condition` is a serde type: in data, a condition is written as its variant,
with each tag by its name.

The rest are `Condition::all`, `any`, `owner_has`, `source_has`, `fact_set`,
`fact_above`, `fact_below` and `fact_every`.

## Owner, target and source

Who is who depends on what's reading:

- **The owner** is the actor whose value is being worked out. For a ring's
  modifier, it's the mage wearing it.
- **The target** is who a read is about: the actor a hit lands on.
- **The source** is who caused it: the caster of a fireball, or the attacker.

A read that names no target or source has none, so `TargetHas` and
`SourceHas` don't hold. [Reading in context](reading.md) shows how a read
names them.

## Facts

A fact is a number the read supplies, named by a tag: how far the fireball
flew (`fact.flown`), whether the mage struck from behind. A fact the read
didn't supply isn't there. `FactSet`, the comparisons and `FactEvery` don't
hold, and a ramp weighs 0. So a read with no hit gets no hit bonuses.

## What a weight in part means

A weight between 0 and 1 means whatever reads it says it means. A modifier
counts as much as its condition holds: at half weight, the ring gives half
its resistance. A requirement asks for the condition to hold in full,
[`Condition::holds`]: an effect applies, or an ability activates, only at
weight 1.

## Weighing a condition yourself

Mettle weighs conditions for you as it reads attributes and checks effects
and abilities. To weigh one in your own code, give it a [`Context`]: the
read's tags, what the owner, target and source own, and the facts.

```rust
# use bevy_mettle::prelude::*;
use bevy_mettle::core::condition::Context;

# #[derive(Tag)] struct Damage;
# #[derive(Tag)] #[tag(parent = Damage)] struct Elemental;
#[derive(Tag)]
#[tag(parent = Elemental)]
struct Fire;

# #[derive(Tag)] struct Immune;
# #[derive(Tag)] #[tag(parent = Immune, name = "fire")] struct ImmuneToFire;
# #[derive(Tag)] struct Fact;
#[derive(Tag)]
#[tag(parent = Fact)]
struct Flown;

/// A fireball's hit, landing on a target that owns `target`.
struct Hit {
    flown: f32,
    target: OwnedTags,
}

impl Context for Hit {
    fn tag(&self, tag: TagKey) -> bool {
        TagKey::from(Fire).is_within(tag)
    }
    fn has(&self, _: TagKey) -> bool {
        false
    }
    fn target(&self, tag: TagKey) -> bool {
        self.target.contains(tag)
    }
    fn source(&self, _: TagKey) -> bool {
        false
    }
    fn fact(&self, fact: TagKey) -> Option<f32> {
        (fact == TagKey::from(Flown)).then_some(self.flown)
    }
}

let falloff = Condition::read_tagged("damage.elemental")
    .and(Condition::fact_ramp("fact.flown", 2.0, 8.0))
    .and(!Condition::target_has("immune.fire"));

let close = Hit { flown: 1.0, target: OwnedTags::default() };
let far = Hit { flown: 5.0, target: OwnedTags::default() };
assert_eq!(falloff.weight(&close), 1.0);
assert_eq!(falloff.weight(&far), 0.5);
assert!(!falloff.holds(&far));

let mut immune = OwnedTags::default();
immune.add(ImmuneToFire);
assert_eq!(falloff.weight(&Hit { flown: 1.0, target: immune }), 0.0);
```

## Checking

[`Condition::check`] checks every tag a condition names against the
[tag registry](tags.md#the-tag-registry), and that its numbers can be: comparisons with finite
values, a ramp whose ends differ, `FactEvery` of more than 0. Mettle checks
the conditions in effects and abilities as content is loaded, so
you only call it for conditions of your own.

```rust
# use bevy_mettle::prelude::*;
# let tags = TagRegistry::build([
#     TagKey::named("fact"),
#     TagKey::named("fact.flown"),
# ])
# .unwrap();
assert!(Condition::fact_ramp("fact.flown", 2.0, 8.0).check(&tags, None).is_ok());
assert!(Condition::fact_ramp("fact.flown", 2.0, 2.0).check(&tags, None).is_err());
assert!(Condition::fact_set("fact.behind").check(&tags, None).is_err());
```

## Tags by name or by number

A `Condition` names its tags by [`TagKey`]: what content holds and what
mettle weighs. Over a network, a tag goes as its number in the
[tag registry](tags.md#the-tag-registry), so a condition goes as a
[`ConditionOver<TagId>`][ConditionOver]: the same terms over tag numbers.
`Condition` is `ConditionOver<TagKey>`. [`ConditionOver::map_tags`] goes
between the two, and gives `None` if a tag doesn't map:

```rust
# use bevy_mettle::prelude::*;
# let tags = TagRegistry::build([
#     TagKey::named("immune"),
#     TagKey::named("immune.fire"),
# ])
# .unwrap();
let condition = !Condition::target_has("immune.fire");

let numbered: ConditionOver<TagId> = condition.map_tags(|tag| tags.id(tag)).unwrap();
let back = numbered.map_tags(|id| tags.key(id)).unwrap();
assert_eq!(back, condition);
```

Mettle does this for you where it sends conditions: see
[Modifiers](modifiers.md#over-a-network).

## Pitfalls

- **Tags match downward only.** `ReadTagged("damage.elemental")` holds for a
  fire hit. `ReadTagged("damage.elemental.fire")` doesn't hold for a hit
  tagged only `damage.elemental`.
- **`Not` of a ramp is a ramp.** `Not(FactRamp { .. })` weighs one less the
  ramp, so it holds in part too.
- **An empty `All` always holds; an empty `Any` never does.**

[`Condition`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/condition/type.Condition.html
[ConditionOver]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/condition/enum.ConditionOver.html
[`ConditionOver::map_tags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/condition/enum.ConditionOver.html#method.map_tags
[`Condition::holds`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/condition/enum.ConditionOver.html#method.holds
[`Condition::check`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/condition/enum.ConditionOver.html#method.check
[`Context`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/condition/trait.Context.html
[`Condition::map_tags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/condition/enum.ConditionOver.html#method.map_tags
[`TagKey`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagKey.html
