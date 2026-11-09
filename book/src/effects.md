# Defining effects

An effect is what changes an actor: damage, a buff, a cooldown, a stun. It
says what it changes, for how long, and what tags it grants while it lasts.
It's Unreal's `GameplayEffect`, as data.

Effects are content: definitions, each with a name. A game builds them as
[`EffectData`] values, from its files or in code, and hands them to mettle in
[`MettleContent`], which checks them all before anything is applied.

## The mage's effects

The fire mage from earlier chapters needs four effects:

- `burning`, fire damage every half second for four seconds.
- `fireball.cost`, 20 mana taken at once.
- `fireball.cooldown`, two seconds during which the mage owns
  `cooldown.fireball`.
- `staff`, a staff the mage holds for as long as it's equipped.

Here they are, with the builders on [`EffectData`]:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0, max = MaxLife)]
# struct Life;
# #[derive(Attribute)]
# #[attribute(base = 100.0)]
# struct MaxLife;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Mana;
fn effects() -> Vec<EffectData> {
    vec![
        EffectData::lasting("burning", 4.0)
            .tag("effect.affliction.burning")
            .period(0.5, true)
            .modifier(EffectModifier::new::<Life>(
                Operation::Flat,
                Magnitude::param("param.damage"),
            ))
            .grant("state.burning")
            .require(!Condition::owner_has("immune.fire"))
            .ongoing(!Condition::owner_has("state.wet")),
        EffectData::instant("fireball.cost")
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
        EffectData::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
        EffectData::infinite("staff"),
    ]
}
# let burning = &effects()[0];
# assert_eq!(burning.name.as_str(), "burning");
# assert!(burning.lasts());
```

`EffectData` is a serde type, so the same effects can come from files in
any serde format. It's a struct with the fields below; tags, and the
attributes, curves, effects, executions, calculations and routes it names,
are written as their names, strings; and fields left out take their
defaults. [Loading data](loading-data.md) says more.

## Naming effects in code

Content names effects by string, since it's data: an `Name<AnyEffect>`. Code names an effect by
its type, as it does a [tag](tags.md#tags-in-code). Derive [`Effect`] on a
unit struct, and add it to the app:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Effect)]
struct Burning;                    // "burning"

#[derive(Effect)]
#[effect(name = "fireball.cooldown")]
struct FireballCooldown;           // "fireball.cooldown"

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_effect::<Burning>()
    .add_effect::<FireballCooldown>();
```

An effect's name is its type's in `snake_case`, unless `name` says
otherwise. The type is how code applies it: `apply_effect(Burning)`, as
[Applying effects](applying-effects.md) shows. A misspelled type doesn't
compile, where a misspelled string would only fail as it's applied.

`add_effect` is a promise that content defines the effect. Content that
doesn't is rejected with "code names effect `burning`, which content
doesn't define", so the name is checked once, as content loads, not each
time code applies it. Effects only content names, such as those chained by
other effects, needn't be types at all.

The staff does nothing yet. [Grants and triggers](grants-triggers.md) gives
it an ability to grant.

## Fields

Only `name` and `duration` are required. Everything else is empty unless the
effect says.

| Field | What it does |
|---|---|
| `name` | What content and an [`Effect`] type call it by |
| `tags` | What it is: what removal, immunity and requirements match on |
| `duration` | `Instant`, `Infinite`, or `For(..)` so many seconds |
| `period` | Executes every so often while it lasts, instead of its modifiers counting all along |
| `modifiers` | What it changes: modifiers while it lasts, or the base as it executes |
| `executions` | Formulas in code, run as it executes: see [Effects in code](effect-code.md) |
| `grants` | Tags the target owns while it counts |
| `cues` | Cues held on the target while it counts, and played each time it executes: see [Cues](cues.md) |
| `require` | What has to hold for it to apply at all |
| `ongoing` | What has to hold for it to count; while it doesn't, it's inhibited |
| `remove_if` | What takes it off as soon as it holds |
| `removes` | Active effects with any of these tags, taken off as it applies |
| `immunity` | Effects with any of these tags can't apply while it counts |
| `stacking` | How applying it again stacks: see [Stacking](stacking.md) |
| `on_apply`, `on_remove`, `on_expire` | Effects applied to the target as it applies, comes off, or runs out of time |

