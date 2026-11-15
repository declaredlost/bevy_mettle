# Grants and triggers

So far the mage was granted the fireball by hand, and activated it by asking.
Abilities can also come from effects, and be activated by events and tags.

## Granted by effects

The mage's staff lets it channel. Equipping the staff applies the `Staff`
effect, a lasting one, and the channel ability says the staff grants it:
`granted_by::<Staff>`. While the staff's effect lasts, the mage has
`Channel`. When the effect goes, however it goes, the ability goes with it,
cancelled first if it's active. That's how gear gives skills.

The grant lives on the ability, as GAS's granted abilities live in an
effect's abilities component: an effect knows nothing of abilities, and an
ability knows what grants it. Several effects can grant one ability, and one
effect can grant several, each ability saying so. The effect has to last,
since an instant effect would grant nothing for any while: one that doesn't
is rejected as the registry is built.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Mana;
# #[derive(Component, Default, Behaviour)]
# struct Channelling;
#[derive(Effect)]
#[effect(infinite)]
struct Staff;

#[derive(Ability)]
#[ability(behaviour::<Channelling>, granted_by::<Staff>)]
struct Channel;

# let mut app = App::new();
# app.add_plugins(MettlePlugins)
#     .add_attribute::<Mana>()
#     .add_behaviour::<Channelling>();
app.add_effect::<Staff>().add_ability::<Channel>();
app.update();
let world = app.world_mut();
let mage = world.spawn(Base::<Mana>::default()).id();

// Equipping the staff grants channel.
world.apply_effect(mage, Staff);
world.flush();
let granted: Vec<Entity> = world.get::<Abilities>(mage).unwrap().iter().collect();
assert_eq!(granted.len(), 1);

// What the staff grants, by number.
let channel = world.resource::<AbilityRegistry>().id_of::<Channel>().unwrap();
let staff_id = world.resource::<EffectRegistry>().id_of::<Staff>().unwrap();
assert_eq!(world.resource::<AbilityRegistry>().granted_by(staff_id), [channel]);
let staff = world.get::<GrantedBy>(granted[0]).unwrap().0;

// Taking it off takes channel away.
world.despawn(staff);
world.flush();
assert!(world.get_entity(granted[0]).is_err());
```

The granted ability is [`GrantedBy`] the effect, and the effect lists what it
granted in [`GrantedAbilities`]. It's granted at the effect's level.
[`AbilityRegistry::granted_by`]
lists what an effect grants, by its number.

Grants are applied by an observer of the effect being applied, so they land
when that observer's commands do: at once in a system's commands, after
`world.flush()` on the world directly.

## Triggers

A trigger activates an ability without anyone asking. There are three:

| Trigger | Activates it |
|---|---|
| `Event(tag)` | When a gameplay event with that tag, or one under it, is sent to its owner. |
| `TagAdded(tag)` | When its owner comes to own that tag. |
| `TagPresent(tag)` | When its owner comes to own that tag, and cancels it when it no longer does. |

A triggered ability goes through the same checks as any other. A trigger
that fires while the fireball is cooling down does nothing, apart from
[`AbilityFailed`].

An ability whose behaviour takes [input](input.md) can't have triggers: an
event or a tag has no aim to give it. One that has is rejected as the
registry is built.

### Gameplay events

A [`GameplayEvent`] is a tag sent to an actor, about whom, carrying what:
an entity event, triggered on the actor with `commands.trigger(..)` or
`world.trigger(..)`, as GAS sends a gameplay event to an actor. Its
`context` carries who caused it, its source, and [context
data](applying-effects.md#context), as an effect's application does. The
mage ripostes with a fireball when it parries: its fireball has the trigger
`trigger = Trigger::Event(Parried.into())`.

```rust
# use bevy::prelude::*;
# use bevy::ecs::entity::MapEntities;
# use bevy_mettle::prelude::*;
# use serde::{Deserialize, Serialize};
# #[derive(Tag)] struct Event;
# #[derive(Tag)] #[tag(parent = Event)] struct Parried;
/// How cleanly it was parried: what the riposte's damage reads.
#[derive(Reflect, Clone, PartialEq, Serialize, Deserialize, MapEntities)]
#[reflect(ContextData)]
struct Parry {
    timing: f32,
}

fn parried(commands: &mut Commands, mage: Entity, attacker: Entity) {
    commands.trigger(
        GameplayEvent::new(mage, Parried)
            .source(attacker)
            .target(attacker)
            .with(Parry { timing: 0.8 }),
    );
}
# let _ = parried;
```

An event tagged `event.parried.perfect` triggers it too, since it's under
`event.parried`. The event reaches the behaviour in [`ActiveAbility`]'s
`event`, so the fireball knows whom to aim at, and what the event carries:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Component, Default)] struct CastFireball;
fn aim(casting: Query<&ActiveAbility, Added<CastFireball>>) {
    for active in &casting {
        let Some(event) = &active.event else {
            continue;
        };
        // Aim at `event.target`, if there is one; read the event's
        // `event.context.get::<Parry>()` for how hard to strike back.
        # let _ = event.target;
    }
}
```

