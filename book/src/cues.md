# Cues

A burning mage should look like it's burning. A fireball that lands should
flash, and say how much it dealt. Effects and abilities change numbers;
*cues* are what's shown of them, for your game to play: flames, sparks,
sounds, damage numbers. Mettle tells you when, and on whom. What's played
is yours.

## Two kinds

A cue is a gameplay tag, under a branch of your own such as `cue`:

- A **lasting** cue holds while something lasts: an effect while it counts,
  an ability while it's active. Flames while burning, a glow while casting.
- A **one-shot** cue plays at a moment: an effect executing, a hit. Sparks,
  a damage number, a puff each time a burn ticks.

Effects and abilities name the cues they play, in their `cues`, by tag:

```rust
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
let burning = EffectData::lasting("burning", 4.0)
    .period(0.5, false)
    .modifier(EffectModifier::new::<Life>(Operation::Flat, -3.0))
    .cue("cue.burning")
    .cue("cue.hit.fire");
# assert_eq!(burning.cues.written().count(), 2);
```

An effect holds its cues on its target while it counts, and plays them once
each time it executes. So burning shows flames while it lasts, and a puff
with each tick of damage. An ability holds its cues on its owner while it's
active. Cue tags are checked as content is: one that isn't in the tag
registry is an error.

## Lasting cues are entities

Each lasting cue on an actor is an entity of its own: an [`ActiveCue`] with
its tag, [`CueOf`] the actor. It's spawned as the cue starts and despawned
as it ends, or as the actor is. Observe it being added and removed, or
query it, and hang what you play on it: it goes when the cue does.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
/// Flames on every burning actor, for as long as it burns.
fn flames(added: On<Add<ActiveCue>>, cues: Query<&ActiveCue>, mut commands: Commands) {
    let Ok(cue) = cues.get(added.entity) else {
        return;
    };
    if cue.tag.is_within("cue.burning") && !cue.joined {
        commands.entity(added.entity).insert(Name::new("flames"));
    }
}

/// A last puff of smoke as the burning ends: the cue's still readable.
fn smoke(removed: On<Remove<ActiveCue>>, cues: Query<(&ActiveCue, &CueOf)>) {
    if let Ok((cue, of)) = cues.get(removed.entity)
        && cue.tag.is_within("cue.burning")
    {
        info!("smoke on {}", of.0);
    }
}
# App::new().add_observer(flames).add_observer(smoke);
```

Nothing has to remember to stop a looping effect: despawning the cue's entity
does. The actor's [`ActiveCues`] lists them.

An `ActiveCue` carries its `tag`; `joined`, whether it was going already as
this world came to see the actor; and `unreplicated`, whether only sources
a server doesn't replicate hold it. Its actor is its `CueOf`.

A cue held by two things, two burns or a burn and your own, is one entity,
held until both let go.

## One-shots trigger

[`CueExecuted`] is triggered on the actor as a one-shot plays, with what
the effect playing it changed, the level of what played it, what played it
carried (its [context](applying-effects.md#context): who caused it, and the
game's own data), the activation it was played under, and the effect, if
one did.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
/// A fire hit: embers and a damage number. Any other hit: a spark.
fn hits(played: On<CueExecuted>) {
    if played.tag.is_within("cue.hit.fire") {
        info!("embers on {}: {}", played.entity, -played.magnitude());
    } else if played.tag.is_within("cue.hit") {
        info!("spark on {}", played.entity);
    }
}

#[derive(Effect)]
#[effect(name = "fireball.hit")]
struct FireballHit;

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_attribute::<Life>()
    .add_effect::<FireballHit>()
    .add_observer(hits)
    .insert_resource(MettleContent {
        tags: ["cue", "cue.hit", "cue.hit.fire"]
            .into_iter()
            .map(TagKey::named)
            .collect(),
        effects: vec![EffectData::instant(Key::of::<FireballHit>())
            .modifier(EffectModifier::new::<Life>(Operation::Flat, -15.0))
            .cue("cue.hit.fire")],
        ..default()
    });
app.update();
let mage = app.world_mut().spawn(Base::<Life>::default()).id();
app.world_mut().apply_effect(mage, FireballHit);
```

`is_within` matches a tag and every tag under it: `cue.hit` matches a fire
hit too. Every observer sees every cue, so to play only the most specific,
check the child tag first, as `hits` does.

