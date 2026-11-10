//! Cues: lasting ones held while effects count and abilities are active, as
//! entities observed as they're added and removed; one-shots triggered as
//! effects execute; and what a server keeps for its clients.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::core::registry::{AttributeId, TagId};
use bevy_mettle::effect::context::ContextReplica;
use bevy_mettle::effect::cue::{Burst, CueBursts, ServedCues};
use bevy_mettle::prelude::*;

use crate::common::{Life, Mana, attributes, close, id, mettle};

#[derive(Effect)]
struct Burning;

#[derive(Effect)]
struct Smoulder;

#[derive(Effect)]
#[effect(name = "fireball.hit")]
struct FireballHit;

#[derive(Effect)]
struct Drain;

#[derive(Ability)]
struct Fireball;

#[derive(Component, Default, Behaviour)]
struct Cast;

#[derive(Component, Default, Behaviour)]
struct Channel;

/// What the observers saw: each lasting cue added or removed, and each
/// one-shot under `cue.hit` they played, the most specific first.
#[derive(Resource, Default)]
struct Seen(Vec<String>);

/// A lasting cue added: its tag, actor, whether it was joined and whether
/// it's unreplicated.
fn on_added(added: On<Add<ActiveCue>>, cues: Query<(&ActiveCue, &CueOf)>, mut seen: ResMut<Seen>) {
    let (cue, of) = cues.get(added.entity).unwrap();
    seen.0.push(format!(
        "added {} on {} joined {} unreplicated {}",
        cue.tag, of.0, cue.joined, cue.unreplicated
    ));
}

/// A lasting cue removed, read as it goes.
fn on_removed(
    removed: On<Remove<ActiveCue>>,
    cues: Query<(&ActiveCue, &CueOf)>,
    mut seen: ResMut<Seen>,
) {
    let (cue, of) = cues.get(removed.entity).unwrap();
    seen.0.push(format!("removed {} on {}", cue.tag, of.0));
}

/// A hit's one-shot: a fire hit's own, else any hit's.
fn on_hit(played: On<CueExecuted>, mut seen: ResMut<Seen>) {
    if played.tag.is_within("cue.hit.fire") {
        seen.0.push(format!("fire hit {}", played.tag));
    } else if played.tag.is_within("cue.hit") {
        seen.0.push(format!("hit {}", played.tag));
    }
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
        tags: TAGS.iter().map(|&name| TagKey::named(name)).collect(),
        effects: vec![
            EffectData::lasting(Key::of::<Burning>(), 4.0)
                .cue("cue.burning")
                .ongoing(!Condition::owner_has("state.wet")),
            EffectData::lasting(Key::of::<Smoulder>(), 4.0)
                .period(1.0, false)
                .modifier(EffectModifier::new::<Life>(Operation::Flat, -2.0))
                .cue("cue.burning")
                .cue("cue.hit.fire"),
            EffectData::instant(Key::of::<FireballHit>())
                .modifier(EffectModifier::new::<Life>(Operation::Flat, -15.0))
                .cue("cue.hit.fire"),
            EffectData::instant(Key::of::<Drain>())
                .modifier(EffectModifier::new::<Life>(Operation::Flat, -20.0))
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, 10.0))
                .modifier(EffectModifier::new::<Life>(Operation::Flat, -10.0))
                .cue("cue.hit"),
        ],
        abilities: vec![
            AbilityData::new(Key::of::<Fireball>(), Key::of::<Cast>()).cue("cue.casting"),
        ],
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
                .init_resource::<Seen>()
                .add_observer(on_added)
                .add_observer(on_removed)
                .add_observer(on_hit)
                .add_effect::<Burning>()
                .add_effect::<Smoulder>()
                .add_effect::<FireballHit>()
                .add_effect::<Drain>()
                .add_ability::<Fireball>()
                .insert_resource(content());
        })
        .watch_triggered::<CueExecuted>()
        .build();
    let actor = app.spawn((Base::<Life>::default(), Base::<Mana>::default()));
    app.tick();
    (app, actor)
}

/// What the observers saw, taken.
fn seen(app: &mut TestApp) -> Vec<String> {
    std::mem::take(&mut app.resource_mut::<Seen>().0)
}

/// `tag`'s number in `app`'s tree.
fn tag_id(app: &TestApp, tag: &'static str) -> TagId {
    app.world().resource::<TagRegistry>().id(tag).unwrap()
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
    app.world_mut().apply_effect(mage, Burning);
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.burning"]);
    assert_eq!(
        seen(&mut app),
        [format!(
            "added cue.burning on {mage} joined false unreplicated false"
        )]
    );

    // Wet: the burning doesn't count, so its cue ends.
    app.world_mut()
        .get_mut::<OwnedTags>(mage)
        .unwrap()
        .add("state.wet");
    app.tick();
    app.tick();
    assert_eq!(shown(&app, mage), Vec::<String>::new());
    assert_eq!(seen(&mut app), [format!("removed cue.burning on {mage}")]);
}

