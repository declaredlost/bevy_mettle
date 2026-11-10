# Granting and activating

An actor can only use abilities it's been granted. Granting gives it an
entity for the ability. Activating runs it. Your behaviour system does the
work and ends it.

## Granting

Code names an ability by its type, `#[derive(Ability)] struct Fireball;`,
added with `app.add_ability::<Fireball>()`: see [Defining
abilities](abilities.md#naming-abilities-in-code). Grant the fireball to the
mage:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Ability)]
struct Fireball;

fn spawn_mage(mut commands: Commands) {
    commands
        .spawn((Base::<Life>::default(), Base::<Mana>::default()))
        .grant_ability(Fireball);
}
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
```

Granting takes an [`AbilitySpec`]: which ability, and at what level. The
ability type is its own spec, at level 1, so `grant_ability(Fireball)`
grants it at level 1 and `grant_ability(Fireball.level(2.0))` at level 2.

Activating takes an [`Ability`] type: `activate_ability(Fireball)`. Code
that doesn't know the type, such as a hotbar listing whatever the actor was
granted, activates the granted ability's entity: `activate(ability)`.

The granted ability is an entity of its own, with a [`GrantedAbility`] and
an [`AbilityOf`] pointing at the mage. The mage gets [`Abilities`], the list
of what it's been granted. Despawning the mage despawns its abilities.

[`GrantedAbility`] tells you which ability it is (`id()`, its number, and
`def()`), its level
(`level()`), and whether
this activation has paid its cost (`is_cost_paid()`), started its cooldown
(`is_cooldown_started()`), or both (`is_committed()`). The level is what the
cost's and cooldown's curves are read at. The effect that granted it, if one
did, is its [`GrantedBy`].

Granting the same ability twice gives the actor two entities. Activating by
type or number uses the first.

## Activating

`activate_ability(Fireball)` on the mage activates its first granted
fireball. An ability whose behaviour takes input, such as an aim, is
activated with it: `activate_ability_with::<CastFireball>(Fireball,
Aim(at))`. [Input and targets](input.md) covers that. Mettle checks, in this order:

1. **Granted.** The mage has a fireball, and it isn't active already.
   Otherwise `Failure::NotGranted` or `Failure::Active`.
2. **Not cooling down.** The mage owns none of the cooldown effect's owner
   tags.
   Otherwise `Failure::Cooldown`.
3. **Affordable.** Paying the cost leaves no attribute below 0. Otherwise
   `Failure::Cost`.
4. **Not blocked.** No other active ability `blocks` the fireball's tags.
   Otherwise `Failure::Blocked`.
5. **Required.** `require` holds. Otherwise `Failure::Requirement`.
6. **Not interrupted.** `end_if` doesn't hold. Otherwise
   `Failure::Interrupted`.

Then it activates:

- with `Commit::OnActivate`, its cost and cooldown are applied first, so
  its behaviour starts paid for;
- active abilities it `ends` are cancelled;
- the mage owns its `owner_tags`;
- [`ActiveAbility`] is put on the ability's entity, with its behaviour's
  component;
- [`AbilityActivated`] is triggered on the mage.

If a check fails, nothing happens but [`AbilityFailed`], triggered on the
mage with the reason.

Here's the whole round, run on the world directly:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Tag)] struct State;
# #[derive(Tag)] #[tag(parent = State)] struct Casting;
# #[derive(Tag)] struct Cooldown;
# #[derive(Tag)] #[tag(parent = Cooldown, name = "fireball")] struct FireballCooldown;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Mana;
# #[derive(Component, Default, Behaviour)]
# struct CastFireball;
# #[derive(Ability)] struct Fireball;
# use core::time::Duration;
# use bevy::time::TimeUpdateStrategy;
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(100)))
#     .add_attribute::<Mana>()
#     .add_behaviour::<CastFireball>()
#     .add_ability::<Fireball>();
# let fireball = AbilityData::new(Key::of::<Fireball>(), Key::of::<CastFireball>())
#     .cost("fireball.cost")
#     .cooldown("fireball.cooldown")
#     .owner_tag("state.casting");
# app.insert_resource(MettleContent {
#     tags: ["state", "state.casting", "cooldown", "cooldown.fireball"].map(TagKey::named).to_vec(),
#     effects: vec![
#         EffectData::instant("fireball.cost").modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
#         EffectData::lasting("fireball.cooldown", 2.0).owner_tag("cooldown.fireball"),
#     ],
#     abilities: vec![fireball],
#     ..default()
# });
let mage = app.world_mut().spawn(Base::<Mana>::default()).id();
// A tick, so its attributes are worked out.
app.update();
app.update();
let world = app.world_mut();

// Not granted yet.
assert_eq!(world.activate_ability(mage, Fireball), Err(Failure::NotGranted));

world.grant_ability(mage, Fireball);
let ability = world.activate_ability(mage, Fireball).unwrap().ability();

// Paid, casting, cooling down, and running its behaviour.
assert_eq!(world.get::<Base<Mana>>(mage).unwrap().get(), 80.0);
let owned = world.get::<OwnedTags>(mage).unwrap();
assert!(owned.contains(Casting));
assert!(owned.contains(FireballCooldown));
assert!(world.get::<CastFireball>(ability).is_some());

// Once is enough.
assert_eq!(world.activate_ability(mage, Fireball), Err(Failure::Active));

// Ended: no longer casting, but still cooling down.
world.end_ability(ability);
assert!(world.get::<CastFireball>(ability).is_none());
let owned = world.get::<OwnedTags>(mage).unwrap();
assert!(!owned.contains(Casting));
assert_eq!(world.activate_ability(mage, Fireball), Err(Failure::Cooldown));

// Two and a half seconds later, the cooldown has run out.
for _ in 0..25 {
    app.update();
}
assert!(app.world_mut().activate_ability(mage, Fireball).is_ok());
```

