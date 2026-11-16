# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### `bevy_mettle_core`

- `Mettle`, the schedule mettle's gameplay runs in, and its steps,
  `MettleSet::{Effects, Attributes, Settle, Publish}`, in order.
  `MettleCorePlugin` runs it each fixed step, in `FixedPostUpdate` unless
  `MettleCorePlugin::new` says otherwise, from a system in `MettleSystems`,
  which game systems order themselves against. Every part's plugin adds it
  if it isn't there yet (`MettleCorePlugin::ensure`).
- Keys and ids, as Bevy's `ComponentId`: content names what it defines by
  a key, a `Key<K>` (`"fireball.cost"`), of one of mettle's kinds (`Kind`, an
  empty enum such as `AnyEffect`); checked, it's known by its number, an
  `Id<K>`. Keys are written as strings; ids aren't written. Code with a
  type for something names it as `Key::of::<T>()`, its kind inferred, by
  `Keyed` (its `Kind` and `KEY`), which the derives implement, so a typo
  doesn't compile. A type is of one kind: deriving two on one is rejected.
  Every trait code names content by (`Attribute`, `Effect`, `Ability`,
  `Behaviour`, `Execution`, `Calculation`, `Route`) requires it, its name
  written once, as `T::KEY`; `Registry::id_of::<T>()` takes only a type of
  its kind. `TagId`,
  `CurveId`, `AttributeId`, `EffectId` and `AbilityId`, with their `Any*`
  kinds, are in `registry`, so one part names another's without depending
  on it. Everything that runs, and everything sent, holds numbers; names
  stay for data, debug output and what's shown.
- `Registry<K, T>`, every registry's one shape, as Bevy's `Components`:
  entries by number, numbers by name and by type (`get`, `id`, `id_of`,
  `name`, `iter`), a key resolved to its number or an error saying where
  it was named (`resolve`, as `TagRegistry::resolve` for tags), entries added by a type known only at runtime
  (`add_type`), and a `fingerprint` of its names and numbers. Code names
  entries by type everywhere; a number comes back in events and replicas,
  read through the registry. Content
  reloaded keeps the numbers what's there had. Two types adding one name
  panic, naming both.
- Gameplay tags, typed and interned. `#[derive(Tag)]`, `#[tag(parent =
  State)]`: code names a tag by type, so a misspelled one doesn't compile;
  the `Tag` trait has its `SEGMENT`, `Parent` (`Root` at the top) and
  `key()`. `TagKey` is a tag interned process-wide, a `Copy` number with
  its name and parent, read without a lock. A tag type and its name give
  the same key, and wherever mettle takes a tag it takes `impl
  Into<TagKey>`. What has a tag has every tag above it (`is_within`,
  `lineage`).
- `TagRegistry`, the tags there are: content's, by name, and those code
  adds (`TagApp::add_tag`). `Tags`, a set of them. `TagCounts` and
  `OwnedTags`, what an actor owns by count, told by `TagAdded` and
  `TagRemoved`, with the `owns` guard.
- Curves defined in code: `#[derive(ScalingCurve)]` on a marker type,
  `#[curve(keys = [(1.0, 10.0), (10.0, 85.0)], ease = ..)]`, added with
  `add_curve::<C>()` (`CurveApp`). A `CurveData` is keys `(at, value)` and
  an `Interpolation` (`Constant`, `Linear`, `Ease`), built with
  `CurveData::new`, `key`, `keys`, `constant` and `ease`, into a
  `CurveRegistry` as Bevy's curves: values by key, read at the level or at
  what else a magnitude says. Content defining a curve with a code curve's
  key replaces it: what tuning is, as Unreal's curve tables are.
- Self-defining types: effects, abilities and curves define themselves,
  each derive key the builder method of the same name (`lasting = 4.0` is
  `.lasting(4.0)`, `flat::<Life>(5.0)` is `.flat::<Life>(5.0)`), or a
  function, `define = burning`. Each kind's registry keeps its types
  (`Definitions`) and defines them again as it's rebuilt; with the
  `hotpatching` feature, through Bevy's hot patching, so a patched
  definition is the one loaded. Content defining a key code defines is
  rejected (`Error::DefinedInCode`), but for a curve.
