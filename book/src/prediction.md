# Prediction

In a multiplayer game the server decides. But a fireball that waits a round
trip before it casts feels sluggish. So a client *predicts* its own player's
abilities: it casts at once, pays, starts the cooldown, and asks the server.
The server answers. If it agrees, the prediction gives way to the server's
own state. If not, everything predicted is undone. This is how Unreal's
Gameplay Ability System does it too.

Mettle sends nothing itself. It says what to send, as messages and
components, and your transport ships them. [Over lightyear](lightyear.md)
does all of that for lightyear.

## Roles

[`NetRole`] is a resource that says what this world is:

| Role | What it does |
|---|---|
| `Standalone` | Plays alone: decides everything, replicates nothing. The default. |
| `Server` | Decides everything, and keeps what clients need to show it. |
| `Client` | Shows what the server sends, and predicts its own activations. |

Insert it on each side, before the game starts:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
let mut server = App::new();
server.add_plugins(MettlePlugins).insert_resource(NetRole::Server);

let mut client = App::new();
client.add_plugins(MettlePlugins).insert_resource(NetRole::Client);
```

## Values the server serves

An actor's attributes are worked out on both sides, from the same modifiers.
But only the server knows everything that counts: a buff from an item the
client never saw, say. So the server *serves* each value, and the client
shows it.

- **On the server**, an actor with a [`Served<A>`] keeps it equal to its
  `Current<A>`. That's the value to replicate.
- **On a client**, an actor marked [`Remote`] shows its `Served<A>` as its
  `Current<A>`, and as its reads with nothing in play, rather than folding
  the modifiers replicated to it.
- **While it predicts**, it shows its own fold instead. A source of modifiers
  marked [`Provisional`] is a prediction. While one counts for a `Remote`
  actor, the actor shows its own values, so the player sees the mana they
  just spent.
- **Reads in context**, like a fire hit's resistance, are worked out where
  they're read. On a client they're advisory: good for a tooltip, never for
  deciding.

[`AttributeChanged`] is told as the value shown changes, whichever it comes
from.

```rust
# use core::time::Duration;
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Life;
# let mut client = App::new();
# client
#     .add_plugins((MinimalPlugins, MettlePlugins))
#     .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(50)))
#     .insert_resource(NetRole::Client)
#     .add_attribute::<Life>();
// A mage the server says has 60 life, though its base says 100.
let mage = client
    .world_mut()
    .spawn((Base::<Life>::default(), Served::<Life>::new(60.0), Remote))
    .id();