#[test]
fn a_cue_held_twice_ends_once_both_let_go() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut().apply_effect(mage, Burning);
    app.world_mut().add_cue(mage, "cue.burning");
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.burning"], "one entity, held twice");
    let effect = app
        .entity(mage)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    app.world_mut().despawn(effect);
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.burning"], "still held");
    app.world_mut().remove_cue(mage, "cue.burning");
    app.tick();
    assert_eq!(shown(&app, mage), Vec::<String>::new());
}

#[test]
fn removing_a_cue_removes_whichever_source_holds_it() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut().add_unreplicated_cue(mage, "cue.casting");
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.casting"]);
    app.world_mut().remove_cue(mage, "cue.casting");
    app.tick();
    assert_eq!(
        shown(&app, mage),
        Vec::<String>::new(),
        "the unreplicated one"
    );
    // None holds it: warned of, and nothing changes.
    app.world_mut().remove_cue(mage, "cue.casting");
    app.tick();
    assert_eq!(shown(&app, mage), Vec::<String>::new());
}

#[test]
fn a_cue_is_unreplicated_while_only_unreplicated_sources_hold_it() {
    let (mut app, mage) = app(NetRole::Server);
    app.world_mut().add_unreplicated_cue(mage, "cue.casting");
    app.tick();
    let cue = |app: &TestApp| {
        let entity = app.entity(mage).get::<ActiveCues>().iter().next().unwrap();
        app.entity(entity).get::<ActiveCue>().clone()
    };
    assert!(cue(&app).unreplicated);
    app.world_mut().add_cue(mage, "cue.casting");
    app.tick();
    assert!(!cue(&app).unreplicated, "served now too");
}

#[test]
fn an_abilitys_cue_is_held_while_its_active() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut().grant_ability(mage, Fireball).unwrap();
    let fireball = app
        .world_mut()
        .activate_ability(mage, Fireball)
        .unwrap()
        .ability();
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.casting"]);
    app.world_mut().end_ability(fireball);
    app.tick();
    assert_eq!(shown(&app, mage), Vec::<String>::new());
}

#[test]
fn an_effect_executing_plays_its_cue_with_what_it_dealt() {
    let (mut app, mage) = app(NetRole::Standalone);
    let hit = app
        .world()
        .resource::<EffectRegistry>()
        .id_of::<FireballHit>();
    let caster = app.spawn(Base::<Mana>::default());
    app.world_mut()
        .apply_effect(mage, FireballHit.level(3.0).source(caster));
    app.assert_triggered::<CueExecuted>(|executed| {
        executed.tag.name() == "cue.hit.fire"
            && close(executed.magnitude(), -15.0)
            && close(executed.level, 3.0)
            && executed.context.source == Some(caster)
            && hit.is_some()
            && executed.effect == hit
    });
}

#[test]
fn a_periodic_effect_holds_its_cue_and_plays_it_each_period() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut().apply_effect(mage, Smoulder);
    app.tick();
    app.read_triggered::<CueExecuted>();
    assert_eq!(shown(&app, mage), ["cue.burning", "cue.hit.fire"]);
    app.secs(2.0);
    let played = app.read_triggered::<CueExecuted>();
    for tag in ["cue.burning", "cue.hit.fire"] {
        let each = played
            .iter()
            .filter(|executed| executed.tag.name() == tag)
            .count();
        assert_eq!(
            each, 2,
            "{tag}: each cue, each period, twice in two seconds"
        );
    }
    assert!(
        played
            .iter()
            .all(|executed| close(executed.magnitude(), -2.0))
    );
}

#[test]
fn an_observer_matches_a_tag_and_its_children_the_most_specific_first() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut().execute_cue(mage, &Cue::new("cue.hit.fire"));
    app.world_mut().execute_cue(mage, &Cue::new("cue.hit.cold"));
    assert_eq!(
        seen(&mut app),
        ["fire hit cue.hit.fire", "hit cue.hit.cold"],
        "fire's own, then the hit one for cold"
    );
}

#[test]
fn a_server_serves_its_cues_but_those_unreplicated() {
    let (mut app, mage) = app(NetRole::Server);
    app.world_mut().apply_effect(mage, Burning);
    app.world_mut().add_unreplicated_cue(mage, "cue.casting");
    app.world_mut()
        .execute_cue(mage, &Cue::new("cue.hit.fire").level(9.0));
    app.world_mut()
        .execute_cue(mage, &Cue::new("cue.hit.cold").unreplicated());
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.burning", "cue.casting"]);
    let tree = app.world().resource::<TagRegistry>();
    let served: Vec<TagKey> = app
        .entity(mage)
        .get::<ServedCues>()
        .0
        .iter()
        .map(|&tag| tree.key(tag).unwrap())
        .collect();
    assert_eq!(
        served,
        [TagKey::named("cue.burning")],
        "the unreplicated one kept back"
    );
    let bursts = app.entity(mage).get::<CueBursts>();
    assert_eq!(bursts.recent.len(), 1, "the unreplicated one kept back");
    assert_eq!(bursts.recent[0].tag, tag_id(&app, "cue.hit.fire"));
    assert!(close(bursts.recent[0].level, 9.0));
}