`magnitude()` is what the execution changed the first attribute it changes
by, the one the effect is about: a fireball's hit for 15 is `-15.0`. Every
attribute it changed is in `changes`, in the order it first changed each,
for a drain that shows the life it took and the mana it gave. Where it hit
is carried if the applying code put a [`Hit`] on the context:
`played.context.get::<Hit>()`. Otherwise, read positions from the actor and
the source in the world showing the cue, so the spark lands where that world
shows the target.

## Cues from code

Not everything shown is an effect. Play cues yourself, on an actor, with
[`execute_cue`], [`add_cue`] and [`remove_cue`]:

```rust
# use bevy::prelude::*;
# use bevy::ecs::entity::MapEntities;
# use bevy_mettle::prelude::*;
# use serde::{Deserialize, Serialize};
/// How hard something landed: what a landing's cue plays by.
#[derive(Reflect, Clone, PartialEq, Serialize, Deserialize, MapEntities)]
#[reflect(ContextData)]
struct Impact(f32);

/// Called as the mage lands hard.
fn land(commands: &mut Commands, mage: Entity) {
    // A one-shot, with how hard it landed as its own data.
    commands.entity(mage).execute_cue(Cue::new("cue.land").with(Impact(8.0)));
    // A lasting cue, held until removed as often as it was added.
    commands.entity(mage).add_cue("cue.dust");
}
```

A one-shot from code changes nothing, so its `magnitude()` is 0: what it
plays by is its [context data](applying-effects.md#context), registered
(`app.register_type::<Impact>()`) so a server can send it.

`remove_cue(tag)` lets go of one source of the tag, whichever holds it,
and warns if none does. [`add_unreplicated_cue`] and
[`Cue::unreplicated`] keep a cue in the world that plays it: a server
doesn't send it to its clients. On a client, cues from code are always its
own: a client can't send them.

## Over a network

A server's cues reach the clients that see the actor, with the actor:

- **Lasting cues**, the server's [`ServedCues`], are replicated: a handful
  of tags, each sent as its number, a `TagId`, in the [tag
  registry](tags.md#the-tag-registry),
  not the effects. A client shows them next to its own, so what it
  predicts and what the server serves are one entity, with no flicker
  between.
- **One-shots** the server plays are replicated as [`CueBursts`]: those of
  the last half second, numbered. A client plays each once. One it played
  itself, predicting under its activation, isn't played again, however
  many times it played it; if the server rejects the activation, or never
  plays it, the client forgets it. A burst names its cue by its tag's
  number too, and its effect, if one played it, by its [`EffectId`].

A cue whose tag isn't in the tag registry has no number to send, so it isn't
replicated: the server warns, and plays it only for itself. Cues from code
are tags like any other, so define them in content or add them with
`add_tag`.

A client coming to see an actor joins what's going: its lasting cues are
spawned with `joined` set, for a quiet start rather than a burst, and it plays
the one-shots of the last half second, what's just happened: a projectile's
launch, spawned with it. Older ones are past.

Since other clients see cues, they needn't see the effects themselves.
[Over lightyear](lightyear.md) sends an actor's effect replicas to its owner
alone, unless you say, following control as it changes hands.

## Pitfalls

- **One tag, both kinds.** An effect's cue is held while it counts and
  played as it executes. A periodic burn with one cue gets both: give it two
  tags if they should look different.
- **A cue has to be a tag in the registry.** One named in content is checked as
  content loads. One from code that isn't defined is played where it's
  made, but never sent.
- **Cues are shown late in the frame.** Lasting cues are worked out in
  `PostUpdate`, in [`CueSystems`]. One-shots play at once.
- **Cue observers are for showing.** They run where the cue is shown, a
  client too. Change gameplay in effects and abilities, not in cues.

[`ActiveCue`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/struct.ActiveCue.html
[`CueOf`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/struct.CueOf.html
[`ActiveCues`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/struct.ActiveCues.html
[`CueExecuted`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/struct.CueExecuted.html
[`Hit`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/context/struct.Hit.html
[`execute_cue`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/trait.CueCommands.html#tymethod.execute_cue
[`add_cue`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/trait.CueCommands.html#tymethod.add_cue
[`remove_cue`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/trait.CueCommands.html#tymethod.remove_cue
[`add_unreplicated_cue`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/trait.CueCommands.html#tymethod.add_unreplicated_cue
[`Cue::unreplicated`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/struct.Cue.html#method.unreplicated
[`ServedCues`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/struct.ServedCues.html
[`CueBursts`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/struct.CueBursts.html
[`EffectId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.EffectId.html
[`CueSystems`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/cue/struct.CueSystems.html
