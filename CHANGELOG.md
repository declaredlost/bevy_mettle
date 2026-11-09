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
- Interrupting abilities: `cancel_if` on an ability, a condition that
  cancels it as soon as it holds while active (weighed once per actor as
  its tags change, as it activates, and as its definition is replaced) and
  keeps it from activating (`Failure::Interrupted`). Over a network, the
  server interrupts; a client leaves it to the server.
  `cancel_abilities(tags)` and `cancel_all_abilities()` on an actor.
- Endings over a network: a server tells a client of the end of an ability
  run under its activation (`AbilityEnd`), and a client asks the server to
  cancel its own as it cancels it (`AbilityCancelRequest`); a cancel that
  overtakes its request is held for it. A client's other endings are its
  own: the server decides. `bevy_mettle_lightyear` carries both, a cancel
  after the request it cancels.
- Committing in parts: `commit_cost()` and `commit_cooldown()` beside
  `commit_ability()`, each once an activation; `GrantedAbility` says
  `is_cost_paid()` and `is_cooldown_started()`, each true where there's
  nothing to commit, and `is_committed()` once both are. A client that
  can't pay from what it predicts leaves the ability to the server.
- Asking without doing: `can_activate`, `can_afford` and
  `cooldown_remaining`, on `AbilityWorld` and the new `AbilityReader` system
  parameter.
- Meta attributes: `#[derive(MetaAttribute)]`, an amount effects deal that
  no actor has (incoming damage), added with its router,
  `add_meta_attribute::<M>(router)`, which routes all an execution dealt it
  into attributes actors have. Single-pass, the same on a server and a
  predicting client, and seen by previews and ability costs. Dealt flat, by
  effects that execute, with `EffectModifier::meta` or `BaseChange::meta`;
  never read, held or replicated.
- `BaseChange::on_source()`: a change to the source's base, life stolen
  back; `Whose` says whose.
- `EffectExecuted` says what came of it: `source`, `changes` to the
  target's bases after routing and limits, `source_changes` to its
  source's, and what each meta attribute was dealt (`routed`).
- `on_limit = Clamp | Ratio` on an attribute: its base follows a limit
  that's another attribute as it moves, the same frame, on the world that
  decides attributes. `Clamp`, the default, keeps it within its limits;
  `Ratio` keeps its fraction of its maximum.
- Cues (`bevy_mettle_effect::cue`): what's shown of effects and abilities.
  Lasting cues held while an effect counts or an ability is active (`cues`
  in their data), or by `add_cue`, each an entity (`ActiveCue`, `CueOf`);
  one-shots as an effect executes, with what it dealt to the first
  attribute it changed and every change (`CueChange`), or by `execute_cue`;
  `CueAdded` (`joined` for an actor come into view), `CueRemoved` and
  `CueExecuted`; `add_cue_handler`, run for the most specific tag; local
  cues; replicated as `ServedCues` and `CueBursts` (the last half second,
  played by a client coming into view), a predicted one-shot played once
  however often it was predicted.

### Changed

- `CueChange` is `Delta`, shared with `EffectExecuted`.
- `bevy_mettle_macros` has a file per derive.
- `bevy_mettle_ability::net` is split by what goes over the wire:
  `net::request` (`AbilityRequest`, `AbilityRequestWith`, `AbilityReply`),
  `net::input` (`AbilityInput`), `net::end` (`AbilityEnd`,
  `AbilityCancelRequest`) and `net::replica` (`AbilityReplica`). The prelude
  still has them all.
- `AbilityWorld::end_ability(ability, cancelled)` is `end_ability(ability)`
  and `cancel_ability(ability)`, as the commands are.
- `NetRole` is core's, `bevy_mettle_core::net::NetRole`: what every part
  reads, not the effect crate's alone.

- Active effects tick in two steps: time runs down and what's due is noted
  alongside the game (`advance`), and only what's due is done with the whole
  world (`settle`, run only then). Requirements are weighed as the tick's
  state is, whatever order effects come in. Content loading and showing
  replicas run only when there's something to do, so a quiet frame has no
  exclusive mettle systems.

- `MettleNetPlugin` sends an actor's effect and ability replicas to the
  client controlling it alone (a visibility filter, following control as it
  changes hands, the replicas still coming and going with their actor), and
  every client that sees it its cues;
  `MettleNetPlugin { replicas: Replicas::Everyone }` sends them to all.

- Behaviours are `#[derive(Behaviour)]`, named as data names them, and added
  with `add_behaviour::<B>()`; their ids are `Id<AnyBehaviour>`. They're in
  `bevy_mettle_ability::behaviour`, code apart from `data`, as effects'
  executions are in `effect::code`.
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

- An ability committing as it activates pays before it starts: before
  what it cancels, grants or cues, and before its behaviour. One that can't
  pay by then fails with `Failure::Cost` with nothing started, rather than
  activating unpaid.
- Effects and abilities let go of exactly what they started with: an effect
  taken off however it goes (despawned, replicated away) lets go of its
  cues, and an ability ended after its definition was replaced takes off
  the behaviour, tags and cues it activated with.
- An effect refreshed earlier in a tick doesn't run out in it: what's due is
  weighed as each effect stands when it's done.

- An actor spawned this frame can pay for an ability: its cost is checked
  against its bases until its attributes are first worked out, rather than
  against nothing.

### Documented

- A stack keeps the spec its effect was first applied with; a new
  application's level, params and source go only to what it chains.