- Conditions: `Condition` (`ConditionOver<TagKey>`), an expression over a
  read's tags, what its owner, target and source own, and the facts it
  supplies (`fact_above`, `fact_below`, `fact_ramp`, `fact_every`), weighed
  from 0 to 1. Combined with `all`, `any`, `and` and `!`; weighed in a
  game's own `Context`; generic over their tags (`map_tags`), to go over
  the wire by `TagId`.
- `NetRole` (`Standalone`, `Server`, `Client`): what every part reads.
- `Activation`, what an ability's run, and what's done under it, is known
  by: a client's own (`Activation::client`), the server's own
  (`Activation::server`), or one derived from another's for an ability it
  triggers (`derived`), the same wherever it's derived. `is_servers` and
  `is_derived` say which; it's sent as a plain number.
- Checking content returns every `Error` at once, in `Errors`.

#### `bevy_mettle_attribute`

- Attributes as marker types: `#[derive(Attribute)]`, with its `base`, its
  limits (`min` and `max`, numbers or other attributes: `Limit`), its
  `rounding` (`Rounding`), its `on_limit` and its `name`. Added with
  `add_attribute` (`AttributeApp`) into the `AttributeRegistry`, each
  numbered (`AttributeId`); `world.attribute_id::<A>()` and
  `AttributeRegistry::id_of::<A>()` find it.
- `on_limit = Clamp | Ratio` (`OnLimit`): a base follows a limit that's
  another attribute as it moves, the same step. `Clamp`, the default, keeps
  it within its limits; `Ratio` keeps its fraction of its maximum.
- `Base<A>` and `Current<A>`: an actor has an attribute when it has its
  `Base`, the value the game and effects set. Its `Current` is the value
  with every modifier counted, within its limits, written only when it
  changes. `AttributeChanged` is told once a step, with the settled value.
- Modifiers: `Modifiers` on whatever holds them (an item, an effect, the
  actor), counting for the actor they're `ModifierOf` (`ModifierSources`).
  Each an `Operation`: `Base`, `Flat`, `Additive` (increased),
  `Multiplicative` (more), `Divide`, `Unscaled` or `Override`; counting
  while a condition holds, per so much of another attribute (`Per`), or
  only for its own source (`local`). Code writes them by type, as
  `ModifierSpec`s, put on with `modifiers(..)` (`ModifierCommands`) or
  made by hand with `resolve`; a `Modifier` is the resolved form, read
  back by number.
- Reads in context, by type: `AttributeReader` (`value`, `breakdown`) and
  `AttributeWorld` (`value`, `breakdown`, `base`, and `apply_to_base` and
  `base_after`, changing a base as an instant effect does) under an
  `AttributeContext` of a
  hit's tags, its target's and source's, and facts: a fire hit's
  resistance, counting modifiers for `damage.elemental`. A `Breakdown`
  says where a value came from. The `attribute_above` and
  `attribute_below` guards.
- `AttributeFold`, the schedule attributes are folded in, run from
  `MettleSet::Attributes`: bases synced, modifiers folded, values worked
  out, bases following their limits. `Current<A>` and `AttributeChanged`
  are published in `MettleSet::Publish`. `AttributePlugin` adds it.
- Served values over a network: a `Remote` actor shows the values its
  server serves (`Served<A>`), and its own while a `Provisional` source's
  modifiers count for it. `ModifiersReplica`, opt in on a modifier source
  the game replicates, carries its modifiers by `TagId` and the actor they
  count for (`Modifier<T>`, `map_tags`).

#### `bevy_mettle_effect`

- Effects defined in code: `#[derive(Effect)]` on a marker type, its keys
  `EffectData`'s builder methods, added with `add_effect::<E>()` or
  `add_effects::<(A, B)>()` (`EffectApp`, `EffectGroup`). An effect is a
  marker: the derive rejects fields, and what changes from one application
  to the next goes on its spec's context. Content adds effects by keys code
  doesn't define, as `EffectData`, a serde type.
