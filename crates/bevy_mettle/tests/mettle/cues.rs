//! Cues: lasting ones held while effects count and abilities are active, as
//! entities; one-shots as effects execute; handlers run for the most specific
//! tag; and what a server keeps for its clients.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::effect::cue::{Burst, CueBursts, ServedCues};
use bevy_mettle::prelude::*;

use crate::common::{Life, Mana, attributes, close, mettle};

#[derive(Component, Default, Behaviour)]
struct Cast;

#[derive(Component, Default, Behaviour)]
struct Channel;

/// Which handler ran, for which cue, and how.
#[derive(Resource, Default)]
struct Ran(Vec<String>);

fn on_hit(In(event): In<CueEvent>, mut ran: ResMut<Ran>) {
    ran.0.push(format!("hit {}", event.tag()));
}

fn on_fire_hit(In(event): In<CueEvent>, mut ran: ResMut<Ran>) {
    ran.0.push(format!("fire hit {}", event.tag()));
}

const TAGS: &[&str] = &[
    "cue",
    "cue.land",
    "cue.burning",
    "cue.casting",
    "cue.hit",
    "cue.hit.fire",
    "cue.hit.cold",
    "state",
    "state.wet",
];

fn content() -> MettleContent {
    MettleContent {
        tags: TAGS.iter().map(|&id| Tag { id: Id::new(id) }).collect(),
        effects: vec![
            Effect::lasting("burning", 4.0)
                .cue("cue.burning")
                .ongoing(!Condition::owner_has("state.wet")),
            Effect::lasting("smoulder", 4.0)
                .period(1.0, false)
                .modifier(EffectModifier::new::<Life>(Operation::Flat, -2.0))
                .cue("cue.burning")
                .cue("cue.hit.fire"),
            Effect::instant("fireball.hit")
                .modifier(EffectModifier::new::<Life>(Operation::Flat, -15.0))
                .cue("cue.hit.fire"),
            Effect::instant("drain")
                .modifier(EffectModifier::new::<Life>(Operation::Flat, -20.0))
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, 10.0))
                .modifier(EffectModifier::new::<Life>(Operation::Flat, -10.0))
                .cue("cue.hit"),
        ],
        abilities: vec![Ability::new("fireball", "cast").cue("cue.casting")],
        ..default()
    }
}

/// An app playing as `role`, with the content and the handlers, and an actor
/// in it.
fn app(role: NetRole) -> (TestApp, Entity) {
    let mut app = TestApp::builder()
        .plugins(mettle())
        .with(attributes)
        .with(move |app| {
            app.insert_resource(role)
                .add_behaviour::<Cast>()
                .add_behaviour::<Channel>()
                .init_resource::<Ran>()
                .add_cue_handler("cue.hit", on_hit)
                .add_cue_handler("cue.hit.fire", on_fire_hit)
                .insert_resource(content());
        })
        .watch_triggered::<CueAdded>()
        .watch_triggered::<CueRemoved>()
        .watch_triggered::<CueExecuted>()
        .build();
    let actor = app.spawn((Base::<Life>::default(), Base::<Mana>::default()));
    app.tick();
    (app, actor)
}

/// The tags of the lasting cues shown on `actor`.
fn shown(app: &TestApp, actor: Entity) -> Vec<String> {
    let mut tags: Vec<String> = app
        .entity(actor)
        .try_get::<ActiveCues>()
        .map(|cues| cues.iter().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|cue| app.entity(cue).get::<ActiveCue>().tag.to_string())
        .collect();
    tags.sort();
    tags
}

#[test]
fn a_lasting_effects_cue_is_held_while_it_counts() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut()
        .apply_effect(mage, EffectSpec::new("burning"));
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.burning"]);
    app.assert_triggered::<CueAdded>(|added| added.tag.as_str() == "cue.burning" && !added.joined);

    // Wet: the burning doesn't count, so its cue ends.
    app.world_mut()
        .get_mut::<OwnedTags>(mage)
        .unwrap()
        .add(&Id::new("state.wet"));
    app.tick();
    app.tick();
    assert_eq!(shown(&app, mage), Vec::<String>::new());
    app.assert_triggered::<CueRemoved>(|removed| removed.tag.as_str() == "cue.burning");
}