## Running the behaviour

Your system finds the ability by its behaviour's component, and ends it when
it's done. The fireball takes half a second to cast:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Component, Default, Behaviour)]
struct CastFireball {
    timer: f32,
}

fn cast_fireball(
    mut casting: Query<(Entity, &ActiveAbility, &mut CastFireball)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (ability, active, mut cast) in &mut casting {
        cast.timer += time.delta_secs();
        if cast.timer > 0.5 {
            // Spawn the projectile from `active.actor` here.
            commands.entity(ability).end_ability();
        }
    }
}
```

[`ActiveAbility`] says which ability it is, who it's active for (`actor`),
at what level, under which activation, and the gameplay event that triggered
it, if one did.

A behaviour applies effects through it, with `effect_spec`, Unreal's
`MakeOutgoingGameplayEffectSpec`. The spec it makes is from the actor,
struck by it, at the ability's level, under this activation, its
[context](applying-effects.md#context) told the ability, an `AbilityRef`.
A projectile says it struck instead, with `.with(Causer(..))`:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Effect)] #[effect(name = "fireball.hit")] struct FireballHit;
#[derive(Component, Default, Behaviour)]
struct Fireball {
    target: Option<Entity>,
    projectile: Option<Entity>,
}

fn hit(fireballs: Query<(&ActiveAbility, &Fireball)>, mut commands: Commands) {
    for (active, fireball) in &fireballs {
        if let (Some(target), Some(projectile)) = (fireball.target, fireball.projectile) {
            commands
                .entity(target)
                .apply_effect(active.effect_spec::<FireballHit>().with(Causer(projectile)));
        }
    }
}
```

It takes the effect by type, and the builders set more after:
another level, params, what struck. Apply what an ability does this way, not
with a spec of your own. Under the
activation, a client's prediction of it is taken back if the server rejects
the activation ([Prediction](prediction.md)); one applied without it stays.
The ability's cost and cooldown are applied the same way.

