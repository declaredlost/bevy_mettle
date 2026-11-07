//! Weighing expressions.

use super::{Condition, Context};
use crate::Id;
use crate::tag::{Tag, TagCounts, TagTree, Tags};

const FLOWN: Id<Tag> = Id::from_static("fact.flown");
const BEHIND: Id<Tag> = Id::from_static("fact.behind");
const HIT_COUNT: Id<Tag> = Id::from_static("fact.hit_count");

/// A read: its tags, its owner's and target's, and its facts.
#[derive(Default)]
struct Read {
    tags: Tags,
    owner: TagCounts,
    target: TagCounts,
    source: TagCounts,
    facts: Vec<(Id<Tag>, f32)>,
}

impl Context for Read {
    fn tag(&self, tag: &Id<Tag>) -> bool {
        self.tags.contains(tag)
    }

    fn has(&self, tag: &Id<Tag>) -> bool {
        self.owner.contains(tag)
    }

    fn target(&self, tag: &Id<Tag>) -> bool {
        self.target.contains(tag)
    }

    fn source(&self, tag: &Id<Tag>) -> bool {
        self.source.contains(tag)
    }

    fn fact(&self, fact: &Id<Tag>) -> Option<f32> {
        self.facts
            .iter()
            .find(|(it, _)| it == fact)
            .map(|(_, value)| *value)
    }
}

fn with(fact: Id<Tag>, value: f32) -> Read {
    Read {
        facts: vec![(fact, value)],
        ..Read::default()
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn a_tag_term_holds_for_the_tag_or_one_under_it() {
    let read = Read {
        tags: [Id::from_static("delivery.attack.melee")]
            .into_iter()
            .collect(),
        ..Read::default()
    };
    assert!(Condition::read_tagged("delivery.attack").holds(&read));
    assert!(Condition::read_tagged("delivery.attack.melee").holds(&read));
    assert!(!Condition::read_tagged("delivery.spell").holds(&read));
}

#[test]
fn owned_and_target_tags_are_apart_from_the_reads() {
    let mut read = Read::default();
    read.owner.add(&Id::from_static("state.low_life"));
    read.target.add(&Id::from_static("creature.undead"));
    assert!(Condition::owner_has("state").holds(&read));
    assert!(!Condition::target_has("state").holds(&read));
    assert!(Condition::target_has("creature.undead").holds(&read));
    assert!(!Condition::read_tagged("creature.undead").holds(&read));
}

#[test]
fn a_ramp_weighs_in_proportion() {
    let ramp = Condition::fact_ramp(FLOWN, 2.0, 8.0);
    assert!(close(ramp.weight(&with(FLOWN, 1.0)), 1.0));
    assert!(close(ramp.weight(&with(FLOWN, 5.0)), 0.5));
    assert!(close(ramp.weight(&with(FLOWN, 9.0)), 0.0));
    assert!(
        close(ramp.weight(&Read::default()), 0.0),
        "no fact, no weight"
    );
}

#[test]
fn facts_compare_as_written() {
    assert!(Condition::fact_set(BEHIND).holds(&with(BEHIND, 1.0)));
    assert!(!Condition::fact_set(BEHIND).holds(&with(BEHIND, 0.0)));
    assert!(Condition::fact_above(FLOWN, 2.0).holds(&with(FLOWN, 2.5)));
    assert!(
        !Condition::fact_above(FLOWN, 2.0).holds(&with(FLOWN, 2.0)),
        "above, not at"
    );
    assert!(!Condition::fact_below(FLOWN, 2.0).holds(&with(FLOWN, 2.0)));
    let every = Condition::fact_every(HIT_COUNT, 3);
    let held: Vec<bool> = (0_u8..=6)
        .map(|count| every.holds(&with(HIT_COUNT, f32::from(count))))
        .collect();
    assert_eq!(held, [false, false, false, true, false, false, true]);
}

#[test]
fn combinators_weigh_as_product_greatest_and_complement() {
    let ramp = Condition::fact_ramp(FLOWN, 2.0, 8.0);
    let read = Read {
        tags: [Id::from_static("damage.fire")].into_iter().collect(),
        facts: vec![(FLOWN, 5.0)],
        ..Read::default()
    };
    assert!(close(
        Condition::all([ramp.clone(), Condition::read_tagged("damage")]).weight(&read),
        0.5
    ));
    assert!(close(
        Condition::any([ramp.clone(), Condition::read_tagged("spell")]).weight(&read),
        0.5
    ));
    assert!(close((!ramp.clone()).weight(&read), 0.5));
    assert_eq!(
        ramp.clone()
            .and(Condition::read_tagged("damage"))
            .and(Condition::fact_set(BEHIND)),
        Condition::all([
            ramp,
            Condition::read_tagged("damage"),
            Condition::fact_set(BEHIND)
        ])
    );
}

#[test]
fn it_reads_from_data_and_checks_against_the_tags() {
    let condition: Condition = ron::from_str(
        r#"All([ReadTagged("delivery.attack"), FactRamp(fact: "fact.flown", full: 2, none: 8), Not(TargetHas("creature.undead"))])"#,
    )
    .unwrap();
    let defs: Vec<Tag> = [
        "delivery",
        "delivery.attack",
        "creature",
        "fact",
        "fact.flown",
    ]
    .into_iter()
    .map(|id| Tag {
        id: Id::from_static(id),
    })
    .collect();
    let tags = TagTree::build(&defs).unwrap();
    assert_eq!(
        condition.check(&tags, None).unwrap_err().to_string(),
        "there's no tag called `creature.undead`"
    );
    let bad: Condition =
        ron::from_str(r#"FactRamp(fact: "fact.flown", full: 2, none: 2)"#).unwrap();
    assert_eq!(
        bad.check(&tags, Some("haste")).unwrap_err().to_string(),
        "a ramp of `fact.flown` from 2 to 2: they have to differ (in `haste`)"
    );
}

#[test]
fn source_terms_settle_as_the_source_was() {
    let condition = Condition::all([
        Condition::source_has("state.enraged"),
        Condition::read_tagged("delivery"),
    ]);
    let enraged = condition.settle_source(&|tag| tag.as_str() == "state.enraged");
    let calm = condition.settle_source(&|_| false);
    let read = Read {
        tags: [Id::from_static("delivery.attack")].into_iter().collect(),
        ..Read::default()
    };
    assert!(enraged.holds(&read));
    assert!(!calm.holds(&read));
    let mut sourced = Read::default();
    sourced
        .source
        .add(&Id::from_static("state.enraged.berserk"));
    assert!(Condition::source_has("state.enraged").holds(&sourced));
}

#[test]
fn numbers_are_checked_without_the_tag_tree() {
    let flat = Condition::all([Condition::fact_ramp(FLOWN, 5.0, 5.0)]);
    assert!(flat.check_values(None).is_err(), "a ramp that doesn't ramp");
    assert!(
        Condition::fact_ramp(FLOWN, 2.0, 8.0)
            .and(Condition::owner_has("not.in.any.tree"))
            .check_values(None)
            .is_ok(),
        "tags aren't checked"
    );
}
