# bevy_mettle_effect

`bevy_mettle`'s effects, as in Unreal's Gameplay Ability System: what
changes attributes and grants tags, instantly, for a while, for good, or
every so often.

```rust,ignore
let registry = EffectRegistry::build(&effects, &tags, &curves, &attributes, &code)?;
app.add_plugins(EffectPlugin::default())
    .insert_resource(registry);

commands.entity(target).apply_effect(
    EffectSpec::new("burning").level(3.0).param("param.damage", 12.0).source(caster),
);
```

## Definitions

An effect is data, named by an `Id<Effect>`: [`Effect`].

```ron
(
    id: "burning",
    tags: ["effect.affliction.burning"],
    duration: For(Value(4)),
    period: (every: Value(0.5), on_apply: true),
    modifiers: [(attribute: "life", op: Flat, value: Param("param.damage"))],
    grants: ["state.burning"],
    require: Not(OwnerHas("immune.fire")),
    ongoing: Not(OwnerHas("state.wet")),
    stacking: (limit: 3, expire: RemoveOne),
)
```

| Field | What it does |
|---|---|
| `duration` | `Instant` (executes once, changing bases), `Infinite`, or `For(..)` seconds |
| `period` | Executes every so often while it lasts, instead of its modifiers counting all along |
| `modifiers` | What it changes: as modifiers while it lasts, or on the base as it executes |
| `executions` | Formulas in code, run as it executes: damage |
| `grants` | Tags its target owns while it lasts |
| `require`, `ongoing`, `remove_if` | To apply at all; to count (inhibited while not); to come off |
| `removes`, `immunity` | Effects tagged so taken off as it applies; kept off while it lasts |
| `stacking` | By target or source, a limit, refresh or keep the time, what running out does, overflow |
| `on_apply`, `on_remove`, `on_expire` | Effects applied as it does |

A [`Magnitude`](Magnitude) is a `Value`, a curve `Scaled` by the
spec's level, a `Param` the applying code set (by a tag: `param.damage`,
as Unreal's `SetByCaller`), an `Attribute` of the source's or target's, or
a `Custom` calculation in code.

[`EffectRegistry::build`] checks every definition against the tag tree,
the curve table, the attributes the app added, and the code ([`EffectCode`]: executions and calculations
by name), every error at once. Mettle doesn't load anything: the game
reads its files and hands over the definitions. Insert a new registry (a
hot reload) and active effects take their new definitions.

## Applying

An [`EffectSpec`] is what's applied, and what's replicated: the effect,
its level, its params, its source, and the activation it's applied under.
`commands.entity(target).apply_effect(spec)` checks its requirement and
immunity, takes off what it removes, then executes it, stacks it onto one
already there, or spawns it.

## Active effects

A lasting effect is an entity [`EffectOf`] its target, which despawns with
it. While it counts, it carries its modifiers as `Modifiers` `ModifierOf`
the target, which the attribute crate folds, and the target owns what it
grants; while it's inhibited, neither, and its time stands still.
[`EffectPlugin`] ticks them in [`EffectSystems`] (`FixedUpdate`): removal
and ongoing requirements weighed, periods come round, time run out.

Stacked, an effect's amounts count once per stack; a base or an override
once.

## Told

[`EffectApplied`], [`EffectStacked`], [`EffectBlocked`],
[`EffectExecuted`] and [`EffectRemoved`] are entity events on the target:
for cues and abilities to observe.

## Over a network

[`NetRole`] says what a world is: `Standalone` (the default), `Server` or
`Client`. Mettle doesn't send anything; the game, or an adapter, ships
what it says to.

- **On a server**, each active effect keeps an [`EffectReplica`] (its
  spec, stacks, time left, whether it's inhibited) for the game to
  replicate with its [`EffectOf`]. Its definition is data both sides
  have, so it's named, not sent.
- **On a client**, an effect whose replica arrives is shown as a
  [`Proxy`]: counting, granting its tags, but not ticked; the server's
  replica says how it stands, and its despawn takes it off.
- **Predicting**: what a client applies under an activation is
  [`PredictedUnder`]. A lasting effect stays until the server's own replaces
  it. An instant change is never written to a base (the server owns
  bases, and replicates them): it's shown as a modifier until the server
  answers. `confirm_prediction` keeps what was predicted;
  `reject_prediction` undoes all of it. Abilities do both as the server
  answers.
