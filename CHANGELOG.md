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
- `add_execution`, `add_calculation` and `add_route` (`EffectApp`): effect
  code added to the app, as behaviours are, into `ExecutionRegistry`,
  `CalculationRegistry` and `RouteRegistry`.
- `TagRegistry` and `CurveRegistry` are resources: those the content was
  checked against.
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
- Routes: `#[derive(Route)]`, an amount effects deal that no actor holds
  (incoming damage), added with its router, `add_route::<R>(router)`, which
  routes all an execution dealt it into attributes actors have.
  Single-pass, the same on a server and a predicting client, and seen by
  previews and ability costs. Dealt flat, by effects that execute: in data
  `(route: "incoming_damage", value: ..)`, in code `EffectModifier::route`
  or `BaseChange::route`; never read, held or replicated.
- `BaseChange::on_source()`: a change to the source's base, life stolen
  back; `Whose` says whose.
- `EffectExecuted` says what came of it: `source`, `changes` to the
  target's bases after routing and limits, `source_changes` to its
  source's, and what each route was dealt (`routed`).
- `on_limit = Clamp | Ratio` on an attribute: its base follows a limit
  that's another attribute as it moves, the same frame, on the world that
  decides attributes. `Clamp`, the default, keeps it within its limits;
  `Ratio` keeps its fraction of its maximum.
- Benchmarks of mettle's hot paths (`cargo bench -p bevy_mettle`):
  attributes kept and read, tags and conditions, effects applied, routed
  and ticked, for 1000 actors.
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

- Numbers, as Bevy's `ComponentId`: attributes, behaviours, routes,
  executions and calculations each get a number as they're added
  (`AttributeId`, `BehaviourId`, `RouteId`, `ExecutionId`,
  `CalculationId`, each a `Key` of its kind), and everything that runs
  knows them by it: actors' state, modifiers, changes, events and the
  network. Names stay for data, debug output and what a client shows.
  - Typed code finds them: `world.attribute_id::<A>()`,
    `AttributeRegistry::id_of::<A>()`, `BehaviourRegistry::id_of::<B>()`,
    `EffectContext::attribute_id` and `route_id`; typed reads
    (`value::<A>()`) do it themselves. `Attribute::ID` and `Behaviour::ID`
    are gone.
  - `Modifier`, `Per` and `BaseChange` name attributes by number. Code
    writes modifiers by type, as `ModifierSpec`s, and puts them on with
    `modifiers(..)` on `EntityCommands` or `EntityWorldMut`, which resolves
    them as Bevy resolves a component's type on insert; with a number in
    hand, `Modifier::flat(power, 5.0)`. `Modifier` is no longer written in
    data; an item's stats are an infinite effect.
  - Checking content compiles it: `EffectRegistry` holds `EffectDef`s and
    `AbilityRegistry` `AbilityDef`s, what their data names resolved to
    numbers. Active effects and granted abilities run by them.
  - Attribute limits name their attribute by type (`Limit::attribute`),
    resolved once both are added (`Bound`).
  - Attribute state is kept densely, by number, not hashed by name.
- Gameplay tags are typed and interned. `Id<Tag>` and the `Tag` struct are
  gone.
  - Code names a tag by type: `#[derive(Tag)] #[tag(parent = State)] struct
    Stunned;` is `state.stunned`, so a misspelled tag doesn't compile. The
    `Tag` trait has the tag's `SEGMENT`, its `Parent` (`Root` at the top)
    and its `key()`; a loop of parents panics, naming both, on first use.
  - `TagKey` is a tag interned process-wide, as Bevy interns labels: a
    `Copy` number with its name and parent. A tag type and its name give
    the same key. Wherever mettle takes a tag, it takes `impl Into<TagKey>`:
    a tag type or a name.
  - `TagCounts`, and so `OwnedTags`, count densely by key: asking is an
    index.
  - `TagApp::add_tag::<T>()` adds a tag code names, with those above it, to
    the `TagRegistry` content is checked against.
  - Content lists its tags as names. Data still reads and writes a tag as
    its name.
- Code names effects, abilities, executions and calculations by type, never
  by string. Content keeps naming them by name.
  - The data structs are `EffectData` and `AbilityData`. Content names are
    typed by `Any*` markers, as routes, attributes and behaviours were:
    `Name<AnyEffect>`, `Name<AnyAbility>`, `Name<AnyCurve>`,
    `Name<AnyExecution>`, `Name<AnyCalculation>`. Content structs hold their
    name in `name`, not `id`.
  - `#[derive(Effect)] struct Burning;` is the effect content defines as
    `burning`, added with `add_effect::<Burning>()`. The type is its spec:
    `apply_effect(Burning)`, `apply_effect(Burning.level(3.0).param(..))`.
    `EffectSpec` no longer holds a name: it's made from a type
    (`EffectSpec::of`) or a number (`EffectSpec::from_id`), and resolved as
    it's applied.
  - `#[derive(Ability)] struct Fireball;`, added with
    `add_ability::<Fireball>()`. Granting takes an `AbilitySpec`, Unreal's
    ability spec, as applying takes an effect spec: `grant_ability(Fireball)`,
    `grant_ability(Fireball.level(2.0))`, `AbilitySpec::from_id`.
    `activate_ability(Fireball)` takes an `Ability` type or an `AbilityId`
    (`IntoAbilityId`).
  - `#[derive(Execution)]` and `#[derive(Calculation)]`, added with
    `add_execution::<T>(f)` and `add_calculation::<T>(f)`, as routes are.
  - Content without an effect or ability code adds fails its check
    (`Error::NotInContent`), so a misspelled name is caught as content
    loads.
  - `EffectContext` has the effect's `def`.
