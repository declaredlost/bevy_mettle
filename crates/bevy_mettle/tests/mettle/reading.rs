//! Reading: the formula, bases, limits, families, rounding, `per`, local
//! modifiers, breakdowns, and reads in a system.

use bevy::prelude::*;
use bevy_mettle::prelude::*;

use crate::common::{
    AttackSpeed, Charges, Cooldown, Damage, FIRE, Life, MaxLife, MaxResistance, PHYSICAL,
    Projectiles, Resistance, Strength, actor, app, close, get, own, set_base, value,
};

#[test]
fn an_attribute_is_flat_then_additive_then_multiplicative() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::flat::<Damage>(10.0),
            Modifier::additive::<Damage>(0.2),
            Modifier::additive::<Damage>(0.3),
            Modifier::multiplicative::<Damage>(0.2),
            Modifier::multiplicative::<Damage>(0.5),
        ],
    );
    // (10 + 10) × 1.5 × 1.2 × 1.5
    assert!(close(get::<Damage>(&app, npc), 54.0));
}

#[test]
fn divide_sums_then_divides_and_unscaled_comes_after() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::divide::<Cooldown>(0.5),
            Modifier::divide::<Cooldown>(0.5),
            Modifier::additive::<Damage>(1.0),
            Modifier::unscaled::<Damage>(5.0),
        ],
    );
    assert!(close(get::<Cooldown>(&app, npc), 2.0));
    assert!(close(get::<Damage>(&app, npc), 25.0));
}

#[test]
fn a_base_modifier_replaces_the_base_and_an_override_everything() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::base::<Damage>(20.0),
            Modifier::additive::<Damage>(0.5),
            Modifier::overriding::<AttackSpeed>(3.0),
            Modifier::additive::<AttackSpeed>(1.0),
        ],
    );
    assert!(close(get::<Damage>(&app, npc), 30.0));
    assert!(close(get::<AttackSpeed>(&app, npc), 3.0));
}

#[test]
fn reduced_or_less_past_everything_is_nothing() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::additive::<Damage>(-1.5),
            Modifier::multiplicative::<AttackSpeed>(-2.0),
        ],
    );
    assert!(close(get::<Damage>(&app, npc), 0.0));
    assert!(close(get::<AttackSpeed>(&app, npc), 0.0));
}

#[test]
fn a_whole_number_attribute_comes_rounded() {
    let mut app = app();
    let npc = actor(&mut app, [Modifier::flat::<Projectiles>(1.9)]);
    assert!(close(get::<Projectiles>(&app, npc), 2.0));
}

#[test]
fn a_limit_counts_what_counts_for_the_read() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::flat::<Resistance>(0.9),
            Modifier::flat::<MaxResistance>(0.05).read_tagged(FIRE),
        ],
    );
    let fire = AttributeContext::new().tag(FIRE);
    let physical = AttributeContext::new().tag(PHYSICAL);
    assert!(close(value::<Resistance>(&app, npc, &fire), 0.8));
    assert!(close(value::<Resistance>(&app, npc, &physical), 0.75));
}

#[test]
fn a_modifier_read_tagged_a_branch_counts_for_reads_tagged_under_it() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [Modifier::flat::<Resistance>(0.2).read_tagged("damage.elemental")],
    );
    let fire = AttributeContext::new().tag(FIRE);
    let physical = AttributeContext::new().tag(PHYSICAL);
    assert!(close(value::<Resistance>(&app, npc, &fire), 0.2));
    assert!(close(value::<Resistance>(&app, npc, &physical), 0.0));
}

#[test]
fn a_limit_reads_the_other_attribute_for_the_same_read() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::flat::<Resistance>(0.9),
            Modifier::flat::<MaxResistance>(0.1).read_tagged("delivery.spell"),
        ],
    );
    assert!(close(get::<Resistance>(&app, npc), 0.75));
    let spell = AttributeContext::new().tag("delivery.spell");
    assert!(close(value::<Resistance>(&app, npc, &spell), 0.85));
}