- What an effect is, `EffectData`: instant unless it's `lasting` or
  `infinite` (`EffectDuration`), a `period` (and `execute_on_apply`),
  `modifiers` (`EffectModifier`; `flat::<A>` and `route::<R>` for those
  counting always), `executions` in code (`execution::<E>()`), the tags its owner holds while it lasts
  (`owner_tags`, `owner_tag(..)`), its `cues`, what it needs to apply
  (`require`) and to count (`ongoing`: inhibited while it doesn't, its time
  standing still), what ends it (`end_if`), the effects it `ends` as it
  applies and `blocks` while it lasts (Unreal's immunity), its `stacking`,
  and the effects it chains `on_apply`, `on_remove` and `on_expire`, each
  by type.
- Stacking (`Stacking::up_to(limit)`): one stack for everyone or per
  source (`StackBy`),
  a `limit`, what applying again does to the time left (`Reapply`) and the
  period (`reset_period`), what running out does (`Expire`), and the
  `overflow` effects applied at the limit. A stack keeps the spec its
  effect was first applied with; a new application's level, context and
  source go only to what it chains.
- `EffectRegistry`: the effects code defines and content adds, checked
  against the tags, curves, attributes, executions, calculations, routes
  and context data types (`EffectScope`), every error at once, and
  compiled into `EffectDef`s, names resolved to numbers.
  `EffectRegistry::build` makes one of content alone for tools and tests;
  `rebuild` defines the types again and keeps the numbers. Executions on a lasting effect with no period are
  rejected. Active effects take new definitions as they're replaced.
- Magnitudes as small expressions (`Magnitude`): terms `Value`, `Level`,
  `Context` (an `f32` field of a type the spec's context carries,
  Unreal's SetByCaller: `Magnitude::context::<Damage>("amount")`),
  `Attribute` (an `AttributeRead`), `Custom` (`calculation::<C>()`) and
  `Curve { curve, at }` (at the level unless it says), combined by `Sum`,
  `Product` and `Clamp`. Unreal's magnitude kinds are compositions of them: a scalable
  float is a value times a curve at the level; an attribute-based
  magnitude's coefficient and additions, a product and a sum, each part
  any magnitude; an attribute curve, a curve at an attribute; a custom
  calculation's coefficient, a product. Builders: `Magnitude::curve::<C>()`,
  `curve_at::<C>(..)`, `scaled::<C>(..)`, `times`, `plus` and `clamp`.
  Checked into a `MagnitudeDef` of numbers, its constants folded. Content
  is rejected for a sum or product of nothing, a clamp whose least is above
  its most, or a context field that isn't a registered type's `f32`.
- `AttributeRead`: which attribute an `Attribute` magnitude reads, whose
  (`Side`: `Source`, the default, or `Target`), which of its values (`Read`: `Current`,
  `Base`, or `Bonus`, its current value less its base), in what tags
  (Unreal's tag filters), and whether it's a snapshot. Started by
  `Magnitude::source::<A>()` or `target::<A>()`, then `.base()`,
  `.bonus()`, `.live()`, `.tagged(..)`, `.times(..)`, `.plus(..)`. Read as
  the hit it's part of: in the effect's tags and its own, with its target
  and source and what each owns.
- Snapshot and live captures, as Unreal's `bSnapshot`. A snapshot (the
  default) is taken at the earliest moment and kept: a source's as the
  spec is made, if its snapshot is taken, or as it's applied; a target's
  as it's applied. A live read is read as it's needed, and a lasting
  effect follows it, its modifiers worked out again in the step the
  attribute changes, only for the effects reading the actor that changed.
  Content is rejected for a lasting effect reading live what it changes
  of the target's.
- Live reads settle in the step they move: in `MettleSet::Settle`, effects
  whose live reads moved are worked out again and attributes folded again,
  until nothing moves, up to 8 rounds. What's left after 8 rounds follows
  on the next step, with a warning once.
- `EffectWorld::snapshot` and the `Snapshots` system param (`take`): a spec
  with its source's snapshot captures taken now, in its `Snapshot`, for a
  projectile to carry its caster's attributes as it was cast.
- Effect specs, `EffectSpec`: an `Effect` type is its own spec
  (`apply_effect(Burn.level(3.0).with(Damage { amount: 5.0 }))`), or
  `EffectSpec::of::<E>()`. Its `level`, its context, and the activation
  it's predicted under.
- An effect spec's context, Unreal's `FGameplayEffectContext`:
  `EffectContext`, of its `source`, and context data, one of each type,
  `.with(..)` and `get::<C>()`. Mettle's own: `Causer` (what struck),
  `SourceObject` (what it came from), `AbilityRef` (which ability, at what
  level), `Hit` (point and normal) and `Origin`, registered by
  `EffectPlugin`. A game adds its own with a `ContextData` type, by
  reflection (`#[reflect(ContextData)]`, `register_type`;
  `ReflectContextData`, `ContextDataRegistry`). Magnitudes, executions
  and calculations read it; chained effects carry it; `EffectExecuted` and
  `CueExecuted` are told it. It's replicated with an effect's replica and
  a one-shot cue's burst (`ContextReplica`), entities mapped, those in a
  game's types too.
- Applying and removing: `apply_effect`, `remove_effects` (by tags) and
  `remove_effects_under` (an activation), on `EffectCommands` and
  `EffectWorld`; `preview_effect`, what an instant effect would change
  without changing it. A lasting effect is an entity (`ActiveEffect`)
  `EffectOf` its target (`ActiveEffects`), with its `stacks`, `remaining`
  time, whether it `is_inhibited`, and whether it `is::<E>()`. Removing one is despawning it: its
  tags and cues taken back, `EffectRemoved` told, its `on_remove`
  chained.
- Events on the target: `EffectApplied`, `EffectStacked`, `EffectBlocked`
  (`Blocked`: `Requirement`, `Effect`, `StackLimit`), `EffectExecuted`
  and `EffectRemoved` (`expired`). `EffectExecuted` says what came of it:
  its `context`, the `changes` to the target's bases after routing and
  limits (`Delta`), `source_changes` to its source's, and what it routed
  into each route (`routed`, `Routed`). Each carries its effect's `key`, so
  an observer asks `is::<Burn>()`: an effect's own logic is an observer.
- Effect code: `#[derive(Execution)]` and `#[derive(Calculation)]`, their
  `ExecutionFn::execute` and `CalculationFn::calculate` taking `&self`, a
  value registered with `add_execution(FireDamage)` and
  `add_calculation(..)` into `ExecutionRegistry` and
  `CalculationRegistry`, each a `CodeRegistry` (as `RouteRegistry` is). An execution returns `BaseChange`s, named by
  type, `BaseChange::of::<Life>(op, value)` and
  `BaseChange::route::<IncomingDamage>(value)`, and numbered as they're
  applied: one to a type that isn't added is warned of once and dropped,
  and what it changes is read back by number with `changes()`. An
  execution routes into the routes it declares,
  `#[execution(routes(IncomingDamage, ..))]` (`Execution::routes`), each
  checked as added with the content; one routing into a route it doesn't
  declare has the change dropped, warned of once per execution and route.
  `BaseChange::on_source()` changes
  the source's base, life stolen back (`on: Side`, `Target` unless it
  says).
- Declared captures: `#[execution(source(A, ..), target(B, ..))]`,
  `#[calculation(..)]` and `#[route(..)]`, with the `AttributeRead`
  options (`base`, `bonus`, `live`, `tagged(..)`), as
  `Execution::captures`, `Calculation::captures` and `Route::captures`.
  They're the effect's captures like its magnitudes': snapshotted,
  replicated, predicted and checked. An effect captures what each route it
  routes into captures: those its modifiers name and its executions
  declare. A route whose captures can't be rejects only the effects
  routing into it.
- `ExecutionContext`, what executions, calculations and routers see: the
  effect, its `level`, its `context` (`get::<C>()`), `source`, the actors' `owned`
  tags, and `captured::<A>(Side)`, what the code declared it captures, as
  Unreal's `AttemptCalculateCapturedAttributeMagnitude`. `breakdown::<A>`
  and the `world` field read past captures, unsnapshotted and
  unpredicted.
- Routes: `#[derive(Route)]`, an amount effects route into that no actor
  holds (incoming damage), added with its router (`RouteFn::route`),
  `add_route(IncomingDamage)`, into `RouteRegistry`. The router turns all
  an effect routed into it into changes to attributes actors have: through
  shields into life. Single-pass, the same on a server and a predicting
  client, and seen by previews and ability costs. Routed into flat, by
  effects that execute: in data `(route: "incoming_damage", value: ..)`, in
  code `EffectModifier::route` or, from a declaring execution,
  `BaseChange::route`; never read, held or replicated.
- Effects tick in `MettleSet::Effects`, added by `EffectPlugin`: time runs
  down alongside the game, and only what's due is done with the whole
  world. Attributes fold what they changed in the same step.
- Cues (`cue`): what's shown of effects and abilities. Lasting cues held
  while an effect counts or an ability is active (`cues` in their data),
  or by `add_cue(tag)` and `add_unreplicated_cue(tag)`, let go of by
  `remove_cue(tag)`: each an entity (`ActiveCue`: its `tag`, `joined` for
  an actor come into view, `unreplicated`; `CueOf`, `ActiveCues`), observed
  as it's added and removed. One-shots trigger `CueExecuted` as an effect
  executes, with every change it made (`magnitude()`, the first's), or by
  `execute_cue` (`Cue`, `CueCommands`, `CueWorld`); unreplicated with
  `Cue::unreplicated`. Replicated as `ServedCues` and `CueBursts` (the
  last half second, played by a client coming into view), a predicted
  one-shot played once however often it was predicted. Played in
  `PostUpdate`, in `CueSystems`.
- Prediction: a server keeps each active effect's `EffectReplica`, its
  effect, context and source's tags by number, for a client to show
  (`Proxy`). A client predicts its own effects (`PredictedUnder`), kept
  or undone with `confirm_prediction` and `reject_prediction`.

#### `bevy_mettle_ability`

- Abilities defined in code: `#[derive(Ability)]` on a marker type, its
  keys `AbilityData`'s builder methods, added with `add_ability::<A>()` or
  `add_abilities::<(A, B)>()` (`AbilityApp`, `AbilityGroup`). Content adds
  abilities by keys code doesn't define, as `AbilityData`.
- What an ability is, `AbilityData`: the behaviour that runs it
  (`behaviour::<B>()`), what it costs (`cost`, `Cost::of::<Mana>(20.0)`)
  and how long it cools down (`cooldown`, `cooldown_tag` to share one),
  the effects that grant it (`granted_by::<E>()`), what it needs (`require`: kept from
  activating while its owner owns a tag by `!Condition::owner_has(..)`),
  what it `ends` (cancels) as it starts and `blocks` while active,
  its `owner_tags` (`owner_tag(..)`), its `cue`s, its `trigger`s
  (`Trigger`: `Event`, `TagAdded`, `TagPresent`), when it commits
  (`Commit`: `OnActivate`, `Manual`), what interrupts it (`end_if`) and how
  it runs over a network (`net`).
- Costs and cooldowns as effects mettle makes of them
  (`AbilityRegistry::upkeep`): an instant `<ability>.cost`, and a lasting
  `<ability>.cooldown` granting `cooldown.<ability>` unless it says,
  its tags registered with it. Granting takes an
  `AbilitySpec`, Unreal's ability spec: `grant_ability(Fireball)`,
  `grant_ability(Fireball.level(2.0))`. `activate_ability` takes an
  `Ability` type; `activate` a granted ability's entity.
- `AbilityRegistry`: the abilities code defines and content adds, checked
  against the tags, effects and behaviours (`AbilityScope`), every error at
  once, compiled into `AbilityDef`s; `granted_by` the effects granting
  each while they last, gear's skills (`GrantedBy`). An ability needs a
  behaviour, and one whose behaviour takes input can't have triggers.
- Behaviours: a component of yours, `#[derive(Behaviour)]`, added with
  `add_behaviour::<B>()` (`BehaviourApp`) into `BehaviourRegistry`. It's
  put on the ability's entity as it activates, for the game's systems to
  run, and taken off as it ends.
- Granting and activating: `grant_ability`, `activate_ability`,
  `activate_tagged` and `activate`, on `AbilityCommands` and
  `AbilityWorld`. A granted ability is an entity (`GrantedAbility`) of its
  actor (`AbilityOf`, `Abilities`), and of the effect that granted it, if
  one did (`GrantedBy`, `GrantedAbilities`); an active one has
  `ActiveAbility`. Revoking one is despawning it: active, it's cancelled
  and told as it ends. Activating checks it's granted, not cooling
  down, affordable, not blocked and its requirement holds, and returns an
  `Activated`: running here, or asked of the server. `AbilityActivated`,
  `AbilityFailed` (a `Failure`) and `AbilityEnded` are triggered on the
  actor.
- Committing: as it activates, paying before anything starts, or in parts
  (`commit_ability`, `commit_cost`, `commit_cooldown`), each once an
  activation; `GrantedAbility` says `is_cost_paid()`,
  `is_cooldown_started()` and `is_committed()`. Costs and cooldowns carry
  their ability in their context.
- Ending: `end_ability` and `cancel_ability`; `cancel_abilities(tags)` and
  `cancel_all_abilities()` on an actor. `end_if` cancels an ability as
  soon as it holds while active, and keeps it from activating
  (`Failure::Interrupted`). Over a network, the server interrupts.
- Asking without doing: `can_activate`, `can_afford` and
  `cooldown_remaining`, on `AbilityWorld`, from a system taking `&World`
  as from one with the world in hand.
- `ActiveAbility::effect_spec::<E>()`, Unreal's
  `MakeOutgoingGameplayEffectSpec`: a spec of an effect as a behaviour
  applies it, from its actor, struck by it (`Causer`), at its level, under
  its activation (so a rejected prediction takes it back), its context told
  the ability (`AbilityRef`).
- Ability input: what a player chose (an aim, a target), a component of
  yours a behaviour takes (`#[behaviour(input = Aim)]`, `B::Input`,
  `NoInput` if none), given with `activate_ability_with::<B>` or, while
  it runs, `set_input::<B>`, so only its input compiles. Checked by a
  system registered with `check_input` (`InputCheck`) before it's taken;
  predicted on a client and sent to the server, which checks it again
  (`Failure::Input`).
- Triggers: a `GameplayEvent` triggered on an actor (`commands.trigger`,
  `world.trigger`) activates what it triggers, as do tags it comes to
  own. It's about a `target`, and carries an `EffectContext` (its
  `source`, and context data, `with(..)`) for what it triggers to apply
  effects with. One sent under a running
  ability's activation (`GameplayEvent::under`) triggers a predicted
  ability under an activation derived from it, the same on a client and
  its server, undone with its parent while that awaits its answer.
- Network execution policies, as Unreal's: `NetExecution`. `Predicted`
  (the default) runs at once on a client and asks the server;
  `ServerOnly` is asked of the server and run there alone;
  `ServerInitiated` is run by the server, which tells the controlling
  client to run its behaviour too; `LocalOnly` runs where it's activated,
  is never sent, and can't have a cost or cooldown.
- The `net` messages and replica a transport carries: `AbilityRequest`
  and `AbilityStarted` (each carrying the ability's input, `NoInput` by
  default), `AbilityReply`, `AbilityInput`, `AbilityEnd`,
  `AbilityCancelRequest` and `AbilityReplica`. Endings go both ways: a server tells a client of the
  end of an ability run under its activation, and a client asks the
  server to cancel its own; a cancel that overtakes its request is held
  for it. Kept in `AbilitySystems`, in `PreUpdate`, added by
  `AbilityPlugin`.
- A prediction the server never answers is taken back after
  `PredictionTimeout` (5 seconds by default), as a rejection is, with
  what it triggered, failing `Failure::Unanswered`; a reply after that is
  ignored. `abandon_pending` takes back every one at once.

#### `bevy_mettle_lightyear`

- `MettleNetPlugin`: mettle's prediction over lightyear, on the server and
  its clients alike, by `NetRole`. Every active effect's and granted
  ability's replica replicated as their actor is; a client's requests
  sent on `MettleChannel` (reliable, ordered), taken only for an actor
  `ControlledBy` the client asking, and answered to the client
  controlling it; endings both ways, a cancel after the request it
  cancels.
- A client holds a reply confirming its prediction until its actor's
  state as of the reply has arrived, so what it predicted gives way to
  the server's in the same frame: never paid twice, or not at all.
- A client disconnecting takes back every prediction still awaiting its
  answer at once, each failing `Failure::Unanswered`, and drops the
  replies it holds.
- Owner-only replicas: an actor's effect and ability replicas go to the
  client controlling it alone, following control as it changes hands;
  every client that sees it gets its cues (`ServedCues`, `CueBursts`).
  `MettleNetPlugin { replicas: Replicas::Everyone }` sends them to all.
  Modifier sources carry their `ModifiersReplica`.
- `MettleNetApp`: `replicate_attribute`, an attribute's base and served
  value, and `replicate_input`, a behaviour's input from clients.
- `ContentFingerprint`: a server tells each client how it numbers its
  tags, curves, effects, abilities and context data types as it connects
  and as its content changes; a client numbering it otherwise logs an
  error and triggers `ContentMismatch`.
- Its prelude.

#### `bevy_mettle_macros`

- The derives, each re-exported by its part: `Tag` and `ScalingCurve` by
  `bevy_mettle_core`; `Attribute` by `bevy_mettle_attribute`; `Effect`,
  `Route`, `Execution` and `Calculation` (the last three with what they
  capture, an execution with the routes it routes into) by
  `bevy_mettle_effect`; `Ability` and `Behaviour` by
  `bevy_mettle_ability`. `Effect`, `Ability` and `ScalingCurve` take their
  keys as builder calls, checked by the compiler as the methods they
  call.

#### `bevy_mettle`

- `bevy_mettle`, a gameplay framework in the spirit of Unreal's GAS: a
  facade over `bevy_mettle_core` (always there, as `bevy_mettle::core`)
  and each part behind a feature: `attribute`, `effect` and `ability`, by
  default, and `lightyear`, as `bevy_mettle::attribute`, `effect`,
  `ability` and `lightyear`; and `hotpatching`, Bevy's hot patching of
  what code defines. Its prelude has every enabled part's.
- `MettlePlugins`: the plugin of every part its features enable, with
  `MettleCorePlugin` first and `MettleContentPlugin`. Run mettle elsewhere
  with `MettlePlugins.set(MettleCorePlugin::new(..))`. With `lightyear`,
  it carries `MettleNetPlugin`, and goes after lightyear's plugins.
- `MettleContentPlugin` loads what code defines and `MettleContent` adds,
  checked as a whole: in `PreStartup`, with or without content; in
  `PreUpdate` as content changes; and, with the `hotpatching` feature, as
  code is hot patched. `ContentLoaded` and `ContentRejected` (with every
  error) are triggered; what's rejected leaves what was loaded in place.
- `MettleContent`: what data adds as one resource (tags, curves, effects
  and abilities), serde types loaded however a game likes; its curves tune
  code's. `MettleContent::check` answers at once.
- Benchmarks of mettle's hot paths (`cargo bench -p bevy_mettle`):
  attributes kept and read, tags and conditions, effects applied, routed
  and ticked, captures, and abilities, for 1000 actors.

#### The book

- The bevy_mettle book: a guide to every part, from your first attributes
  to prediction over lightyear, adding content in data and testing, its examples
  compiled as doctests.
