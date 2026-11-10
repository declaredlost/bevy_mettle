//! Gameplay tags as types: named by code without misspelling, the same tags
//! data names, and joining the tree content is checked against.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{Damage, Resistance, actor, app, close, mettle, value};

#[derive(Tag)]
struct State;

#[derive(Tag)]
#[tag(parent = State)]
struct Stunned;

#[derive(Tag)]
#[tag(parent = State, name = "dazed")]
struct Concussed;

#[derive(Tag)]
struct Element;

#[derive(Tag)]
#[tag(parent = Element)]
struct Fire;

#[test]
fn a_tag_type_is_the_tag_its_name_says() {
    assert_eq!(TagKey::from(Stunned), TagKey::named("state.stunned"));
    assert_eq!(TagKey::from(Concussed).name(), "state.dazed");
    assert_eq!(TagKey::from(State).name(), "state");
    assert!(TagKey::from(Stunned).is_within(State));
    assert_eq!(TagKey::from(Stunned).parent(), Some(TagKey::from(State)));
    assert_eq!(TagKey::from(State).parent(), None);
}

#[test]
fn owned_tags_reads_and_conditions_take_tag_types() {
    let mut app = app();
    let mage = actor(
        &mut app,
        [ModifierSpec::flat::<Damage>(5.0).owner_has(Stunned)],
    );
    let mut owned = app.world_mut().get_mut::<OwnedTags>(mage).unwrap();
    owned.add(Stunned);
    assert!(owned.contains(Stunned));
    assert!(owned.contains(State), "what's above it too");
    assert!(owned.contains("state.stunned"), "the same as its name");
    app.tick();
    let read = AttributeContext::new();
    assert!(close(value::<Damage>(&app, mage, &read), 15.0));

    // A read tagged by type.
    app.world_mut()
        .entity_mut(mage)
        .modifiers([ModifierSpec::flat::<Resistance>(0.5).read_tagged(Element)]);
    app.tick();
    let fire = AttributeContext::new().tag(Fire);
    assert!(close(value::<Resistance>(&app, mage, &fire), 0.5));
    assert!(close(value::<Resistance>(&app, mage, &read), 0.0));
}

#[test]
fn tags_code_adds_join_the_tree_content_is_checked_against() {
    let app = TestApp::builder()
        .plugins(mettle())
        .with(|app| {
            app.add_tag::<Stunned>().add_tag::<Fire>();
        })
        .build();
    let content = MettleContent {
        // The content defines `state` itself; code adds `state.stunned`, and
        // `element` with `element.fire`.
        tags: vec![TagKey::named("state")],
        effects: vec![
            EffectData::lasting("stun", 1.0).owner_tag(Stunned),
            EffectData::lasting("burn", 1.0).owner_tag("element.fire"),
        ],
        ..default()
    };
    content.check(app.world()).unwrap();

    let misspelled = MettleContent {
        effects: vec![EffectData::lasting("stun", 1.0).owner_tag("state.stunted")],
        ..default()
    };
    let errors = misspelled.check(app.world()).unwrap_err().to_string();
    assert!(
        errors.contains("there's no tag called `state.stunted`"),
        "{errors}"
    );
}

#[derive(Tag)]
#[tag(parent = Tock)]
struct Tick;

#[derive(Tag)]
#[tag(parent = Tick)]
struct Tock;

#[test]
#[should_panic(expected = "is its own ancestor")]
fn a_tag_under_itself_says_so() {
    let _ = TagKey::from(Tick);
}