#[test]
fn a_base_is_kept_within_limits_on_reads() {
    let mut app = app();
    let npc = actor(&mut app, [Modifier::flat::<MaxLife>(10.0)]);
    set_base::<Life>(&mut app, npc, 35.0);
    app.tick();
    assert!(close(get::<Life>(&app, npc), 35.0), "damage taken");
    set_base::<Life>(&mut app, npc, 500.0);
    app.tick();
    assert!(close(get::<Life>(&app, npc), 60.0), "at most max life");
    set_base::<Life>(&mut app, npc, -5.0);
    app.tick();
    assert!(close(get::<Life>(&app, npc), 0.0), "at least none");
}

#[test]
fn a_per_modifier_counts_once_for_each_whole_so_much() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::flat::<MaxLife>(2.0).per::<Strength>(10.0),
            Modifier::flat::<Strength>(15.0),
        ],
    );
    assert!(close(get::<MaxLife>(&app, npc), 54.0));
}

#[test]
fn a_whole_number_attribute_is_rounded_then_kept_within_its_limits() {
    let mut app = app();
    let npc = actor(&mut app, [Modifier::flat::<Charges>(1.8)]);
    assert!(
        close(get::<Charges>(&app, npc), 2.5),
        "2.8, up to 3, kept to 2.5"
    );
}

#[test]
fn of_bases_and_overrides_the_last_counts_whether_or_not_it_always_does() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::overriding::<Damage>(1.0).owner_has("state.low_life"),
            Modifier::overriding::<Damage>(2.0),
        ],
    );
    own(&mut app, npc, "state.low_life", true);
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 2.0), "the later");
    let other = actor(
        &mut app,
        [
            Modifier::overriding::<Damage>(2.0),
            Modifier::overriding::<Damage>(1.0).owner_has("state.low_life"),
        ],
    );
    own(&mut app, other, "state.low_life", true);
    app.tick();
    assert!(
        close(get::<Damage>(&app, other), 1.0),
        "the later, while it holds"
    );
}

#[test]
fn a_loop_only_modifiers_make_stops_where_it_would_loop() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::flat::<Strength>(1.0).per::<MaxLife>(10.0),
            Modifier::flat::<MaxLife>(1.0).per::<Strength>(10.0),
        ],
    );
    assert!(close(get::<MaxLife>(&app, npc), 51.0));
    assert!(close(get::<Strength>(&app, npc), 15.0));
}

#[test]
fn a_local_modifier_counts_only_for_reads_naming_its_source() {
    let mut app = app();
    let npc = actor(&mut app, []);
    let sword = app.spawn((
        ModifierOf(npc),
        Modifiers(vec![Modifier::flat::<Damage>(7.0).local()]),
    ));
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 10.0));
    assert!(close(
        value::<Damage>(&app, npc, &AttributeContext::new().source(sword)),
        17.0
    ));
}

#[test]
fn a_breakdown_reports_the_sums_apart() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::flat::<Damage>(5.0),
            Modifier::additive::<Damage>(0.5),
            Modifier::multiplicative::<Damage>(1.0),
            Modifier::unscaled::<Damage>(1.0),
        ],
    );
    let breakdown = app.run_system(move |attributes: AttributeReader| {
        attributes
            .breakdown::<Damage>(npc, &AttributeContext::new())
            .unwrap()
    });
    assert!(close(breakdown.base, 10.0));
    assert!(close(breakdown.flat, 5.0));
    assert!(close(breakdown.additive, 0.5));
    assert!(close(breakdown.multiplier, 2.0));
    assert!(close(breakdown.unscaled, 1.0));
    assert!(close(breakdown.total(), 46.0));
}

#[test]
fn current_values_are_plain_components() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::flat::<Projectiles>(2.4),
            Modifier::flat::<Resistance>(0.3),
        ],
    );
    let (count, resistance) = app.run_system(
        move |projectiles: Query<&Current<Projectiles>>,
              resistances: Query<&Current<Resistance>>| {
            (
                projectiles.get(npc).unwrap().get(),
                resistances.get(npc).unwrap().get(),
            )
        },
    );
    assert!(close(count, 3.0));
    assert!(close(resistance, 0.3));
}

#[test]
fn an_actor_without_an_attribute_has_no_value_of_it() {
    let mut app = app();
    let bare = app.spawn(Base::<Damage>::default());
    app.tick();
    let reads = app.run_system(move |attributes: AttributeReader| {
        (
            attributes.value::<Damage>(bare, &AttributeContext::new()),
            attributes.value::<Life>(bare, &AttributeContext::new()),
        )
    });
    assert_eq!(reads, (Some(10.0), None));
}

