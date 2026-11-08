# Grants and triggers

So far the mage was granted the fireball by hand, and activated it by asking.
Abilities can also come from effects, and be activated by events and tags.

## Granted by effects

The mage's staff lets it channel. Equipping the staff applies the `staff`
effect, and a [`Grant`] says what that effect grants:

```ron
[(effect: "staff", abilities: ["channel"])]
```

While the staff's effect lasts, the mage has `channel`. When the effect goes,
however it goes, the ability goes with it, cancelled first if it's active.
That's how gear gives skills.

Grants are the ability crate's part of an effect, as Unreal's abilities
component is. An effect knows nothing of abilities, so grants are a list of
their own: `grants` in [`MettleContent`], next to the abilities. Your game
can still keep an item's effect and its grant in one file of its own, and
add each to its list.

Each grant is checked as the content is: its effect exists and lasts, since
an instant effect would grant nothing for any while, and its abilities
exist.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Mana;
# #[derive(Component, Default, Behaviour)]
# struct Channel;
# let mut app = App::new();
# app.add_plugins(MettlePlugins)
#     .add_attribute::<Mana>()
#     .add_behaviour::<Channel>();
app.insert_resource(MettleContent {
    effects: vec![Effect::infinite("staff")],
    abilities: vec![Ability::new("channel", "channel")],
    grants: vec![Grant::new("staff", ["channel"])],
    ..default()
});
app.update();
let world = app.world_mut();
let mage = world.spawn(Base::<Mana>::default()).id();

// Equipping the staff grants channel.
world.apply_effect(mage, EffectSpec::new("staff"));
world.flush();
let granted: Vec<Entity> = world.get::<Abilities>(mage).unwrap().iter().collect();
assert_eq!(granted.len(), 1);
let staff = world.get::<GrantedBy>(granted[0]).unwrap().0;

// Taking it off takes channel away.
world.remove_effect(staff);
world.flush();
assert!(world.get_entity(granted[0]).is_err());
```

The granted ability is [`GrantedBy`] the effect, and the effect lists what it
granted in [`GrantedAbilities`]. It's granted at the effect's level. Its
`GrantedAbility::source()` is the effect.

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
event or a tag has no aim to give it. Content with one is rejected.

### Gameplay events

A [`GameplayEvent`] is a tag sent to an actor, with who caused it, who it's
about, and how much. The mage ripostes with a fireball when it parries:

```ron
(
    id: "fireball",
    behaviour: "cast_fireball",
    triggers: [Event("event.parried")],
)
```

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
fn parried(commands: &mut Commands, mage: Entity, attacker: Entity) {
    commands.entity(mage).send_event(GameplayEvent {
        instigator: Some(attacker),
        target: Some(attacker),
        magnitude: 1.0,
        ..GameplayEvent::new(mage, "event.parried")
    });
}
# let _ = parried;
```

An event tagged `event.parried.perfect` triggers it too, since it's under
`event.parried`. The event reaches the behaviour in [`ActiveAbility`]'s
`event`, so the fireball knows whom to aim at:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Component, Default)] struct CastFireball;
fn aim(casting: Query<&ActiveAbility, Added<CastFireball>>) {
    for active in &casting {
        let target = active.event.as_ref().and_then(|event| event.target);
        // Aim at `target`, if there is one.
        # let _ = target;
    }
}
```

A [`GameplayEvent`] is an entity event too, triggered on the actor whether or
not an ability is listening. Observe it for anything else a parry should do.

### Tags

The mage lashes out with `smolder` while it's burning:

```ron
(
    id: "smolder",
    behaviour: "smolder",
    triggers: [TagPresent("state.burning")],
)
```

```rust
# use core::time::Duration;
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Life;
# #[derive(Component, Default, Behaviour)]
# struct Smolder;
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(50)))
#     .add_attribute::<Life>()
#     .add_behaviour::<Smolder>();
# let tags: Vec<Tag> = ["state", "state.burning"]
#     .into_iter()
#     .map(|id| Tag { id: Id::from_static(id) })
#     .collect();
# let tags = TagTree::build(&tags).unwrap();
# let effects = EffectRegistry::check(&[Effect::lasting("burning", 4.0).grant("state.burning")], EffectChecks { tags: &tags, curves: &CurveTable::default(), attributes: app.world().resource::<AttributeRegistry>(), code: &EffectCode::new() })
# .unwrap();
# let abilities = AbilityRegistry::check(&[Ability::new("smolder", "smolder") .trigger(Trigger::TagPresent(Id::from_static("state.burning")))], &[], AbilityChecks { tags: &tags, effects: &effects, behaviours: app.world().resource::<Behaviours>() })
# .unwrap();
# app.insert_resource(effects).insert_resource(abilities);
let mage = app.world_mut().spawn(Base::<Life>::default()).id();
app.world_mut().grant_ability(mage, "smolder", 1.0);

let smoldering = |app: &mut App| {
    app.world_mut()
        .query_filtered::<(), With<Smolder>>()
        .iter(app.world())
        .count()
};

// Set alight: once its owned tags are told, smolder activates.
app.world_mut().apply_effect(mage, EffectSpec::new("burning"));
app.update();
app.update();
assert_eq!(smoldering(&mut app), 1);

// Put out: smolder is cancelled.
let burning: Vec<Entity> = app.world().get::<ActiveEffects>(mage).unwrap().iter().collect();
for effect in burning {
    app.world_mut().remove_effect(effect);
}
app.update();
app.update();
assert_eq!(smoldering(&mut app), 0);
```

`TagAdded` and `TagPresent` name a tag exactly. They fire when that tag
comes to be owned, and owning a tag under it counts: `TagAdded("state")`
fires when the mage first owns any `state.*` tag at all. They fire when
[`TagAdded`](https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagAdded.html)
and [`TagRemoved`](https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRemoved.html)
are told, which is when attributes are next worked out, not the moment the
tag is added.

## Activating by tag

`activate_tagged` activates every ability granted to the actor that's tagged
any of the tags given. Since an ability's tags include those above them,
`ability.spell` activates every spell the mage has:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
fn cast_everything(commands: &mut Commands, mage: Entity) {
    let spells: Tags = [Id::from_static("ability.spell")].into_iter().collect();
    commands.entity(mage).activate_tagged(spells);
}
# let _ = cast_everything;
```

Each goes through its checks on its own: those that can activate do.

## On a client

A client leaves triggers, and abilities effects grant, to the server. Gameplay
events are still told to observers there, but activate nothing. What the
server activates and grants arrives by replication. See
[Prediction](prediction.md).

[`AbilityFailed`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.AbilityFailed.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`ActiveAbility`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.ActiveAbility.html
[`GameplayEvent`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.GameplayEvent.html
[`Grant`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/data/struct.Grant.html
[`GrantedAbilities`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.GrantedAbilities.html
[`GrantedBy`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.GrantedBy.html
