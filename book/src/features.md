# Feature flags

## bevy_mettle

| Feature | Default | Enables |
|---|---|---|
| `attribute` | Yes | Attributes: `bevy_mettle_attribute` as `bevy_mettle::attribute`. Adds `AttributePlugin` to `MettlePlugins`. |
| `effect` | Yes | Effects: `bevy_mettle_effect` as `bevy_mettle::effect`. Adds `EffectPlugin` to `MettlePlugins`. Turns on `attribute`. |
| `ability` | Yes | Abilities: `bevy_mettle_ability` as `bevy_mettle::ability`. Adds `AbilityPlugin` to `MettlePlugins`. Turns on `effect`. |
| `lightyear` | No | Prediction over lightyear: `bevy_mettle_lightyear` as `bevy_mettle::lightyear`. Adds `MettleNetPlugin` to `MettlePlugins`, which then goes after lightyear's plugins. Turns on `ability`. |
| `hotpatching` | No | Hot patching: Bevy's `hotpatching`, run under the Dioxus CLI. As code is hot patched, each effect, ability and curve type defines itself again and the registries are built again, so a change to a derive's keys or a `define` function shows while the game runs. |

`bevy_mettle_core` is always there, as `bevy_mettle::core`: gameplay tags,
curves and conditions, and the `Mettle` schedule. `MettleCorePlugin` and
`MettleContentPlugin` are always in `MettlePlugins`, and `MettleContent`
holds what the enabled parts let data add: tags and curves always, effects
with `effect`, abilities with `ability`.

The prelude brings in every enabled part's prelude, with `MettlePlugins`,
`MettleContent`, `ContentLoaded` and `ContentRejected`. That's what using
the parts takes: the derives and the app traits (`AttributeApp`, `TagApp`,
`CurveApp`, `EffectApp`, `BehaviourApp`, `AbilityApp`); the data types and
their builders (`EffectData`, `AbilityData`, `CurveData`, `Cost` and the
rest); the specs (`EffectSpec`, `AbilitySpec`); context data
(`ContextData` and mettle's own, such as `Causer`); the components, commands
and events; the registries content is read through (`TagRegistry`,
`CurveRegistry`, `AttributeRegistry`, `EffectRegistry`, `AbilityRegistry`)
with `Key`, `TagId`, `CurveId` and `AttributeId`; and `ModifiersReplica`.
Core's brings the schedule: `Mettle`, `MettleSet`, `MettleSystems` and
`MettleCorePlugin`.

What checking content takes, and what's sent over a network, are reached by
their modules:

- `bevy_mettle::core::registry`: the kinds and ids, such as `AnyEffect`,
  `EffectId`, `AnyAbility` and `AbilityId`.
- `bevy_mettle::effect::code`: `ExecutionRegistry`, `CalculationRegistry`,
  `RouteRegistry` and `CodeRegistry`.
- `bevy_mettle::effect`: `EffectScope`; `bevy_mettle::effect::net`:
  `EffectReplica`, `PredictedUnder` and `Proxy`.
- `bevy_mettle::ability`: `AbilityScope` and `Upkeep`;
  `bevy_mettle::ability::behaviour`: `BehaviourRegistry`;
  `bevy_mettle::ability::net`: `AbilityReplica`, `AbilityRequest`,
  `AbilityReply`, `AbilityInput`, `AbilityEnd` and
  `AbilityCancelRequest`.

## The parts

`bevy_mettle_core` has `hotpatching`, which `bevy_mettle`'s turns on. The
other parts' crates have no features of their own. Each depends on the parts
before it: `bevy_mettle_effect` on `bevy_mettle_attribute`,
`bevy_mettle_ability` on both, and `bevy_mettle_lightyear` on all three.
