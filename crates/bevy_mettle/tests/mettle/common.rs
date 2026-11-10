//! What the tests share: a game's attributes, and an app keeping them.

use std::sync::OnceLock;

use bevy::app::PluginGroupBuilder;
use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::core::registry::AttributeId;
use bevy_mettle::prelude::*;

#[derive(Attribute)]
#[attribute(base = 10.0)]
pub struct Strength;

#[derive(Attribute)]
#[attribute(base = 50.0, min = 0.0, max = MaxLife)]
pub struct Life;

#[derive(Attribute)]
#[attribute(base = 50.0)]
pub struct MaxLife;

#[derive(Attribute)]
#[attribute(base = 10.0)]
pub struct Damage;

#[derive(Attribute)]
#[attribute(base = 30.0, min = 0.0)]
pub struct Mana;

#[derive(Attribute)]
#[attribute(base = 1.0)]
pub struct AttackSpeed;

#[derive(Attribute)]
#[attribute(base = 4.0)]
pub struct Cooldown;

#[derive(Attribute)]
#[attribute(base = 1.0, rounding = Down)]
pub struct Projectiles;

#[derive(Attribute)]
#[attribute(base = 1.0, rounding = Up, max = 2.5)]
pub struct Charges;

#[derive(Attribute)]
#[attribute(max = MaxResistance)]
pub struct Resistance;

#[derive(Attribute)]
#[attribute(base = 0.75)]
pub struct MaxResistance;

/// Damage types: what reads of resistances are tagged.
pub const PHYSICAL: &str = "damage.physical";
pub const FIRE: &str = "damage.elemental.fire";

/// The fact the tests' reads supply.
pub const FLOWN: &str = "fact.flown";

/// Adds every attribute the tests name.
pub fn attributes(app: &mut App) {
    app.add_attribute::<Strength>()
        .add_attribute::<Life>()
        .add_attribute::<MaxLife>()
        .add_attribute::<Damage>()
        .add_attribute::<Mana>()
        .add_attribute::<AttackSpeed>()
        .add_attribute::<Cooldown>()
        .add_attribute::<Projectiles>()
        .add_attribute::<Charges>()
        .add_attribute::<Resistance>()
        .add_attribute::<MaxResistance>();
}

/// `A`'s number, as every app adding the tests' attributes with
/// [`attributes`] numbers it: the same code adds them in the same order, so
/// the same number, in every app here, as on a server and its clients.
pub fn id<A: Attribute>() -> AttributeId {
    registry()
        .id_of::<A>()
        .expect("one of the tests' attributes")
}

/// An app keeping the tests' attributes.
pub fn app() -> TestApp {
    TestApp::builder().with(attributes).build()
}

/// `spec`, resolved as every app adding the tests' attributes numbers them.
pub fn modifier(spec: ModifierSpec) -> Modifier {
    spec.resolve(registry()).expect("of the tests' attributes")
}

/// `specs`, resolved, as a source's modifiers.
pub fn modifiers(specs: impl IntoIterator<Item = ModifierSpec>) -> Modifiers {
    specs.into_iter().map(modifier).collect()
}

/// The attributes every app adding the tests' attributes has.
fn registry() -> &'static AttributeRegistry {
    static REGISTRY: OnceLock<AttributeRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut app = App::new();
        attributes(&mut app);
        app.world().resource::<AttributeRegistry>().clone()
    })
}

/// An actor with every attribute, `modifiers` of its own, folded.
pub fn actor(app: &mut TestApp, modifiers: impl IntoIterator<Item = ModifierSpec>) -> Entity {
    let actor = app.spawn((
        (
            Base::<Strength>::default(),
            Base::<Life>::default(),
            Base::<MaxLife>::default(),
            Base::<Damage>::default(),
            Base::<Mana>::default(),
            Base::<AttackSpeed>::default(),
            Base::<Cooldown>::default(),
            Base::<Projectiles>::default(),
            Base::<Charges>::default(),
        ),
        (
            Base::<Resistance>::default(),
            Base::<MaxResistance>::default(),
        ),
    ));
    app.world_mut().entity_mut(actor).modifiers(modifiers);
    app.tick();
    actor
}

/// `actor`'s `A` with nothing in play, read now.
pub fn get<A: Attribute>(app: &TestApp, actor: Entity) -> f32 {
    value::<A>(app, actor, &AttributeContext::new())
}

/// `actor`'s `A` for `query`, read now.
pub fn value<A: Attribute>(app: &TestApp, actor: Entity, query: &AttributeContext<'_>) -> f32 {
    app.world()
        .value::<A>(actor, query)
        .expect("the actor has it")
}

/// Sets `actor`'s `A`'s base.
pub fn set_base<A: Attribute>(app: &mut TestApp, actor: Entity, value: f32) {
    app.world_mut()
        .get_mut::<Base<A>>(actor)
        .expect("the actor has it")
        .set(value);
}

/// `actor` owns `tag` once more, or once less.
pub fn own(app: &mut TestApp, actor: Entity, tag: &'static str, on: bool) {
    let mut owned = app
        .world_mut()
        .get_mut::<OwnedTags>(actor)
        .expect("an actor");
    if on {
        owned.add(tag);
    } else {
        owned.remove(tag);
    }
}

/// Whether `a` and `b` are the same, but for rounding.
pub fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

/// `MettlePlugins`, but for `MettleNetPlugin` with `lightyear` on: these
/// apps have no lightyear to carry it.
pub fn mettle() -> PluginGroupBuilder {
    let plugins = MettlePlugins.build();
    #[cfg(feature = "lightyear")]
    let plugins = plugins.disable::<MettleNetPlugin>();
    plugins
}