## Durations

- **`Instant`** executes once and is gone. Its modifiers change the target's
  bases: the fireball's cost takes 20 off the mage's base mana for good.
- **`Infinite`** lasts until something takes it off: gear, an aura, a curse
  that has to be cleansed.
- **`For(seconds)`** lasts that long. Its time runs down as effects tick.

A lasting effect's modifiers count as modifiers of the target while it
counts, and stop counting when it goes. The base is left alone, so a
two-second buff can't leave anything behind.

An instant effect can't have anything that needs it to last: a period,
granted tags, immunity, an ongoing or removal requirement, stacking, or
effects on removal or expiry. Content with one is rejected.

## Periods

A period turns a lasting effect into one that executes every so often, like
an instant effect would, for as long as it lasts. `burning` executes every
half second, and once more as it's applied, since `on_apply` is `true`.

A periodic effect's modifiers never count as modifiers. Each execution
changes the base instead, so burning's damage stays taken when it ends.

If a tick is longer than the period, the effect executes as many times as
periods came round. A period has to be more than 0.

## Modifiers

An [`EffectModifier`] names an attribute, an [`Operation`] and a
[`Magnitude`]. In a lasting effect, it's a modifier like any other, folded as
[Modifiers](modifiers.md) explains. As an effect executes, it changes the
base instead:

| Op | What executing does to the base |
|---|---|
| `Flat`, `Unscaled` | Adds the value |
| `Additive`, `Multiplicative` | Scales by `1 + value` |
| `Divide` | Divides by `1 + value` |
| `Base`, `Override` | Sets it to the value |

The base is then rounded and kept within the attribute's limits, so burning
can't take life below 0.

A modifier can have a condition, with `.condition(..)`. In a lasting effect,
it's weighed on each read, as an item's would be:

- `ReadTagged` matches the read's tags: +20% damage with fire hits.
- `OwnerHas` matches the target's owned tags.
- `TargetHas` matches the owned tags of the read's target.
- `SourceHas` matches what the effect's source owned when it applied.

As an effect executes, `ReadTagged` matches the effect's own tags, and
`OwnerHas` and `TargetHas` match the target's. A condition that holds in part
scales the amount. A `Base` or `Override` counts only when it holds in full.

`.per::<A>(every)` makes a modifier count once for each whole `every` of the
target's `A`: +1 damage per 10 spell power.

A modifier can deal a [route](effect-code.md#routes) instead: an amount no
actor holds, routed into attributes they do as the effect executes.
`EffectModifier::route::<IncomingDamage>(40.0)`. In data, a route modifier
names its route by name, `"incoming_damage"`, where an attribute's names
the attribute, and its `op` is flat unless it says. A route is dealt flat amounts only, by an effect that executes:
instant, or periodic. The registry rejects any other.

Data names attributes, routes, executions and calculations by name. Checking
the content resolves each name to its number, which is what the effect runs
by: see [Names and numbers](concepts.md#names-and-numbers).

## Magnitudes

A [`Magnitude`] is how much: a modifier's value, a duration, or a period.

| Magnitude | How much |
|---|---|
| `Value(f32)` | That number. A plain `f32` converts to it. |
| `Scaled { base, curve }` | `base` times the [curve](curves.md) at the spec's level |
| `Param(tag)` | What the code applying it set, by a tag such as `param.damage` |
| `Attribute { attribute, from, coefficient }` | The source's or the target's attribute, times the coefficient |
| `Custom(name)` | A calculation in code, by name, such as `"burn_time"`: see [Effects in code](effect-code.md) |

In code, the same come from `4.0.into()`, [`Magnitude::scaled`],
[`Magnitude::param`], [`Magnitude::source`] or [`Magnitude::target`], and
`Magnitude::Custom("burn_time".into())`.

`Param` is Unreal's `SetByCaller`. The applying code knows something the
data doesn't, such as how hard the fireball hit, and passes it on the spec.
A spec that doesn't set the param gets 0, with a warning.

`Attribute` reads the source's attribute (`from` is `Capture::Source`, the
default) or the target's (`Capture::Target`), with nothing in play, times
`coefficient` (1 unless it says). A modifier's `Attribute` magnitude is captured as the effect
applies, and kept. New stacks and periods use the value captured, not the
source's spell power now. With no source, it's 0.

## Requirements

Three conditions decide whether an effect applies, counts, and stays. Each
is a [condition](conditions.md), weighed against the effect's own tags (as
`ReadTagged`), the target's owned tags (as `OwnerHas` and `TargetHas`), and
what the source owned as it applied (as `SourceHas`). It has to hold in full.
Facts are never set here.