#[test]
fn a_per_modifier_counts_a_fractional_every_whole() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [Modifier::flat::<Damage>(1.0).per::<Resistance>(0.1)],
    );
    set_base::<Resistance>(&mut app, npc, 0.3);
    app.tick();
    assert!(
        close(get::<Damage>(&app, npc), 13.0),
        "three times, not two"
    );
}

#[test]
fn a_multiplicative_per_modifier_multiplies_once_each_time() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [Modifier::multiplicative::<Damage>(0.1).per::<Strength>(5.0)],
    );
    // 10 × 1.1 × 1.1
    assert!(close(get::<Damage>(&app, npc), 12.1));
}

#[test]
fn a_divide_of_minus_all_or_less_divides_by_a_hundredth() {
    let mut app = app();
    let npc = actor(&mut app, [Modifier::divide::<Cooldown>(-1.5)]);
    assert!(close(get::<Cooldown>(&app, npc), 400.0));
}

#[test]
fn a_change_to_a_base_is_kept_within_a_limit_changed_since_kept() {
    let mut app = app();
    let npc = actor(&mut app, []);
    set_base::<MaxLife>(&mut app, npc, 200.0);
    app.world_mut()
        .apply_to_base(npc, &Life::ID, Operation::Flat, 100.0);
    assert!(close(app.entity(npc).get::<Base<Life>>().get(), 150.0));
}

#[test]
fn a_per_loop_reads_the_same_whichever_end_is_read_first() {
    let mut app = app();
    let npc = actor(
        &mut app,
        [
            Modifier::flat::<Strength>(1.0).per::<Damage>(5.0),
            Modifier::flat::<Damage>(1.0).per::<Strength>(5.0),
        ],
    );
    // Read afresh, each from its own end, against what refreshing all found.
    let fresh = AttributeContext::new().tag("damage");
    for read in [
        value::<Strength>(&app, npc, &fresh),
        value::<Damage>(&app, npc, &fresh),
        get::<Strength>(&app, npc),
        get::<Damage>(&app, npc),
    ] {
        assert!(close(read, 12.0), "{read}");
    }
}

#[test]
fn taking_a_base_off_takes_the_attribute_away() {
    let mut app = app();
    let npc = actor(&mut app, []);
    app.entity_mut(npc).remove::<Base<Damage>>();
    app.tick();
    assert_eq!(
        app.world()
            .value_at(npc, &Damage::ID, &AttributeContext::new()),
        None
    );
    app.entity(npc).assert_lacks::<Current<Damage>>();
    app.entity_mut(npc).insert(Base::<Damage>::new(4.0));
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 4.0), "and back");
}

#[test]
#[should_panic(expected = "are both named `damage`")]
fn two_attributes_cant_share_a_name() {
    #[derive(Attribute)]
    #[attribute(name = "damage")]
    struct OtherDamage;
    let mut app = app();
    app.app_mut().add_attribute::<OtherDamage>();
}

#[test]
fn an_actors_attributes_reflect() {
    use bevy::ecs::reflect::ReflectComponent;
    use bevy::reflect::TypePath;

    let mut app = app();
    let npc = actor(&mut app, [Modifier::flat::<Damage>(5.0)]);
    let registry = app.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let base = registry
        .get_with_type_path(Base::<Damage>::type_path())
        .expect("registered");
    assert_eq!(
        Base::<Damage>::type_path(),
        "bevy_mettle_attribute::Base<damage>"
    );
    let reflected = base
        .data::<ReflectComponent>()
        .unwrap()
        .reflect(app.world().entity(npc))
        .unwrap();
    assert!(
        reflected
            .reflect_partial_eq(&Base::<Damage>::new(10.0))
            .unwrap()
    );
    for path in [
        Current::<Damage>::type_path(),
        Modifiers::type_path(),
        ModifierOf::type_path(),
        OwnedTags::type_path(),
        bevy_mettle::attribute::registry::AttributeState::type_path(),
    ] {
        assert!(registry.get_with_type_path(path).is_some(), "{path}");
    }
}