#[test]
fn a_cue_held_twice_ends_once_both_let_go() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut()
        .apply_effect(mage, EffectSpec::new("burning"));
    app.world_mut().add_cue(mage, &Cue::new("cue.burning"));
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.burning"], "one entity, held twice");
    let effect = app
        .entity(mage)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    app.world_mut().remove_effect(effect);
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.burning"], "still held");
    app.world_mut().remove_cue(mage, &Cue::new("cue.burning"));
    app.tick();
    assert_eq!(shown(&app, mage), Vec::<String>::new());
}

#[test]
fn an_abilitys_cue_is_held_while_its_active() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut()
        .grant_ability(mage, "fireball", 1.0)
        .unwrap();
    let fireball = app.world_mut().activate_ability(mage, "fireball").unwrap();
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.casting"]);
    app.world_mut().end_ability(fireball, false);
    app.tick();
    assert_eq!(shown(&app, mage), Vec::<String>::new());
}

#[test]
fn an_effect_executing_plays_its_cue_with_what_it_dealt() {
    let (mut app, mage) = app(NetRole::Standalone);
    let caster = app.spawn(Base::<Mana>::default());
    app.world_mut().apply_effect(
        mage,
        EffectSpec::new("fireball.hit").level(3.0).source(caster),
    );
    app.assert_triggered::<CueExecuted>(|executed| {
        executed.tag.as_str() == "cue.hit.fire"
            && close(executed.magnitude, -15.0)
            && close(executed.level, 3.0)
            && executed.source == Some(caster)
            && executed
                .effect
                .as_ref()
                .is_some_and(|effect| effect.as_str() == "fireball.hit")
    });
}

#[test]
fn a_periodic_effect_holds_its_cue_and_plays_it_each_period() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut()
        .apply_effect(mage, EffectSpec::new("smoulder"));
    app.tick();
    app.read_triggered::<CueExecuted>();
    assert_eq!(shown(&app, mage), ["cue.burning", "cue.hit.fire"]);
    app.secs(2.0);
    let played = app.read_triggered::<CueExecuted>();
    for tag in ["cue.burning", "cue.hit.fire"] {
        let each = played
            .iter()
            .filter(|executed| executed.tag.as_str() == tag)
            .count();
        assert_eq!(
            each, 2,
            "{tag}: each cue, each period, twice in two seconds"
        );
    }
    assert!(
        played
            .iter()
            .all(|executed| close(executed.magnitude, -2.0))
    );
}

#[test]
fn the_most_specific_handler_runs_alone() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut().execute_cue(mage, &Cue::new("cue.hit.fire"));
    app.world_mut().execute_cue(mage, &Cue::new("cue.hit.cold"));
    assert_eq!(
        app.resource::<Ran>().0,
        ["fire hit cue.hit.fire", "hit cue.hit.cold"],
        "fire's own, then the hit one for cold"
    );
}

#[test]
fn a_server_serves_its_cues_but_those_it_keeps_local() {
    let (mut app, mage) = app(NetRole::Server);
    app.world_mut()
        .apply_effect(mage, EffectSpec::new("burning"));
    app.world_mut()
        .add_cue(mage, &Cue::new("cue.casting").local());
    app.world_mut()
        .execute_cue(mage, &Cue::new("cue.hit.fire").magnitude(9.0));
    app.world_mut()
        .execute_cue(mage, &Cue::new("cue.hit.cold").local());
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.burning", "cue.casting"]);
    let served: Vec<&str> = app
        .entity(mage)
        .get::<ServedCues>()
        .0
        .iter()
        .map(Id::as_str)
        .collect();
    assert_eq!(served, ["cue.burning"], "the local one kept back");
    let bursts = app.entity(mage).get::<CueBursts>();
    assert_eq!(bursts.recent.len(), 1, "the local one kept back");
    assert_eq!(bursts.recent[0].tag.as_str(), "cue.hit.fire");
    assert!(close(bursts.recent[0].magnitude, 9.0));
}