- **`require`** is checked once, as it's applied. If it doesn't hold, the
  effect doesn't apply, and [`EffectBlocked`] says why. Burning doesn't apply
  to a mage who owns `immune.fire`.
- **`ongoing`** is checked every tick. While it doesn't hold, the effect is
  inhibited: its modifiers stop counting, its tags are taken back, and its
  time stands still. Burning is inhibited while the mage is wet, and picks
  up where it left off when the mage dries.
- **`remove_if`** is checked every tick. As soon as it holds, the effect
  comes off.

## Removing and immunity

`removes` takes active effects off the target as the effect applies: a
potion that removes `effect.affliction` cleanses burning and poison. It's
done before the new effect lands, so it never removes itself.

`immunity` keeps effects off while it counts. A fire ward with
`.immunity("effect.affliction.burning")` blocks burning, with
[`EffectBlocked`]'s reason `Immunity`. An inhibited ward protects no one.

Tags match by their tree: removing `effect.affliction` removes everything
tagged under it.

## Chains

An effect can apply other effects to its target:

- `on_apply` as it applies, and again as it gains a stack.
- `on_remove` as it comes off, however that happens.
- `on_expire` as it runs out of time, after `on_remove`.

A chained effect is applied with the same spec: the same level, params,
source and activation. A chain more than 16 effects deep is taken for a loop,
and stopped with a warning.

## Checking them