A projectile that lands later should hit with its caster's attributes as it
was cast. Take the spec's snapshot as it's made, `snapshots.take(spec)`, and
carry it: see [Snapshots](applying-effects.md#snapshots).

Add it like any other system:

```rust,no_run
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Component, Default, Behaviour)] struct CastFireball;
# fn cast_fireball() {}
fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MettlePlugins))
        .add_behaviour::<CastFireball>()
        .add_systems(Update, cast_fireball)
        .run();
}
```

## Committing, ending, cancelling, revoking

These are commands on the ability's entity:

| Command | What it does |
|---|---|
| `commit_ability()` | Pays the cost and starts the cooldown, for a `Commit::Manual` ability. One that can't pay is cancelled. |
| `commit_cost()` | Pays the cost alone. One that can't is cancelled. |
| `commit_cooldown()` | Starts the cooldown alone. |
| `end_ability()` | Ends it: its behaviour's component taken off, the tags it granted taken back. On a client, its own prediction alone. |
| `cancel_ability()` | The same, but told as cancelled. On a client, the server's copy is cancelled too. |

Ending an ability doesn't refund anything. The cost is paid and the cooldown
runs its course.

Revoking an ability is despawning it: `commands.entity(ability).despawn()`,
or `world.despawn(ability)`. One that's active is cancelled first, as
`cancel_ability` ends it: its behaviour's component and the tags and cues it
held taken back, [`AbilityEnded`] told, and on a server, the client running
it told too. So is one whose entity loses its [`GrantedAbility`] some other
way, such as replication taking it away. One despawned with its actor tells
nothing: there's no one left to tell.

On the actor, the commands are `grant_ability`, `activate_ability`,
`activate_tagged`, and `cancel_abilities(tags)` and `cancel_all_abilities()`: every active
ability tagged any of `tags` cancelled, or every one, as the actor dies.
In an exclusive system or a test, [`AbilityWorld`] does the same on the
`World` at once, and says how it went: `activate_ability` returns the
ability or the [`Failure`], `commit_ability` whether it could pay;
`end_ability` and `cancel_ability` end an ability as the commands do.

## Asking

A UI greys out what can't be cast and sweeps cooldowns; an AI picks what it
can cast. [`AbilityWorld`] asks about a granted ability with nothing done,
from a system taking `&World`:

- `can_activate(ability)`: every check activating it makes, `Ok` or the
  [`Failure`] it would fail with;
- `can_afford(ability)`: whether its owner could pay its cost;
- `cooldown_remaining(ability)`: the seconds left on its cooldown, `None` if
  it isn't cooling down, and `f32::INFINITY` if nothing that cools it runs
  out. It says cooling down exactly when `can_activate` does.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
fn buttons(abilities: Query<Entity, With<GrantedAbility>>, world: &World) {
    for ability in &abilities {
        let usable = world.can_activate(ability).is_ok();
        let sweep = world.cooldown_remaining(ability).unwrap_or(0.0);
        // Grey the button out unless usable; sweep it by what's left.
#       let _ = (usable, sweep);
    }
}
# App::new().add_systems(Update, buttons);
```

It reads the whole world, so it doesn't run alongside systems that change
abilities, effects or tags.

## Interrupted

A stun landing mid-cast should stop the cast. Say what interrupts an ability
with `end_if`, a [condition](conditions.md) weighed as `require` is: in the
ability's own tags and its owner's.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Tag)] struct State;
# #[derive(Tag)] #[tag(parent = State)] struct Stunned;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Component, Default, Behaviour)] struct CastFireball;
#[derive(Ability)]
struct Fireball;

let fireball = AbilityData::new(Key::of::<Fireball>(), Key::of::<CastFireball>())
    .end_if(Condition::owner_has("state.stunned"));

# use std::time::Duration;
# use bevy::time::TimeUpdateStrategy;
let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
#   // Each update moves time on by one fixed step.
#   .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 64.0)))
    .add_attribute::<Mana>()
    .add_behaviour::<CastFireball>()
    .add_ability::<Fireball>()
    .insert_resource(MettleContent {
        tags: ["state", "state.stunned"]
            .into_iter()
            .map(TagKey::named)
            .collect(),
        abilities: vec![fireball],
        ..default()
    });
app.update();
let mage = app.world_mut().spawn(Base::<Mana>::default()).id();
app.world_mut().grant_ability(mage, Fireball).unwrap();
let cast = app.world_mut().activate_ability(mage, Fireball).unwrap().ability();

// Stunned mid-cast: interrupted.
app.world_mut()
    .get_mut::<OwnedTags>(mage)
    .unwrap()
    .add(Stunned);
app.update();
# app.update();
assert!(app.world().get::<ActiveAbility>(cast).is_none());

// Stunned still: it can't start again.
assert_eq!(
    app.world_mut().activate_ability(mage, Fireball),
    Err(Failure::Interrupted)
);
```

It's weighed whenever its owner's tags change, once however many changed,
so an `end_if` of `!Condition::owner_has("state.raging")` ends a rage as the
raging stops. It's weighed as the ability activates too, past its owner
tags and what it ends, and as its definition is replaced. An interrupted
ability is cancelled: [`AbilityEnded`] says so, its cost stays paid and its
cooldown runs. Over a network, the server interrupts: see
[Prediction](prediction.md#endings-both-ways).

`end_if` keeps an ability from starting as well as stopping it once it
has, so an interruption needs nothing more. A `require` only keeps it from
starting: for what shouldn't stop a cast already under way.

## Told

Three entity events are triggered on the actor. Each names its ability by
number, `id`, an [`AbilityId`]:

- [`AbilityActivated`]: which ability, its entity, and the activation.
- [`AbilityFailed`]: which ability, and why, as a [`Failure`].
- [`AbilityEnded`]: which ability, its entity, and whether it was cancelled.

Observe them on the mage, or globally, for sounds, animations and UI:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
fn out_of_mana(failed: On<AbilityFailed>, registry: Res<AbilityRegistry>) {
    if failed.reason == Failure::Cost
        && let Some(name) = registry.name(failed.id)
    {
        info!("{} can't afford {name}", failed.entity);
    }
}
# App::new().add_observer(out_of_mana);
```

## Pitfalls

- **Owned tags.** An actor needs [`OwnedTags`] to activate anything. Any
  `Base<A>` brings it, so an actor with attributes has it.
- **Freshly spawned.** The cost is checked against the actor's values as
  they were last worked out. An actor spawned this tick has none yet, so its
  bases count, as its `Current`s do until then: modifiers from what it
  spawned with count only once its attributes are worked out.
- **Commands run later.** `activate_ability` as a command runs when commands
  are applied, and says nothing back. Observe [`AbilityFailed`] to hear why
  one didn't activate.
- **Ending isn't refunding.** Cancel a `Commit::Manual` ability before it
  commits if you don't want it to cost anything.

[`Ability`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/trait.Ability.html
[`AbilitySpec`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilitySpec.html
[`AbilityId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.AbilityId.html
[`Abilities`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.Abilities.html
[`AbilityOf`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.AbilityOf.html
[`AbilityActivated`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.AbilityActivated.html
[`AbilityEnded`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.AbilityEnded.html
[`AbilityFailed`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.AbilityFailed.html
[`AbilityWorld`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/trait.AbilityWorld.html
[`ActiveAbility`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.ActiveAbility.html
[`Failure`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/enum.Failure.html
[`GrantedAbility`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.GrantedAbility.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
[`GrantedBy`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.GrantedBy.html