#[test]
fn a_client_shows_the_servers_cues_and_plays_its_one_shots_once() {
    let (mut server, there) = app(NetRole::Server);
    let (mut client, _) = app(NetRole::Client);
    server
        .world_mut()
        .apply_effect(there, EffectSpec::new("burning"));
    server
        .world_mut()
        .execute_cue(there, &Cue::new("cue.hit.fire"));
    // Long enough ago that the server's let go of it.
    server.secs(1.0);
    server.world_mut().execute_cue(there, &Cue::new("cue.land"));
    server.tick();
    let state = |server: &TestApp| {
        (
            server.entity(there).get::<ServedCues>().clone(),
            server.entity(there).get::<CueBursts>().clone(),
        )
    };

    // The client comes to see the mage, as replication spawns it: it's
    // burning already; what's just happened plays, and the hit from a second
    // ago is past.
    let here = client.spawn((Base::<Life>::default(), state(&server)));
    client.tick();
    assert_eq!(shown(&client, here), ["cue.burning"]);
    client.assert_triggered::<CueAdded>(|added| added.joined);
    let played = client.read_triggered::<CueExecuted>();
    assert_eq!(played.len(), 1);
    assert_eq!(played[0].tag.as_str(), "cue.land");

    // A hit, once, however often it's restated.
    server
        .world_mut()
        .execute_cue(there, &Cue::new("cue.hit.cold"));
    server.tick();
    for _ in 0..2 {
        client.entity_mut(here).insert(state(&server));
        client.tick();
    }
    let played = client.read_triggered::<CueExecuted>();
    assert_eq!(played.len(), 1);
    assert_eq!(played[0].tag.as_str(), "cue.hit.cold");

    // The burning ends on the server.
    let effect = server
        .entity(there)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    server.world_mut().remove_effect(effect);
    server.tick();
    client.entity_mut(here).insert(state(&server));
    client.tick();
    assert_eq!(shown(&client, here), Vec::<String>::new());
    client.assert_triggered::<CueRemoved>(|removed| removed.tag.as_str() == "cue.burning");
}

#[test]
fn a_cue_starting_on_an_actor_already_seen_isnt_joined() {
    let (mut client, here) = app(NetRole::Client);
    client
        .entity_mut(here)
        .insert(ServedCues([Id::new("cue.burning")].into()));
    client.tick();
    client.assert_triggered::<CueAdded>(|added| !added.joined);
}

#[test]
fn a_cue_that_isnt_a_tag_is_rejected() {
    let (app, _) = app(NetRole::Standalone);
    let mut content = content();
    content
        .effects
        .push(Effect::lasting("frozen", 1.0).cue("cue.frozen"));
    let errors = content.check(app.world()).unwrap_err().to_string();
    assert!(errors.contains("cue.frozen"), "{errors}");
}

#[test]
fn an_effect_despawned_lets_go_of_its_cues() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut()
        .apply_effect(mage, EffectSpec::new("burning"));
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.burning"]);
    let effect = app
        .entity(mage)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    app.entity_mut(effect).despawn();
    app.tick();
    assert_eq!(shown(&app, mage), Vec::<String>::new());
}

#[test]
fn a_shown_effect_replicated_away_lets_go_of_its_cues() {
    let (mut server, there) = app(NetRole::Server);
    let (mut client, here) = app(NetRole::Client);
    server
        .world_mut()
        .apply_effect(there, EffectSpec::new("burning"));
    server.tick();
    let effect = server
        .entity(there)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    let replica = server.entity(effect).get::<EffectReplica>().clone();
    let shown_effect = client.spawn((replica, EffectOf(here)));
    client.tick();
    assert_eq!(shown(&client, here), ["cue.burning"]);
    // The server's ends: replication despawns what the client showed.
    client.entity_mut(shown_effect).despawn();
    client.tick();
    assert_eq!(shown(&client, here), Vec::<String>::new());
}