#[test]
fn a_client_shows_the_servers_cues_and_plays_its_one_shots_once() {
    let (mut server, there) = app(NetRole::Server);
    let (mut client, _) = app(NetRole::Client);
    server.world_mut().apply_effect(there, Burning);
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
    assert!(seen(&mut client).contains(&format!(
        "added cue.burning on {here} joined true unreplicated false"
    )));
    let played = client.read_triggered::<CueExecuted>();
    assert_eq!(played.len(), 1);
    assert_eq!(played[0].tag.name(), "cue.land");

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
    assert_eq!(played[0].tag.name(), "cue.hit.cold");

    // The burning ends on the server.
    let effect = server
        .entity(there)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    server.world_mut().despawn(effect);
    server.tick();
    client.entity_mut(here).insert(state(&server));
    client.tick();
    assert_eq!(shown(&client, here), Vec::<String>::new());
    assert!(seen(&mut client).contains(&format!("removed cue.burning on {here}")));
}

#[test]
fn a_cue_starting_on_an_actor_already_seen_isnt_joined() {
    let (mut client, here) = app(NetRole::Client);
    let burning = tag_id(&client, "cue.burning");
    client.entity_mut(here).insert(ServedCues([burning].into()));
    client.tick();
    assert_eq!(
        seen(&mut client),
        [format!(
            "added cue.burning on {here} joined false unreplicated false"
        )]
    );
}

#[test]
fn a_cue_that_isnt_a_tag_is_rejected() {
    let (app, _) = app(NetRole::Standalone);
    let mut content = content();
    content
        .effects
        .push(EffectData::lasting("frozen", 1.0).cue("cue.frozen"));
    let errors = content.check(app.world()).unwrap_err().to_string();
    assert!(errors.contains("cue.frozen"), "{errors}");
}

#[test]
fn an_effect_despawned_lets_go_of_its_cues() {
    let (mut app, mage) = app(NetRole::Standalone);
    app.world_mut().apply_effect(mage, Burning);
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
    server.world_mut().apply_effect(there, Burning);
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
    app.world_mut().grant_ability(mage, Fireball).unwrap();
    let fireball = app
        .world_mut()
        .activate_ability(mage, Fireball)
        .unwrap()
        .ability();
    app.tick();
    assert_eq!(shown(&app, mage), ["cue.casting"]);

    // A reload mid-cast: the fireball's run by another behaviour now, and
    // plays another cue.
    app.resource_mut::<MettleContent>().abilities =
        vec![AbilityData::new(Key::of::<Fireball>(), Key::of::<Channel>()).cue("cue.burning")];
    app.tick();
    app.world_mut().end_ability(fireball);
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
fn bursts(tag: TagId, activation: Activation, n: u64) -> CueBursts {
    CueBursts {
        next: n,
        recent: (0..n)
            .map(|seq| Burst {
                seq,
                at: 0.0,
                tag,
                changes: Vec::new(),
                level: 1.0,
                context: ContextReplica::default(),
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
            .apply_effect(here, FireballHit.spec().activation(Activation::client(7)));
    }
    assert_eq!(client.read_triggered::<CueExecuted>().len(), 2, "predicted");
    let fire = tag_id(&client, "cue.hit.fire");
    client
        .entity_mut(here)
        .insert(bursts(fire, Activation::client(7), 2));
    client.tick();
    client.assert_triggered_none::<CueExecuted>(|_| true);
}

#[test]
fn a_rejected_predictions_cues_are_forgotten() {
    let (mut client, here) = app(NetRole::Client);
    client
        .world_mut()
        .apply_effect(here, FireballHit.spec().activation(Activation::client(7)));
    client.world_mut().reject_prediction(Activation::client(7));
    client.read_triggered::<CueExecuted>();
    // Were the server to play it after all, it'd be its own.
    let fire = tag_id(&client, "cue.hit.fire");
    client
        .entity_mut(here)
        .insert(bursts(fire, Activation::client(7), 1));
    client.tick();
    assert_eq!(client.read_triggered::<CueExecuted>().len(), 1);
}

#[test]
fn a_predicted_cue_the_server_never_echoes_is_forgotten_in_time() {
    let (mut client, here) = app(NetRole::Client);
    client
        .world_mut()
        .apply_effect(here, FireballHit.spec().activation(Activation::client(7)));
    client.read_triggered::<CueExecuted>();
    client.secs(2.0);
    let fire = tag_id(&client, "cue.hit.fire");
    client
        .entity_mut(here)
        .insert(bursts(fire, Activation::client(7), 1));
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
    app.world_mut().apply_effect(mage, Drain);
    let played = app.read_triggered::<CueExecuted>();
    assert_eq!(played.len(), 1);
    let drain = &played[0];
    assert!(
        close(drain.magnitude(), -30.0),
        "life's, both of its changes: {drain:?}"
    );
    let changes: Vec<(AttributeId, f32)> = drain
        .changes
        .iter()
        .map(|change| (change.attribute, change.delta))
        .collect();
    assert_eq!(changes, [(id::<Life>(), -30.0), (id::<Mana>(), 10.0)]);
}