client.update();
client.update();
assert_eq!(client.world().get::<Current<Life>>(mage).unwrap().get(), 60.0);
```

Who adds `Served<A>` and `Remote` is up to your transport. The lightyear
adapter adds `Served<A>` to every server actor with `Base<A>`, and `Remote` to
every actor a client receives.

## Effects from the server

- **On the server**, each active effect keeps an [`EffectReplica`]: its spec,
  stacks, how long it lasts and has left, whether it's inhibited, what its
  source owned as it applied, and the attribute values it captured.
  Replicate it with its [`EffectOf`], mapping its context's entities. The
  effect's definition is data both sides have, so it isn't sent: the
  replica carries the effect's number, an [`EffectId`], and its params and
  its source's tags as tag numbers, [`TagId`]s. The replica is sent again as
  it changes, but not as its time runs down.
- **On a client**, an effect whose replica arrives is shown as a [`Proxy`]: it
  counts, grants its tags, and its time runs down, but it isn't ticked
  otherwise. Its replica says how it stands. Its despawn takes it off.

## Modifier sources from the server

An effect's modifiers come with its replica. A source the game makes in
code, an item or a passive, has nothing like it unless the game opts in:
put a [`ModifiersReplica`] on the source and replicate it. The server keeps
it in step with the source's `Modifiers`, each modifier's condition naming
its tags by number, as a `Modifier<TagId>`, and the actor it's `ModifierOf`;
a client gives the source its `Modifiers` and `ModifierOf` from it.
[Modifiers](modifiers.md#over-a-network) shows it at work.

## Numbers on the wire

Everything mettle sends names what it's about by number, never by name:
effects by [`EffectId`], abilities by [`AbilityId`], tags by [`TagId`].
That's effect replicas (the effect, its params and its source's tags),
ability requests, replies and replicas, served cues and cue bursts, and
modifier replicas (each condition's tags). A
number is a few bytes where a name is a string, and both sides know it
already: content is numbered in order of name as it's checked, so a server
and its clients that load the same content number it the same. A reload
keeps the numbers things had and numbers new ones after them, so a reload
on both sides keeps them agreeing. Names stay for content, debug output and
what a client shows.

## Predicting an ability

On a client, `activate_ability` predicts:

1. The fireball activates at once, under a new [`Activation`]. Its
   behaviour runs, and its cost and cooldown are applied as predictions.
2. An [`AbilityRequest`] is written, as a message, for your transport to send
   to the server: the actor, the ability by its number, and the activation.
   One activated with [input](input.md) is an [`AbilityRequestWith`], the input too; new
   input as it runs is an [`AbilityInput`].

What's applied under a predicted activation is marked:

- **A lasting effect**, the cooldown, gets [`PredictedUnder`] and
  `Provisional`. It stays until the server's own replaces it: the same
  effect, from the same source, under the same activation.
- **A stack** predicted onto one of the server's effects is kept apart from
  the server's stacks.
- **An instant change**, the cost, is never written to a base. The server
  owns bases, and replicates them. The change is worked out on the base as
  shown, kept within its limits, and the difference shown as a flat
  modifier until the server answers. A cost dealt to a
  [route](effect-code.md#routes) is routed first, as the server routes it,
  and what's routed to the source is shown on the source the same way.

## Execution policies

How an ability runs over a network is its own: [`NetExecution`], the `net`
field of its definition, as in Unreal.

| Policy | On the client | On the server |
|---|---|---|
| `Predicted` (default) | Runs at once, and asks the server | Runs it, and confirms or rejects |
| `ServerOnly` | Asks the server, running nothing | Runs it; the client sees what it does through replicas and cues |
| `ServerInitiated` | Runs its behaviour as the server says it started, not paying or checking again | Runs it, however it started, and tells the controlling client: [`AbilityStarted`] |
| `LocalOnly` | Runs it, and sends nothing | Runs it only if it's the server's own |

```rust
# use bevy_mettle::prelude::*;
// A random proc: the client can't know the roll.
let proc = AbilityData::new("chaos_bolt", "cast_bolt").net(NetExecution::ServerOnly);
// Channelled from the server, aimed on the client.
let beam = AbilityData::new("beam", "channel_beam").net(NetExecution::ServerInitiated);
// Zoom: the player's own view.
let zoom = AbilityData::new("zoom", "zoom").net(NetExecution::LocalOnly);
# let _ = (proc, beam, zoom);
```

- **`Predicted`** is what a player's own abilities want: what follows is how
  it's predicted and resolved.
- **`ServerOnly`** suits outcomes a client can't know: a random proc, a hidden
  target, a teleport that would look wrong rejected. Activating one on a
  client comes to [`Activated::Asked`]: nothing runs there yet. Rejected, the
  client is told by [`AbilityFailed`], with nothing to undo; a client can't
  cancel it.
- **`ServerInitiated`** suits client-side logic on what the server decides: a
  channel the client aims, a camera following a dash the server started. The
  server pays; the client mirrors its activation, and its end. Asked for on a
  client, it comes to [`Activated::Asked`] until the server's start arrives.
  A client cancelling it asks the server to cancel it too. A predicted ability
  its mirrored behaviour triggers is the server's to decide: a client
  predicts only under its own activations.
- **`LocalOnly`** suits what's only the player's: zoom, an aim preview, a menu.
  It can't have a cost or a cooldown, which content checking rejects: what it
  paid, the server's replicated values would undo. Triggered, it runs where
  its trigger happened, by an event or a tag, on a client too. A server
  rejects a client asking for one.

## The server decides

On the server, each [`AbilityRequest`] is activated under the client's
activation, and answered with an [`AbilityReply`]: the same actor, ability
and activation, and `rejected` with a [`Failure`] if it didn't activate.
Input with a request is checked before anything's activated. New input to a
running activation is checked as it comes: if it doesn't hold, the server
cancels the ability and replies once more, rejecting the activation, and
the client cancels it too.

Each world counts its own activations: `Activation::client(n)` on a client
or a standalone world, `Activation::server(n)` on the server. The server's
own never clash with its clients'. A request under the server's activation,
or a derived one, is rejected, with a warning: a client can't pass its
activation off as the server's. Two clients' activations may be the same.
They're told apart by the actor they're for, the source of what they apply.

## The client resolves

When the client gets the reply back, as an [`AbilityReply`] message:

- **Confirmed**: what was predicted stays until the server's own effects
  replace it. The predicted cost and any stacks on the server's effects give
  way to the server's state. So hand the reply over only once the server's
  state as of its answer has arrived, or the cost shows unpaid until it does.
- **Rejected**: the ability is cancelled, everything predicted under it is
  undone, last first, and [`AbilityFailed`] says why. What its behaviour
  applied is undone only if it was applied under the activation: through
  [`ActiveAbility::effect_spec`](activating.md#running-the-behaviour).

Once an activation is answered, nothing more is predicted under it. What it
does from then on arrives from the server.

Here's a client predicting the fireball, and the server saying no:

```rust
# use core::time::Duration;
# use bevy::ecs::message::Messages;
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Mana;
# #[derive(Component, Default, Behaviour)]
# struct CastFireball;
# #[derive(Ability)] struct Fireball;
# let mut client = App::new();
# client
#     .add_plugins((MinimalPlugins, MettlePlugins))
#     .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(50)))
#     .insert_resource(NetRole::Client)
#     .add_attribute::<Mana>()
#     .add_behaviour::<CastFireball>()
#     .add_ability::<Fireball>();
# client.insert_resource(MettleContent {
#     tags: ["cooldown", "cooldown.fireball"].map(TagKey::named).to_vec(),
#     effects: vec![
#         EffectData::instant("fireball.cost").modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
#         EffectData::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
#     ],
#     abilities: vec![AbilityData::new(Key::of::<Fireball>(), Key::of::<CastFireball>()).cost("fireball.cost").cooldown("fireball.cooldown")],
#     ..default()
# });
# client.update();
# let mana = |app: &App, mage| app.world().get::<Current<Mana>>(mage).unwrap().get();
// The client's mage, as the server serves it.
let mage = client
    .world_mut()
    .spawn((Base::<Mana>::default(), Served::<Mana>::new(100.0), Remote))
    .id();
