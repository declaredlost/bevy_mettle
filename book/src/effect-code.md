# Effects in code

Most of an effect is data. Some numbers can't be: fire damage that grows with
the caster's spell power and shrinks with the target's fire resistance, or a
burn that lasts longer the stronger its caster. Those are code, named in the
data. Mettle has two kinds:

- **Executions** are formulas run as an effect executes. They return the
  changes to make to the target's bases. Unreal calls them execution
  calculations.
- **Calculations** work out one number, a [`Magnitude`] of `Custom`. They can
  stand in for any magnitude: a modifier's value, a duration, a period.

## Registering code

Code is registered on the app by name, with [`add_execution`] and
[`add_calculation`], as attributes and behaviours are. Data names it, and
content naming code that isn't registered is rejected:

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
# #[attribute(max = MaxResistance)]
# struct Resistance;
# #[derive(Attribute)]
# #[attribute(base = 0.75)]
# struct MaxResistance;
# #[derive(Attribute)]
# #[attribute(base = 10.0)]
# struct SpellPower;
fn code(app: &mut App) {
    app.add_execution("fire_damage", fire_damage)
        .add_calculation("burn_time", burn_time);
}

/// The spec's damage, more with the caster's spell power, less with the
/// target's fire resistance, scaled by the stacks of it.
fn fire_damage(effect: &EffectContext) -> Vec<BaseChange> {
    let damage = effect.param(&"param.damage".into()).unwrap_or_default();
    let power = effect
        .source()
        .and_then(|caster| effect.value::<SpellPower>(caster, &AttributeContext::new()))
        .unwrap_or_default();
    let fire = AttributeContext::new().tag("damage.elemental.fire");
    let resisted = effect.value::<Resistance>(effect.target, &fire).unwrap_or_default();
    let dealt = damage * (1.0 + power / 100.0) * (1.0 - resisted) * effect.stacks as f32;
    vec![BaseChange::new::<Life>(Operation::Flat, -dealt)]
}

/// Two seconds, and one more for every 25 spell power.
fn burn_time(effect: &EffectContext) -> f32 {
    let power = effect
        .source()
        .and_then(|caster| effect.value::<SpellPower>(caster, &AttributeContext::new()))
        .unwrap_or_default();
    2.0 + power / 25.0
}
# code(&mut App::new());
```

The effects name them:

```ron
[
    (
        id: "fireball.hit",
        tags: ["damage.elemental.fire"],
        duration: Instant,
        executions: ["fire_damage"],
    ),
    (
        id: "burning",
        tags: ["effect.affliction.burning"],
        duration: For(Custom("burn_time")),
        period: (every: Value(0.5)),
        executions: ["fire_damage"],
    ),
]
```

## The fireball hits

A caster with 25 spell power hits a mage with 50% fire resistance for 20:

```rust
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0, max = MaxLife)]
# struct Life;
# #[derive(Attribute)]
# #[attribute(base = 100.0)]
# struct MaxLife;
# #[derive(Attribute)]
# #[attribute(max = MaxResistance)]
# struct Resistance;
# #[derive(Attribute)]
# #[attribute(base = 0.75)]
# struct MaxResistance;
# #[derive(Attribute)]
# #[attribute(base = 10.0)]
# struct SpellPower;
# fn code(app: &mut App) {
#     app.add_execution("fire_damage", fire_damage).add_calculation("burn_time", burn_time);
# }
# fn power(effect: &EffectContext) -> f32 {
#     effect.source().and_then(|caster| effect.value::<SpellPower>(caster, &AttributeContext::new())).unwrap_or_default()
# }
# fn fire_damage(effect: &EffectContext) -> Vec<BaseChange> {
#     let damage = effect.param(&"param.damage".into()).unwrap_or_default();
#     let fire = AttributeContext::new().tag("damage.elemental.fire");
#     let resisted = effect.value::<Resistance>(effect.target, &fire).unwrap_or_default();
#     let dealt = damage * (1.0 + power(effect) / 100.0) * (1.0 - resisted) * effect.stacks as f32;
#     vec![BaseChange::new::<Life>(Operation::Flat, -dealt)]
# }
# fn burn_time(effect: &EffectContext) -> f32 {
#     2.0 + power(effect) / 25.0
# }
# fn tags() -> Vec<Tag> {
#     [
#         "damage", "damage.elemental", "damage.elemental.fire",
#         "effect", "effect.affliction", "effect.affliction.burning",
#         "param", "param.damage",
#     ]
#     .into_iter()
#     .map(|id| Tag { id: id.into() })
#     .collect()
# }
fn effects() -> Vec<Effect> {
    vec![
        Effect::instant("fireball.hit")
            .tag("damage.elemental.fire")
            .execution("fire_damage"),
        Effect::lasting("burning", Magnitude::Custom("burn_time".into()))
            .tag("effect.affliction.burning")
            .period(0.5, false)
            .execution("fire_damage"),
    ]
}
# let data: Vec<Effect> = ron::from_str(r#"[
#     (id: "fireball.hit", tags: ["damage.elemental.fire"], duration: Instant, executions: ["fire_damage"]),
#     (id: "burning", tags: ["effect.affliction.burning"], duration: For(Custom("burn_time")),
#      period: (every: Value(0.5)), executions: ["fire_damage"]),
# ]"#).unwrap();
# assert_eq!(effects(), data);
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .add_attribute::<Life>()
#     .add_attribute::<MaxLife>()
#     .add_attribute::<Resistance>()
#     .add_attribute::<MaxResistance>()
#     .add_attribute::<SpellPower>()
#     .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
# code(&mut app);
# app.insert_resource(MettleContent { tags: tags(), effects: effects(), ..default() });
# app.finish();
# app.cleanup();
# app.update();
let caster = app.world_mut().spawn(Base::<SpellPower>::new(25.0)).id();
let mage = app
    .world_mut()
    .spawn((
        Base::<Life>::default(),
        Base::<MaxLife>::default(),
        Base::<Resistance>::new(0.5),
        Base::<MaxResistance>::default(),
    ))
    .id();
