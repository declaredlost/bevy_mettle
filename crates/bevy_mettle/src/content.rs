use bevy::ecs::change_detection::Tick;
use bevy::prelude::*;
#[cfg(feature = "ability")]
use bevy_mettle_ability::behaviour::BehaviourRegistry;
#[cfg(feature = "ability")]
use bevy_mettle_ability::data::{AbilityData, Grant};
#[cfg(feature = "ability")]
use bevy_mettle_ability::{AbilityChecks, AbilityRegistry};
#[cfg(feature = "effect")]
use bevy_mettle_attribute::registry::AttributeRegistry;
use bevy_mettle_core::curve::{CurveData, CurveRegistry};
use bevy_mettle_core::tag::{TagKey, TagRegistry};
use bevy_mettle_core::{Error, Errors};
#[cfg(feature = "effect")]
use bevy_mettle_effect::code::{CalculationRegistry, ExecutionRegistry, RouteRegistry};
#[cfg(feature = "effect")]
use bevy_mettle_effect::data::EffectData;
#[cfg(feature = "effect")]
use bevy_mettle_effect::{EffectChecks, EffectRegistry};
use serde::{Deserialize, Serialize};

/// The game's content, all of it: tags, curves, effects and abilities.
///
/// Tags, curves, effects, abilities, and what effects grant.
///
/// Mettle doesn't load anything: the game reads its files however it likes
/// and inserts this as a resource, as the app is built or again on a reload. As
/// it changes, [`MettleContentPlugin`] checks all of it at once, against the
/// attributes, behaviours and effect code the app added: if it all holds,
/// the tag tree, the curves and the registries are replaced and
/// [`ContentLoaded`] is triggered; if not, the content before stays, each
/// error is logged, and [`ContentRejected`] is triggered with them.
///
/// ```
/// # use bevy_mettle::prelude::*;
/// let content = MettleContent {
///     tags: vec![
///         TagKey::named("cooldown"),
///         TagKey::named("cooldown.fireball"),
///     ],
///     effects: vec![
///         EffectData::lasting("fireball.cooldown", 2.0)
///             .grant("cooldown.fireball"),
///     ],
///     ..Default::default()
/// };
/// ```
#[derive(Resource, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MettleContent {
    /// The gameplay tags there are, beside those code adds
    /// ([`add_tag`](bevy_mettle_core::tag::TagApp::add_tag)).
    pub tags: Vec<TagKey>,
    /// The curves effects scale by.
    pub curves: Vec<CurveData>,
    /// The effects there are.
    #[cfg(feature = "effect")]
    pub effects: Vec<EffectData>,
    /// The abilities there are.
    #[cfg(feature = "ability")]
    pub abilities: Vec<AbilityData>,
    /// The abilities each effect grants while it lasts.
    #[cfg(feature = "ability")]
    pub grants: Vec<Grant>,
}

/// Content checked, ready to be loaded.
struct Checked {
    tags: TagRegistry,
    curves: CurveRegistry,
    #[cfg(feature = "effect")]
    effects: EffectRegistry,
    #[cfg(feature = "ability")]
    abilities: AbilityRegistry,
}

impl MettleContent {
    /// Checks all of it against what `world` added: the attributes, the
    /// behaviours, the effect code, and the tags, effects and abilities code
    /// names. What [`MettleContentPlugin`] checks it by, for an answer at
    /// once: a mod before it's loaded, a test.
    ///
    /// Numbered after what `world` has loaded: tags, curves, effects and
    /// abilities there keep their numbers.
    ///
    /// # Errors
    ///
    /// Everything wrong, at once. Effects are checked once the tags and the
    /// curves hold, and abilities once the effects do.
    pub fn check(&self, world: &World) -> Result<(), Errors> {
        self.checked(world).map(|_| ())
    }