client.world_mut().grant_ability(mage, Fireball);
client.update();
client.update();

// Cast at once: paid, as a prediction, and asked of the server.
client.world_mut().activate_ability(mage, Fireball).unwrap();
let request = client
    .world_mut()
    .resource_mut::<Messages<AbilityRequest>>()
    .drain()
    .next()
    .unwrap();
// The request names the fireball by its number.
let fireball = client.world().resource::<AbilityRegistry>().id_of::<Fireball>();
assert_eq!(Some(request.ability), fireball);
client.update();
client.update();
assert_eq!(mana(&client, mage), 80.0);
assert_eq!(client.world().get::<Base<Mana>>(mage).unwrap().get(), 100.0);

// The server says no: the mana comes back, and the cooldown goes.
client.world_mut().write_message(AbilityReply {
    actor: request.actor,
    ability: request.ability,
    activation: request.activation,
    rejected: Some(Failure::Cooldown),
});
client.update();
client.update();
assert_eq!(mana(&client, mage), 100.0);
assert!(client.world().get::<ActiveEffects>(mage).is_none_or(|effects| effects.iter().count() == 0));
```

[`confirm_prediction`] and [`reject_prediction`] on the world are what the
client does with a reply. Abilities call them for you as replies arrive. Call
them yourself only for effects you predict under an activation of your own.

### Unanswered

A reply can go missing with the connection. A prediction still waiting after
[`PredictionTimeout`] (5 seconds by default) is taken back as a rejection is,
with what it triggered, and [`AbilityFailed`] says `Failure::Unanswered`. A
reply that comes after that is ignored: the server's state still arrives, as
it would anyway. [`abandon_pending`] takes back every prediction waiting at
once, for a transport that knows its connection is gone. Over lightyear, a
disconnect does that for you.

## Endings, both ways

A predicted ability runs on both sides, and the server decides how it ends:

- **The server ends it**, for a client's activation: done, interrupted by a
  stun only the server sees yet, its cost unpaid at commit, or revoked. It
  tells that client with an [`AbilityEnd`], and the client ends its copy.
- **The client cancels it**, its own prediction, with `cancel_ability` or
  `cancel_abilities`: a channel let go of, a cast called off. It asks the
  server to cancel its copy with an [`AbilityCancelRequest`].
- **The client ends it any other way**, and only its own copy ends. A cast
  that ends itself when its time is up ends on the client first, as the
  client runs ahead: the server's copy runs its course, to its last frame.
- **The client leaves to the server** what it would work out from what it
  predicts, which may be wrong. It doesn't interrupt: a stun it predicts
  may never land. A commit it can't pay from the mana it predicts returns
  `false` but leaves the ability running. The server weighs both, and its
  [`AbilityEnd`] ends the client's copy if it agrees.

Either side ignores an end of what isn't running any more. A cancel that
gets to the server before the request it cancels is held for it, up to a
second, and up to 16 for one actor. An end isn't a rejection: the cost stays paid and the cooldown runs, on both
sides. A server's own activations aren't told: a client never runs them.

## Cues

A client predicts cues with what plays them: a predicted cooldown's cue is
held at once, a predicted cost's one-shot played at once. When the server's
arrive, the lasting ones are one with the client's, with no flicker, and a
one-shot played under the client's own activation isn't played again.
[Cues](cues.md) has the rest.

## Granted abilities

On the server, each granted ability keeps an [`AbilityReplica`], its
number, an [`AbilityId`], and its level. Replicate it with its
[`AbilityOf`]. On a client, an ability whose replica arrives is granted from the client's own registry, and kept as its
replica changes, a level gained, say.

A client leaves triggers, and abilities effects grant, to the server. They
arrive as replicas like any other.

## Ordering

Ability requests, replies and replicas are handled in [`AbilitySystems`], in
`PreUpdate`. Hand your transport's messages to mettle before it.

## Pitfalls

- **One content, both sides.** Effects, abilities and tags are sent as
  numbers, not definitions. The server and its clients need the same
  content, loaded the same way, or the same number names different things.
  Over lightyear, a client that doesn't is told: see
  [Checking content](lightyear.md#checking-content).
- **Confirm late, not early.** A confirmed prediction gives way to the
  server's state at once. Confirm before that state arrives and the cost
  flickers back for a moment.
- **Only your own.** A client predicts what it activates itself. Everything
  else comes from the server, so don't trigger abilities on a client and
  expect them to run.

[`EffectId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.EffectId.html
[`AbilityId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.AbilityId.html
[`Activation`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/struct.Activation.html
[`TagId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.TagId.html
[`AbilityOf`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.AbilityOf.html
[`AbilityFailed`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.AbilityFailed.html
[`AbilityReplica`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityReplica.html
[`AbilityReply`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityReply.html
[`PredictionTimeout`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.PredictionTimeout.html
[`abandon_pending`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/fn.abandon_pending.html
[`AbilityEnd`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityEnd.html
[`AbilityCancelRequest`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityCancelRequest.html
[`AbilityRequest`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityRequest.html
[`AbilityRequestWith`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityRequestWith.html
[`AbilityInput`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityInput.html
[`AbilitySystems`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilitySystems.html
[`AttributeChanged`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/struct.AttributeChanged.html
[`confirm_prediction`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.EffectWorld.html#tymethod.confirm_prediction
[`EffectOf`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/active/struct.EffectOf.html
[`EffectReplica`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/net/struct.EffectReplica.html
[`Failure`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/enum.Failure.html
[`ModifiersReplica`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.ModifiersReplica.html
[`NetRole`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/net/enum.NetRole.html
[`PredictedUnder`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/net/struct.PredictedUnder.html
[`Provisional`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Provisional.html
[`Proxy`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/net/struct.Proxy.html
[`reject_prediction`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.EffectWorld.html#tymethod.reject_prediction
[`Remote`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/authority/struct.Remote.html
[`Served<A>`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/authority/struct.Served.html
[`NetExecution`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/data/enum.NetExecution.html
[`AbilityStarted`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityStarted.html
[`Activated::Asked`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/enum.Activated.html#variant.Asked
