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
  Replicate it with its [`EffectOf`], mapping the spec's source. The
  effect's definition is data both sides have, so it's named, not sent.
  The replica is sent again as it changes, but not as its time runs down.
- **On a client**, an effect whose replica arrives is shown as a [`Proxy`]: it
  counts, grants its tags, and its time runs down, but it isn't ticked
  otherwise. Its replica says how it stands. Its despawn takes it off.

## Predicting an ability

On a client, `activate_ability` predicts:

1. The fireball activates at once, under a new activation number. Its
   behaviour runs, and its cost and cooldown are applied as predictions.
2. An [`AbilityRequest`] is written, as a message, for your transport to send
   to the server: the actor, the ability, and the activation. One activated
   with [input](input.md) is an [`AbilityRequestWith`], the input too; new
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
  modifier until the server answers.

## The server decides

On the server, each [`AbilityRequest`] is activated under the client's
activation, and answered with an [`AbilityReply`]: the same actor, ability
and activation, and `rejected` with a [`Failure`] if it didn't activate.
Input with a request is checked before anything's activated. New input to a
running activation is checked as it comes: if it doesn't hold, the server
cancels the ability and replies once more, rejecting the activation, and
the client cancels it too.

Activation numbers count up in each world. The server's own have the top
bit set, so they never clash with its clients'. A request under an
activation with the top bit set is rejected, with a warning: a client can't
pass its activation off as the server's. Two clients' activations may share a
number. They're told apart by the actor they're for, the source of what they
apply.

## The client resolves

When the client gets the reply back, as an [`AbilityReply`] message:

- **Confirmed**: what was predicted stays until the server's own effects
  replace it. The predicted cost and any stacks on the server's effects give
  way to the server's state. So hand the reply over only once the server's
  state as of its answer has arrived, or the cost shows unpaid until it does.
- **Rejected**: the ability is cancelled, everything predicted under it is
  undone, last first, and [`AbilityFailed`] says why.

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
# let mut client = App::new();
# client
#     .add_plugins((MinimalPlugins, MettlePlugins))
#     .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(50)))
#     .insert_resource(NetRole::Client)
#     .add_attribute::<Mana>()
#     .add_behaviour::<CastFireball>();
# let tags: Vec<Tag> = ["cooldown", "cooldown.fireball"]
#     .into_iter()
#     .map(|id| Tag { id: Id::from_static(id) })
#     .collect();
# let tags = TagTree::build(&tags).unwrap();
# let effects = EffectRegistry::check(&[ Effect::instant("fireball.cost") .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)), Effect::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"), ], EffectChecks { tags: &tags, curves: &CurveTable::default(), attributes: client.world().resource::<AttributeRegistry>(), code: &EffectCode::new() })
# .unwrap();
# let abilities = AbilityRegistry::check(&[Ability::new("fireball", "cast_fireball") .cost("fireball.cost") .cooldown("fireball.cooldown")], &[], AbilityChecks { tags: &tags, effects: &effects, behaviours: client.world().resource::<Behaviours>() })
# .unwrap();
# client.insert_resource(effects).insert_resource(abilities);
# let mana = |app: &App, mage| app.world().get::<Current<Mana>>(mage).unwrap().get();
// The client's mage, as the server serves it.
let mage = client
    .world_mut()
    .spawn((Base::<Mana>::default(), Served::<Mana>::new(100.0), Remote))
    .id();
client.world_mut().grant_ability(mage, "fireball", 1.0);
client.update();
client.update();

// Cast at once: paid, as a prediction, and asked of the server.
client.world_mut().activate_ability(mage, "fireball").unwrap();
let request = client
    .world_mut()
    .resource_mut::<Messages<AbilityRequest>>()
    .drain()
    .next()
    .unwrap();
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

## Granted abilities

On the server, each granted ability keeps an [`AbilityReplica`], its id and
level. Replicate it with its [`AbilityOf`]. On a client, an ability whose
replica arrives is granted from the client's own registry, and kept as its
replica changes, a level gained, say.

A client leaves triggers, and abilities effects grant, to the server. They
arrive as replicas like any other.

## Ordering

Ability requests, replies and replicas are handled in [`AbilitySystems`], in
`PreUpdate`. Hand your transport's messages to mettle before it.

## Pitfalls

- **One registry, both sides.** Effects and abilities are named, not sent.
  The server and its clients need the same definitions.
- **Confirm late, not early.** A confirmed prediction gives way to the
  server's state at once. Confirm before that state arrives and the cost
  flickers back for a moment.
- **Only your own.** A client predicts what it activates itself. Everything
  else comes from the server, so don't trigger abilities on a client and
  expect them to run.

[`AbilityOf`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.AbilityOf.html
[`AbilityFailed`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.AbilityFailed.html
[`AbilityReplica`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityReplica.html
[`AbilityReply`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityReply.html
[`AbilityRequest`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityRequest.html
[`AbilityRequestWith`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityRequestWith.html
[`AbilityInput`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityInput.html
[`AbilitySystems`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilitySystems.html
[`AttributeChanged`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/struct.AttributeChanged.html
[`confirm_prediction`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.EffectWorld.html#tymethod.confirm_prediction
[`EffectOf`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/active/struct.EffectOf.html
[`EffectReplica`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/net/struct.EffectReplica.html
[`Failure`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/enum.Failure.html
[`NetRole`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/net/enum.NetRole.html
[`PredictedUnder`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/net/struct.PredictedUnder.html
[`Provisional`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Provisional.html
[`Proxy`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/net/struct.Proxy.html
[`reject_prediction`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.EffectWorld.html#tymethod.reject_prediction
[`Remote`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/authority/struct.Remote.html
[`Served<A>`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/authority/struct.Served.html
