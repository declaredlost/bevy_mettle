//! When modifiers count: the read's tags, owned tags, the target's, and
//! facts, weighed.

use bevy_mettle::prelude::*;

use crate::common::{AttackSpeed, Damage, FLOWN, actor, app, attributes, close, get, own, value};

fn ramp() -> Condition {
    Condition::fact_ramp(FLOWN, 2.0, 8.0)
}

fn flown(distance: f32) -> AttributeContext<'static> {
    AttributeContext::new().fact(FLOWN, distance)
}

#[test]
fn a_ramp_counts_a_modifier_in_proportion() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::additive::<Damage>(1.0).condition(ramp()),
            Modifier::multiplicative::<AttackSpeed>(1.0).condition(ramp()),
        ],
    );
    assert!(close(value::<Damage>(&app, npc, &flown(1.0)), 20.0));
    assert!(close(value::<Damage>(&app, npc, &flown(5.0)), 15.0));
    assert!(close(value::<AttackSpeed>(&app, npc, &flown(5.0)), 1.5));
    assert!(close(get::<Damage>(&app, npc), 10.0), "no hit, no bonus");
}

#[test]
fn a_base_or_override_counts_only_in_full() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::base::<Damage>(40.0).condition(ramp()),
            Modifier::overriding::<AttackSpeed>(3.0).condition(ramp()),
        ],
    );
    assert!(close(value::<Damage>(&app, npc, &flown(5.0)), 10.0));
    assert!(close(value::<AttackSpeed>(&app, npc, &flown(5.0)), 1.0));
    assert!(close(value::<Damage>(&app, npc, &flown(1.0)), 40.0));
    assert!(close(value::<AttackSpeed>(&app, npc, &flown(1.0)), 3.0));
}

#[test]
fn an_owned_tag_counts_while_held_without_folding_again() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [Modifier::multiplicative::<AttackSpeed>(1.0).owner_has("state")],
    );
    assert!(close(get::<AttackSpeed>(&app, npc), 1.0));
    own(&mut app, npc, "state.moving", true);
    own(&mut app, npc, "state.moving", true);
    assert!(close(get::<AttackSpeed>(&app, npc), 2.0), "a tag under it");
    own(&mut app, npc, "state.moving", false);
    assert!(close(get::<AttackSpeed>(&app, npc), 2.0), "one source left");
    own(&mut app, npc, "state.moving", false);
    assert!(close(get::<AttackSpeed>(&app, npc), 1.0), "none left");
}

#[test]
fn the_reads_tags_count_for_a_tag_or_one_under_it() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [Modifier::additive::<Damage>(0.5).read_tagged("damage.elemental")],
    );
    let fire = AttributeContext::new().tag("damage.elemental.fire");
    assert!(close(value::<Damage>(&app, npc, &fire), 15.0));
    let physical = AttributeContext::new().tag("damage.physical");
    assert!(close(value::<Damage>(&app, npc, &physical), 10.0));
}

#[test]
fn the_targets_and_sources_tags_are_apart_from_the_reads() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::flat::<Damage>(5.0).condition(Condition::target_has("creature.undead")),
            Modifier::flat::<Damage>(1.0).condition(Condition::source_has("state.enraged")),
        ],
    );
    let mut undead = OwnedTags::default();
    undead.add(&Id::from_static("creature.undead.ghoul"));
    let mut enraged = OwnedTags::default();
    enraged.add(&Id::from_static("state.enraged"));
    assert!(close(
        value::<Damage>(&app, npc, &AttributeContext::new().target_tags(&undead)),
        15.0
    ));
    assert!(close(
        value::<Damage>(&app, npc, &AttributeContext::new().source_tags(&enraged)),
        11.0
    ));
    assert!(close(
        value::<Damage>(&app, npc, &AttributeContext::new().tag("creature.undead")),
        10.0
    ));
}

#[test]
fn not_holds_where_its_term_does_not() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [Modifier::flat::<Damage>(5.0).condition(!Condition::target_has("creature.undead"))],
    );
    let undead = AttributeContext::new().target_tag("creature.undead");
    assert!(close(value::<Damage>(&app, npc, &undead), 10.0));
    assert!(close(get::<Damage>(&app, npc), 15.0));
}

#[test]
fn a_modifier_reads_from_data_as_written() {
    let mut app = app();
    let modifier: Modifier = ron::from_str(
        r#"(attribute: "damage", op: Additive, value: 1.0, condition: All([
            ReadTagged("delivery.attack"),
            FactRamp(fact: "fact.flown", full: 2, none: 8),
            Not(TargetHas("creature.undead")),
        ]))"#,
    )
    .unwrap();
    let npc = actor(&mut app, [modifier]);
    let query = flown(5.0).tag("delivery.attack.melee");
    assert!(close(value::<Damage>(&app, npc, &query), 15.0));
    let undead = query.target_tag("creature.undead");
    assert!(close(value::<Damage>(&app, npc, &undead), 10.0));
}

#[test]
fn a_modifier_is_checked_against_the_attributes_and_tags() {
    let mut app = bevy::prelude::App::new();
    attributes(&mut app);
    let registry = app.world().resource::<AttributeRegistry>().clone();
    let tags = TagTree::build(&[
        Tag {
            id: Id::from_static("delivery"),
        },
        Tag {
            id: Id::from_static("damage"),
        },
    ])
    .unwrap();
    let check = |modifier: Modifier| {
        modifier
            .check(&registry, &tags)
            .map_err(|error| error.to_string())
    };
    assert_eq!(
        check(Modifier::flat::<Damage>(1.0).read_tagged("delivery")),
        Ok(())
    );
    assert_eq!(
        check(Modifier::flat::<Damage>(1.0).read_tagged("delivery.spell")),
        Err("there's no tag called `delivery.spell`".to_owned())
    );
    let unknown: Modifier = ron::from_str(r#"(attribute: "focus", op: Flat, value: 1)"#).unwrap();
    assert_eq!(
        check(unknown),
        Err("there's no attribute called `focus`".to_owned())
    );
    assert!(check(Modifier::flat::<Damage>(f32::INFINITY)).is_err());
}
