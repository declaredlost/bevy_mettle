//! When modifiers count: the read's tags, owned tags, the target's, and facts,
//! weighed.

use bevy_mettle::prelude::*;

use crate::common::{
    AttackSpeed, Damage, FLOWN, actor, app, attributes, close, get, id, own, value,
};

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
            Modifier::additive(id::<Damage>(), 1.0).condition(ramp()),
            Modifier::multiplicative(id::<AttackSpeed>(), 1.0).condition(ramp()),
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
            Modifier::base(id::<Damage>(), 40.0).condition(ramp()),
            Modifier::overriding(id::<AttackSpeed>(), 3.0).condition(ramp()),
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
        [Modifier::multiplicative(id::<AttackSpeed>(), 1.0).owner_has("state")],
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
        [Modifier::additive(id::<Damage>(), 0.5).read_tagged("damage.elemental")],
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
            Modifier::flat(id::<Damage>(), 5.0).condition(Condition::target_has("creature.undead")),
            Modifier::flat(id::<Damage>(), 1.0).condition(Condition::source_has("state.enraged")),
        ],
    );
    let mut undead = OwnedTags::default();
    undead.add("creature.undead.ghoul");
    let mut enraged = OwnedTags::default();
    enraged.add("state.enraged");
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
    let npc =
        actor(
            &mut app,
            [Modifier::flat(id::<Damage>(), 5.0)
                .condition(!Condition::target_has("creature.undead"))],
        );
    let undead = AttributeContext::new().target_tag("creature.undead");
    assert!(close(value::<Damage>(&app, npc, &undead), 10.0));
    assert!(close(get::<Damage>(&app, npc), 15.0));
}

#[test]
fn a_modifiers_condition_of_every_kind_counts() {
    let mut app = app();
    let condition = Condition::all([
        Condition::read_tagged("delivery.attack"),
        Condition::fact_ramp("fact.flown", 2.0, 8.0),
        !Condition::target_has("creature.undead"),
    ]);
    let npc = actor(
        &mut app,
        [Modifier::additive(id::<Damage>(), 1.0).condition(condition)],
    );
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
    let tags = TagRegistry::build([TagKey::named("delivery"), TagKey::named("damage")]).unwrap();
    let check = |modifier: Modifier| {
        modifier
            .check(&registry, &tags)
            .map_err(|error| error.to_string())
    };
    assert_eq!(
        check(Modifier::flat(id::<Damage>(), 1.0).read_tagged("delivery")),
        Ok(())
    );
    assert_eq!(
        check(Modifier::flat(id::<Damage>(), 1.0).read_tagged("delivery.spell")),
        Err("there's no tag called `delivery.spell`".to_owned())
    );
    assert_eq!(
        check(Modifier::flat(AttributeId::new(999), 1.0)),
        Err("a modifier of attribute 999, which there isn't".to_owned())
    );
    assert!(check(Modifier::flat(id::<Damage>(), f32::INFINITY)).is_err());
}
