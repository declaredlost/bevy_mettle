# Feature flags

## bevy_mettle

| Feature | Default | Enables |
|---|---|---|
| `attribute` | Yes | Attributes: `bevy_mettle_attribute` as `bevy_mettle::attribute`. Adds `AttributePlugin` to `MettlePlugins`. |
| `effect` | Yes | Effects: `bevy_mettle_effect` as `bevy_mettle::effect`. Adds `EffectPlugin` to `MettlePlugins`. Turns on `attribute`. |
| `ability` | Yes | Abilities: `bevy_mettle_ability` as `bevy_mettle::ability`. Adds `AbilityPlugin` to `MettlePlugins`. Turns on `effect`. |
| `lightyear` | No | Prediction over lightyear: `bevy_mettle_lightyear` as `bevy_mettle::lightyear`. Adds `MettleNetPlugin` to `MettlePlugins`, which then goes after lightyear's plugins. Turns on `ability`. |

`bevy_mettle_core` is always there, as `bevy_mettle::core`: gameplay tags,
curves and conditions. `MettleSystemsPlugin` and `MettleContentPlugin` are
always in `MettlePlugins`, and `MettleContent` holds what the enabled parts
define: tags and curves always, effects with `effect`, abilities and grants
with `ability`.

The prelude brings in every enabled part's prelude, with `MettlePlugins`,
`MettleSystems`, `MettleContent`, `ContentLoaded` and `ContentRejected`.
That's the derives and the app traits (`AttributeApp`, `TagApp`,
`EffectApp`, `BehaviourApp`, `AbilityApp`); the specs (`EffectSpec`,
`AbilitySpec`, `IntoAbilityId`); every registry and its numbers
(`TagRegistry` and `TagId`, `CurveRegistry` and `CurveId`,
`AttributeRegistry`, `EffectRegistry` and `EffectId`, `AbilityRegistry` and
`AbilityId`, and the rest); the data types with their `AnyEffect` and
`AnyAbility` names; and the replicas, `ModifiersReplica` among them.
`AnyCurve`, `AnyAttribute`, `AnyBehaviour`, `AnyExecution`,
`AnyCalculation` and `AnyRoute` are in their crates' modules, for code that
writes those ids out.

## The parts

The parts' crates have no features of their own. Each depends on the parts
before it: `bevy_mettle_effect` on `bevy_mettle_attribute`,
`bevy_mettle_ability` on both, and `bevy_mettle_lightyear` on all three.