#[test]
fn an_ability_reloaded_mid_cast_ends_what_it_started_with() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut()
        .grant_ability(mage, "fireball", 1.0)
        .unwrap();
    let fireball = app.world_mut().activate_ability(mage, "fireball").unwrap();
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.casting"]);

    // A reload mid-cast: the fireball's run by another behaviour now, and
    // plays another cue.
    app.resource_mut::<MettleContent>().abilities =
        vec![Ability::new("fireball", "channel").cue("cue.burning")];
    app.tick();
    app.world_mut().end_ability(fireball, false);
    app.tick();
    assert_eq!(
        shown(&app, mage),
        Vec::<String>::new(),
        "its own cue let go"
    );
    app.entity(fireball)
        .assert_lacks::<Cast>()
        .assert_lacks::<Channel>();
}

/// The server's one-shots of `tag` under `activation`, `n` of them, as
/// replicated.
fn bursts(tag: &str, activation: u64, n: u64) -> CueBursts {
    CueBursts {
        next: n,
        recent: (0..n)
            .map(|seq| Burst {
                seq,
                at: 0.0,
                tag: Id::new(tag),
                magnitude: -15.0,
                changes: Vec::new(),
                level: 1.0,
                source: None,
                activation: Some(activation),
                effect: None,
            })
            .collect(),
    }
}

#[test]
fn a_cue_predicted_twice_under_one_activation_skips_both_server_copies() {
    let (mut client, here) = app(NetRole::Client);
    for _ in 0..2 {
        client
            .world_mut()
            .apply_effect(here, EffectSpec::new("fireball.hit").activation(7));
    }
    assert_eq!(client.read_triggered::<CueExecuted>().len(), 2, "predicted");
    client.entity_mut(here).insert(bursts("cue.hit.fire", 7, 2));
    client.tick();
    client.assert_triggered_none::<CueExecuted>(|_| true);
}

#[test]
fn a_rejected_predictions_cues_are_forgotten() {
    let (mut client, here) = app(NetRole::Client);
    client
        .world_mut()
        .apply_effect(here, EffectSpec::new("fireball.hit").activation(7));
    client.world_mut().reject_prediction(7);
    client.read_triggered::<CueExecuted>();
    // Were the server to play it after all, it'd be its own.
    client.entity_mut(here).insert(bursts("cue.hit.fire", 7, 1));
    client.tick();
    assert_eq!(client.read_triggered::<CueExecuted>().len(), 1);
}

#[test]
fn a_predicted_cue_the_server_never_echoes_is_forgotten_in_time() {
    let (mut client, here) = app(NetRole::Client);
    client
        .world_mut()
        .apply_effect(here, EffectSpec::new("fireball.hit").activation(7));
    client.read_triggered::<CueExecuted>();
    client.secs(2.0);
    client.entity_mut(here).insert(bursts("cue.hit.fire", 7, 1));
    client.tick();
    assert_eq!(
        client.read_triggered::<CueExecuted>().len(),
        1,
        "long forgotten"
    );
}

#[test]
fn a_one_shots_magnitude_is_its_first_attributes_and_it_carries_every_change() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut().apply_effect(mage, EffectSpec::new("drain"));
    let played = app.read_triggered::<CueExecuted>();
    assert_eq!(played.len(), 1);
    let drain = &played[0];
    assert!(
        close(drain.magnitude, -30.0),
        "life's, both of its changes: {drain:?}"
    );
    let changes: Vec<(&str, f32)> = drain
        .changes
        .iter()
        .map(|change| (change.attribute.as_str(), change.delta))
        .collect();
    assert_eq!(changes, [("life", -30.0), ("mana", 10.0)]);
}