    fn checked(&self, world: &World) -> Result<Checked, Errors> {
        let mut errors = Vec::new();
        // The content's tags, with those code adds.
        let tags = held(
            resource::<TagRegistry>(world).rebuild(self.tags.iter().copied()),
            &mut errors,
        );
        let curves = held(
            resource::<CurveRegistry>(world).rebuild(&self.curves),
            &mut errors,
        );
        #[cfg(feature = "effect")]
        let effects = match (&tags, &curves) {
            (Some(tags), Some(curves)) => held(
                resource::<EffectRegistry>(world).recheck(
                    &self.effects,
                    EffectChecks {
                        tags,
                        curves,
                        attributes: &resource::<AttributeRegistry>(world),
                        executions: &resource::<ExecutionRegistry>(world),
                        calculations: &resource::<CalculationRegistry>(world),
                        routes: &resource::<RouteRegistry>(world),
                    },
                ),
                &mut errors,
            ),
            _ => None,
        };
        #[cfg(feature = "ability")]
        let abilities = match (&tags, &effects) {
            (Some(tags), Some(effects)) => held(
                resource::<AbilityRegistry>(world).recheck(
                    &self.abilities,
                    &self.grants,
                    AbilityChecks {
                        tags,
                        effects,
                        behaviours: &resource::<BehaviourRegistry>(world),
                    },
                ),
                &mut errors,
            ),
            _ => None,
        };
        match (errors.is_empty(), tags, curves) {
            (true, Some(tags), Some(curves)) => Ok(Checked {
                tags,
                curves,
                #[cfg(feature = "effect")]
                effects: effects.expect("checked"),
                #[cfg(feature = "ability")]
                abilities: abilities.expect("checked"),
            }),
            _ => Err(Errors(errors)),
        }
    }
}

/// `world`'s `R`, or an empty one: what content is numbered after, and checked
/// against.
fn resource<R: Resource + Clone + Default>(world: &World) -> R {
    world.get_resource::<R>().cloned().unwrap_or_default()
}

/// What `result` built, its errors kept in `errors`.
fn held<T>(result: Result<T, Errors>, errors: &mut Vec<Error>) -> Option<T> {
    result.map_err(|wrong| errors.extend(wrong.0)).ok()
}

/// Triggered as [`MettleContent`] has been checked and loaded.
#[derive(Event, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentLoaded;

/// Triggered as [`MettleContent`] was found wrong: the content before stays.
#[derive(Event, Debug, Clone, PartialEq)]
pub struct ContentRejected {
    /// Everything wrong with it.
    pub errors: Errors,
}

/// Loads [`MettleContent`] as it changes, before any part runs, or rejects it.
///
/// Checked, then its tag tree, curves and registries replaced, or rejected.
///
/// Inserted as the app is built, it's loaded in `PreStartup`, for `Startup`
/// systems to grant and apply what it defines; inserted or changed later, in
/// the next `PreUpdate`.
///
/// In [`MettlePlugins`](crate::MettlePlugins); add it yourself if you add the
/// parts' plugins one by one.
#[derive(Default)]
pub struct MettleContentPlugin;

impl Plugin for MettleContentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Loaded>()
            .add_systems(PreStartup, load.run_if(changed))
            .add_systems(PreUpdate, loading());
    }
}

/// Loading content in `PreUpdate`: before abilities serve and resolve what
/// arrived, so they do with the content it names. Effects and attributes
/// run after it, in the fixed step.
fn loading() -> bevy::ecs::schedule::ScheduleConfigs<bevy::ecs::system::ScheduleSystem> {
    let load = load.run_if(changed);
    #[cfg(feature = "ability")]
    let load = load.before(bevy_mettle_ability::AbilitySystems);
    load.into_configs()
}

/// The change to [`MettleContent`] last loaded or rejected: whichever
/// schedule saw it first, once.
#[derive(Resource, Default)]
struct Loaded(Option<Tick>);

/// Whether the content changed since it was last loaded or rejected: so
/// [`load`], which needs the whole world, runs only then.
fn changed(content: Option<Res<MettleContent>>, loaded: Res<Loaded>) -> bool {
    content.is_some_and(|content| loaded.0 != Some(content.last_changed()))
}

/// Checks the content as it changed, and loads it or rejects it.
fn load(world: &mut World) {
    let changed = world.resource_ref::<MettleContent>().last_changed();
    world.resource_mut::<Loaded>().0 = Some(changed);
    let checked = world.resource::<MettleContent>().checked(world);
    match checked {
        Ok(checked) => {
            world.insert_resource(checked.tags);
            world.insert_resource(checked.curves);
            #[cfg(feature = "effect")]
            world.insert_resource(checked.effects);
            #[cfg(feature = "ability")]
            world.insert_resource(checked.abilities);
            world.trigger(ContentLoaded);
        }
        Err(errors) => {
            for error in &errors.0 {
                error!("mettle content rejected: {error}");
            }
            world.trigger(ContentRejected { errors });
        }
    }
}