- Content is numbered, as Bevy numbers components: `EffectId`, `AbilityId`,
  `CurveId` and `TagId`, by name, so the same content is numbered the same on
  a server and its clients.
  - Registries are kept by number; definitions, chains, grants, costs,
    cooldowns, active effects and events hold numbers. Names stay for
    content, debug output and what's shown.
  - `EffectRegistry::recheck`, `AbilityRegistry::recheck`,
    `CurveRegistry::rebuild` and `TagRegistry::rebuild` keep the numbers
    what's there had: content reloaded doesn't renumber what holds a number.
    Loading content does so.
  - `Stacking<E>` is named in data and numbered once checked, as durations
    and periods are.
  - Everything sent is numbers: `EffectReplica` carries the effect, its
    params and its source's tags by number; ability requests, replies and
    replicas carry `AbilityId`s; `ServedCues` and `CueBursts` carry
    `TagId`s. A cue whose tag isn't in the registry isn't replicated.
  - Conditions and modifiers are generic over their tags: `Condition` is
    `ConditionOver<TagKey>`, and `Modifier<T>` is `TagKey` by default; both
    have `map_tags`, and go over the wire over `TagId`s.
  - `ModifiersReplica`, opt in on a modifier source the game replicates,
    carries its modifiers by `TagId` and the actor they count for, kept by a
    server as either changes and as it's put on, and made into `Modifiers`
    and `ModifierOf` by a client. The lightyear adapter replicates it;
    `ModifierOf` itself isn't, so a client keeps its own on effects it shows.
  - Every registry has a `fingerprint` of its names and their numbers. Over
    lightyear, a server tells each client its `ContentFingerprint` as it
    connects and as its content changes; a client numbering content
    otherwise logs an error and triggers `ContentMismatch`.
- Names and numbers are told apart, each typed by a `Kind`
  (`#[derive(Kind)]`, an empty enum such as `AnyEffect`): `Id<T>` is
  `Name<K>`, what content names something by, and `Key<K>` is its number.
  `EffectId` is `Key<AnyEffect>`, and so on for every kind; `number!` and
  `IdTarget` are gone.
- Core is reached by its modules: names, keys and registries are all in
  `registry` (`Name`, `Kind`, `Key`, `Registry`). Nothing else is
  re-exported at its root but `Error` and `Errors`, which were
  `MettleError` and `MettleErrors`, and aren't in its prelude.
- Every registry is one shape, `Registry<K, T>` in core, as Bevy's
  `Components` is: entries of kind `K` by number, numbers by name and by
  type, and `get`, `id`, `id_of`, `name`, `iter` and `fingerprint` alike.
  - Two types adding something by one name panic, naming both: data
    couldn't tell them apart.
  - `EffectRegistry::check` and `AbilityRegistry::check` make a registry
    from nothing, for tools and tests; an app's content is `recheck`ed from
    its own registry, which knows the types code names.
  - `TagTree` is `TagRegistry`, `CurveTable` is `CurveRegistry` and
    `Behaviours` is `BehaviourRegistry`; `EffectCode` is split into
    `ExecutionRegistry`, `CalculationRegistry` and `RouteRegistry`.
  - What code names by type is kept in the registry itself: `CodeTags` is
    gone, and `EffectChecks` and `AbilityChecks` take no code.
- RON is no longer used: content is serde types, loaded however a game
  likes (a designed format is #9). Serde shapes are tested with
  `serde_test`.
- Tag keys read without a lock: a tag's name and parent are kept in an
  append-only table, so lineage walks and `is_within` don't contend across
  systems; only interning a new name locks. `TagRegistry::id` is an index by
  key.
- Every hash map mettle keeps uses Bevy's (`bevy::platform::collections`,
  foldhash) rather than the standard one: attribute frames and effect ticks
  take about half the time.
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
  with `add_behaviour::<B>()`; content names them by `Name<AnyBehaviour>`. They're in
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

- An effect chained by another (`on_apply`, `on_remove`, `on_expire`,
  overflow) keeps a spec naming itself, not the effect that chained it.
- An actor's attribute slots compare by what they hold: one taken off past
  the last no longer counts as a change.
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