The abilities it triggers activate as it's triggered, whether or not
anything else is listening. Triggered on a `World` in an exclusive system or
a test, they activate as the world's commands are applied: call
`world.flush()` to have them at once. Observe it for anything else a parry should do. An effect the riposte applies
can carry what the event did: `.with(..)` the event's data on its spec, for
its magnitudes to read.

#### Sent by an ability

A behaviour sending an event marks it with its own activation:
`GameplayEvent::new(mage, Parried).under(active.activation)`. That
matters over a network. A predicted ability it triggers then runs under an
activation derived from it, the same on the client and the server, so the
client predicts the riposte at once and the server confirms it, without
asking. Undone with its parent, if the server rejects that. An event sent by
no ability, from the game's own code, is the server's to act on: see
[On a client](#on-a-client).

### Tags

The mage lashes out with `Smolder` while it's burning: its trigger is
`Trigger::TagPresent(Burning.into())`.

```rust
# use core::time::Duration;
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Life;
# #[derive(Component, Default, Behaviour)]
# struct Smoldering;
# #[derive(Tag)] struct State;
/// What the mage owns while it burns.
#[derive(Tag)]
#[tag(parent = State)]
struct Burning;

#[derive(Effect)]
#[effect(lasting = 4.0, owner_tag = Burning)]
struct SetAlight;

#[derive(Ability)]
#[ability(behaviour::<Smoldering>, trigger = Trigger::TagPresent(Burning.into()))]
struct Smolder;

# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(50)))
#     .add_attribute::<Life>()
#     .add_behaviour::<Smoldering>()
#     .add_tag::<Burning>()
#     .add_effect::<SetAlight>()
#     .add_ability::<Smolder>();
# app.update();
let mage = app.world_mut().spawn(Base::<Life>::default()).id();
app.world_mut().grant_ability(mage, Smolder);

let smoldering = |app: &mut App| {
    app.world_mut()
        .query_filtered::<(), With<Smoldering>>()
        .iter(app.world())
        .count()
};

// Set alight: once its owned tags are told, `Smolder` activates.
app.world_mut().apply_effect(mage, SetAlight);
app.update();
app.update();
assert_eq!(smoldering(&mut app), 1);

// Put out: `Smolder` is cancelled.
let burning: Vec<Entity> = app.world().get::<ActiveEffects>(mage).unwrap().iter().collect();
for effect in burning {
    app.world_mut().despawn(effect);
}
app.update();
app.update();
assert_eq!(smoldering(&mut app), 0);
```

`TagAdded` and `TagPresent` name a tag exactly. They fire when that tag
comes to be owned, and owning a tag under it counts: a `TagAdded` of
`state` fires when the mage first owns any `state.*` tag at all. They fire when
[`TagAdded`](https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagAdded.html)
and [`TagRemoved`](https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRemoved.html)
are told, which is when attributes are next worked out, not the moment the
tag is added.

## Activating by tag

`activate_tagged` activates every ability granted to the actor that's tagged
any of the tags given. Since an ability's tags include those above them,
`Spell`, `ability.spell`, activates every spell the mage has:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Tag)] struct Ability;
# #[derive(Tag)] #[tag(parent = Ability)] struct Spell;
fn cast_everything(commands: &mut Commands, mage: Entity) {
    let spells: Tags = [Spell].into_iter().collect();
    commands.entity(mage).activate_tagged(spells);
}
# let _ = cast_everything;
```

Each goes through its checks on its own: those that can activate do. On a
`World`, [`AbilityWorld`]'s `activate_tagged(mage, &spells)` does the same at
once, and returns those that activated.

## On a client

A client leaves triggers, and abilities effects grant, to the server, with one
exception: a [predicted](prediction.md#execution-policies) ability triggered
by an event sent [under an activation](#sent-by-an-ability) is predicted
there too. Other events are still told to observers on a client, but activate
nothing, and tags owned never trigger anything there. What the server
activates and grants arrives by replication, and a
[`ServerInitiated`](prediction.md#execution-policies) ability it activates is
run on the controlling client too. See [Prediction](prediction.md).

`activate_tagged`, and `activate` on a granted ability, go through each
ability's policy on a client, as `activate_ability` does: a predicted one is
predicted and asked of the server.

[`AbilityFailed`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.AbilityFailed.html
[`AbilityWorld`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/trait.AbilityWorld.html
[`ActiveAbility`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.ActiveAbility.html
[`GameplayEvent`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.GameplayEvent.html
[`GrantedAbilities`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.GrantedAbilities.html
[`AbilityRegistry::granted_by`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilityRegistry.html#method.granted_by
[`GrantedBy`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.GrantedBy.html
