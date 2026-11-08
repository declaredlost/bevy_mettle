# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `bevy_mettle`, a gameplay framework in the spirit of Unreal's GAS: a
  facade over `bevy_mettle_core` (gameplay and owned tags, curves,
  conditions), `bevy_mettle_attribute` (attributes, with
  `bevy_mettle_macros`' derives), `bevy_mettle_effect` (effects) and
  `bevy_mettle_ability` (abilities).
- `bevy_mettle_lightyear`: `bevy_mettle`'s prediction over lightyear.
- `AttributeChanged` is `Clone`, `Copy` and `PartialEq`, as the other
  events are, whatever its attribute.
- `MettlePlugins`: the plugin of every part its features enable. With
  `lightyear`, it carries `MettleNetPlugin`, and goes after lightyear's
  plugins.
- `MettleSystems` and `MettleSystemsPlugin`: every part's set in it, in each
  schedule it runs in; abilities, then effects, then attributes where they
  share one.
- The facade's `lightyear` feature: `bevy_mettle_lightyear` as
  `bevy_mettle::lightyear`, in the prelude.
- `AttributePlugin::schedule` and `EffectPlugin::schedule`.
- `bevy_mettle_lightyear`'s prelude.
- The bevy_mettle book: a guide to every part, its examples compiled as
  doctests.
- `MettleContent`: the game's content as one resource, checked as a whole
  and loaded as it changes (`PreStartup`, then `PreUpdate`), by
  `MettleContentPlugin`, in `MettlePlugins`. `ContentLoaded` and
  `ContentRejected` (with every error) are triggered; rejected content leaves
  the content before in place. `MettleContent::check` answers at once.
- `add_execution` and `add_calculation` (`EffectCodeApp`): effect code
  registered on the app, as behaviours are; `EffectCode` is a resource.
- `TagTree` and `CurveTable` are resources: those the content was checked
  against.
- Ability input: what a player chose (an aim, a target), a component of
  yours a behaviour takes (`#[behaviour(input = Aim)]`), given with
  `activate_ability_with::<B>` or, while it runs, `set_input::<B>`, typed by
  the behaviour (`B::Input`), so only its input compiles; checked by a system
  registered with `check_input` before it's taken; predicted on a client and
  sent to the server (`AbilityRequestWith`, `AbilityInput`), which checks it
  again and rejects the activation if it doesn't hold. `Failure::Input`.
- `replicate_input` in `bevy_mettle_lightyear`.

### Changed

- Behaviours are `#[derive(Behaviour)]`, named as data names them, and added
  with `add_behaviour::<B>()`; their ids are `Id<AnyBehaviour>`.
- An ability whose behaviour takes input can't have triggers.
- `bevy_mettle_lightyear` answers a request to the client controlling its
  actor, rather than remembering who asked: later replies reach it too.

- `EffectRegistry::build` and `AbilityRegistry::build` are
  `EffectRegistry::check` and `AbilityRegistry::check`: the only way to make
  one, against what the definitions can name, as `EffectChecks` and
  `AbilityChecks`. A game hands over `MettleContent` instead.
- `Ability`, `Effect` and `AttributeApp` are only at `data::Ability`,
  `data::Effect` and `registry::AttributeApp`, not at their crates' roots too.
- Each part's prelude has its own crate's items: `bevy_mettle_attribute`'s
  no longer brings in core's tags, ids and conditions. `bevy_mettle`'s
  prelude has them all, as before.
- The attribute prelude brings in the `attribute_above` and
  `attribute_below` guards, and core's the `owns` guard.
- An effect registry rejects executions on a lasting effect with no
  period: it never executes.
- An active effect despawned by the game, or its `ActiveEffect` taken off,
  is told (`EffectRemoved`) and chains its `on_remove`, as `remove_effect`
  does. Effects despawned with their target, and a client's shown effects,
  tell nothing.

### Fixed

- An actor spawned this frame can pay for an ability: its cost is checked
  against its bases until its attributes are first worked out, rather than
  against nothing.

### Documented

- A stack keeps the spec its effect was first applied with; a new
  application's level, params and source go only to what it chains.