// One fixed step, for their attributes to be worked out.
app.update();

let hit = EffectSpec::new("fireball.hit")
    .param("param.damage", 20.0)
    .source(caster);
app.world_mut().apply_effect(mage, hit);
// 20, 25% more, half resisted.
assert_eq!(app.world().get::<Base<Life>>(mage).unwrap().get(), 87.5);

let burn = EffectSpec::new("burning")
    .param("param.damage", 2.0)
    .source(caster);
app.world_mut().apply_effect(mage, burn);
let effect = app.world().get::<ActiveEffects>(mage).unwrap().iter().next().unwrap();
let remaining = app.world().get::<ActiveEffect>(effect).unwrap().remaining();
assert_eq!(remaining, Some(3.0), "two seconds, and one for 25 spell power");
```

## What code sees

An execution or a calculation gets an [`EffectContext`]: the world, read
only, and what's being applied.

| | What it is |
|---|---|
| `world` | The world, to read anything else |
| `target` | Who the effect is applied to |
| `spec` | The [`EffectSpec`] applied |
| `stacks` | How many stacks of it there are: 1 unless it stacks |
| `level()`, `param(..)`, `source()` | The spec's level, a param it set, and its source |
| `owned(entity)` | What an entity owns: its [`OwnedTags`] |
| `value::<A>(entity, read)` | An entity's attribute, read in context |
| `breakdown::<A>(entity, read)` | An entity's attribute's modifiers, summed but not applied |

`value_at` and `breakdown_at` do the same for an attribute named by id.
[Reading in context](reading.md) explains reads in context.

## Executions

An execution runs whenever its effect executes: as an instant effect applies,
or as a periodic effect's period comes round. It runs after the effect's own
modifiers have been worked out, and returns a list of [`BaseChange`]s. Each
names an attribute, an [`Operation`] and a value, and changes the target's
base as an executing modifier would: `Flat` adds, `Additive` scales,
`Override` sets. The base is then rounded and kept within its limits.

Executions don't scale with stacks by themselves, since they work out their
own numbers. Multiply by `stacks` where the effect should count once per
stack, as `fire_damage` does.

A breakdown lets an execution combine a caster's modifiers with a target's.
Damage increased by 30% on the caster, and damage taken increased by 20% on
the target, add up to 50% more, as they would if they were one attribute's:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
# #[derive(Attribute)] #[attribute(base = 10.0)] struct Damage;
# #[derive(Attribute)] #[attribute(base = 0.0)] struct DamageTaken;
fn strike(effect: &EffectContext) -> Vec<BaseChange> {
    let read = AttributeContext::new().tag("damage.elemental.fire");
    let (Some(caster), Some(taken)) = (
        effect.source().and_then(|caster| effect.breakdown::<Damage>(caster, &read)),
        effect.breakdown::<DamageTaken>(effect.target, &read),
    ) else {
        return Vec::new();
    };
    let mut hit = caster;
    hit.additive += taken.additive;
    vec![BaseChange::new::<Life>(Operation::Flat, -hit.total())]
}
# App::new().add_execution("strike", strike);
```

## Calculations

A calculation is a `Custom` magnitude. Anywhere a [`Magnitude`] goes, so can
`Custom("burn_time")`, or `Magnitude::Custom("burn_time".into())` in code.

When a calculation runs depends on what it stands for:

- A duration or a period is worked out once, as the effect applies.
- An executing modifier's value is worked out each time the effect executes.
- A lasting modifier's value is worked out as the effect starts counting, and
  again whenever its stacks change or it's reloaded.

A calculation is never captured, unlike an `Attribute` magnitude. If its
inputs change while the effect lasts, the next time it's worked out uses
them.

## Pitfalls

- **Executions need something to run on.** An effect with neither `Instant`
  duration nor a period never executes, so the registry rejects executions
  on it. Use a modifier, or give the effect a period.
- **Code has no side effects.** It reads the world and returns numbers. To
  spawn something or play a sound as an effect executes, observe
  [`EffectExecuted`].
- **Register code before content.** Content is checked against the code the
  app has: content naming code registered later is rejected. Registered
  code stays, for every reload.
- **Previews run code too.** [`EffectWorld::preview_effect`], which abilities
  use to check a cost, runs executions and calculations without applying
  anything.

[`EffectContext`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/struct.EffectContext.html
[`BaseChange`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/struct.BaseChange.html
[`add_execution`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/trait.EffectCodeApp.html#tymethod.add_execution
[`add_calculation`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/trait.EffectCodeApp.html#tymethod.add_calculation
[`EffectSpec`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectSpec.html
[`EffectExecuted`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectExecuted.html
[`EffectWorld::preview_effect`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.EffectWorld.html#tymethod.preview_effect
[`Magnitude`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html
[`Operation`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/enum.Operation.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