Effects reach mettle in [`MettleContent`], with the tags and curves they
name. As it's inserted, mettle checks every effect against the [tag
registry](tags.md#the-tag-registry), the [curves](curves.md), the attributes the app added, and
the [effect code](effect-code.md), and turns them into the
[`EffectRegistry`]. It reports every error at once:

- effects defined twice;
- effects code names with [`add_effect`](#naming-effects-in-code) that
  content doesn't define;
- tags, curves, attributes, effects, executions, calculations and routes
  that don't exist;
- numbers that can't be: not finite, a duration or period of 0 or less, a
  `per` of 0 or less, a stacking limit of 0;
- instant effects with parts that need them to last;
- executions on a lasting effect with no period, which never executes.

Content that doesn't check out is rejected: each error is logged,
[`ContentRejected`] is triggered with them, and the content before stays.
To ask before inserting it, call [`MettleContent::check`]:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# use bevy_mettle::core::Errors;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0, max = MaxLife)]
# struct Life;
# #[derive(Attribute)]
# #[attribute(base = 100.0)]
# struct MaxLife;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Mana;
# fn effects() -> Vec<EffectData> {
#     vec![
#         EffectData::lasting("burning", 4.0)
#             .tag("effect.affliction.burning")
#             .period(0.5, true)
#             .modifier(EffectModifier::new::<Life>(Operation::Flat, Magnitude::param("param.damage")))
#             .grant("state.burning")
#             .require(!Condition::owner_has("immune.fire"))
#             .ongoing(!Condition::owner_has("state.wet")),
#         EffectData::instant("fireball.cost")
#             .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
#         EffectData::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
#         EffectData::infinite("staff"),
#     ]
# }
fn tags() -> Vec<TagKey> {
    [
        "cooldown", "cooldown.fireball",
        "effect", "effect.affliction", "effect.affliction.burning",
        "immune", "immune.fire",
        "param", "param.damage",
        "state", "state.burning", "state.wet",
    ]
    .into_iter()
    .map(TagKey::named)
    .collect()
}

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_attribute::<Life>()
    .add_attribute::<MaxLife>()
    .add_attribute::<Mana>();

let content = MettleContent {
    tags: tags(),
    effects: effects(),
    ..default()
};
content.check(app.world())?;
app.insert_resource(content.clone());

// A typo is caught before anything runs.
let typo = MettleContent {
    tags: tags(),
    effects: vec![EffectData::lasting("chill", 2.0).grant("state.chilled")],
    ..default()
};
let errors = typo.check(app.world()).unwrap_err();
assert_eq!(errors.to_string(), "there's no tag called `state.chilled` (in `chill`)");

// So is an effect code names that content doesn't define.
#[derive(Effect)]
struct Frostbite;
app.add_effect::<Frostbite>();
let errors = content.check(app.world()).unwrap_err();
assert_eq!(
    errors.to_string(),
    "code names effect `frostbite`, which content doesn't define"
);
# Ok::<(), Errors>(())
```

Mettle doesn't load files. The game reads them however it likes and hands
over the content: see [Loading data](loading-data.md).

## Numbers

As content is checked, each effect is numbered: an [`EffectId`], given in
order of name, as Bevy gives a component its `ComponentId`. Active effects,
chains, events and the network all go by the number. The name is for
content, debug output and what a client shows. The registry goes between
them:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Effect)] struct Burning;
let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_effect::<Burning>()
    .insert_resource(MettleContent {
        tags: ["cooldown", "cooldown.fireball"].map(TagKey::named).to_vec(),
        effects: vec![
            EffectData::lasting("burning", 4.0),
            EffectData::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
        ],
        ..default()
    });
app.update();

let registry = app.world().resource::<EffectRegistry>();
let burning = registry.id("burning").unwrap();
assert_eq!(registry.id_of::<Burning>(), Some(burning));
assert_eq!(registry.name(burning), Some("burning"));
assert!(registry.get(burning).is_some());
assert_eq!(registry.iter().count(), 2);
```

[`EffectRegistry`] reads as every registry does: `get(id)` gives the
checked effect, `id("name")` and `id_of::<E>()` its number, `name(id)` its
name, and `iter()` every one. [`EffectRegistry::recheck`] is the check on
its own, against an [`EffectChecks`] of the tag, curve, attribute,
execution, calculation and route registries; it keeps the numbers effects
had, and the effects code named with `add_effect`.

Numbered in order of name, the same content is numbered the same wherever
it's checked: a server and its clients agree on every number without
sending names.

## Reloading

Change [`MettleContent`], and it's checked and loaded again, as a whole.
Effects keep the numbers they had, and new ones are numbered after them, so
what holds a number still names the same effect. Every active effect takes
its new definition on the next tick. Its
modifiers and granted tags are worked out again, and its `Attribute`
magnitudes are captured again. Its time left is kept. An active effect
whose definition is gone comes off, as if removed.

That's a hot reload, or a mod adding its effects: change the content, and
everything is checked against everything again.

## Pitfalls

- **Damage is negative.** A `Flat` modifier adds. Burning's spec sets
  `param.damage` to a negative number, or the modifier says `-12.0`.
- **Add attributes first.** Content is checked against the app's
  attributes. Checked before `add_attribute`, it rejects every effect that
  names one.
- **A periodic effect's modifiers aren't buffs.** Each period changes the
  base, and the change stays. For a buff that comes and goes, leave out the
  period.

[`EffectRegistry::recheck`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html#method.recheck
[`EffectChecks`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectChecks.html
[`EffectRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`MettleContent::check`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html#method.check
[`ContentRejected`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.ContentRejected.html
[`EffectData`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/struct.EffectData.html
[`Effect`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.Effect.html
[`EffectId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.EffectId.html
[`EffectModifier`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/struct.EffectModifier.html
[`Magnitude`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html
[`Magnitude::scaled`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.scaled
[`Magnitude::param`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.param
[`Magnitude::source`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.source
[`Magnitude::target`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.target
[`Operation`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/enum.Operation.html
[`EffectBlocked`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectBlocked.html
