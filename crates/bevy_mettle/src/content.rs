use bevy::ecs::change_detection::Tick;
use bevy::prelude::*;
#[cfg(feature = "ability")]
use bevy_mettle_ability::behaviour::BehaviourRegistry;
#[cfg(feature = "ability")]
use bevy_mettle_ability::data::AbilityData;
#[cfg(feature = "ability")]
use bevy_mettle_ability::{AbilityRegistry, AbilityScope};
#[cfg(feature = "effect")]
use bevy_mettle_attribute::registry::AttributeRegistry;
use bevy_mettle_core::curve::{CurveData, CurveRegistry};
use bevy_mettle_core::tag::{TagKey, TagRegistry};
use bevy_mettle_core::{Error, Errors};
#[cfg(feature = "effect")]
use bevy_mettle_effect::code::{CalculationRegistry, ExecutionRegistry, RouteRegistry};
#[cfg(feature = "effect")]
use bevy_mettle_effect::context::ContextDataRegistry;
#[cfg(feature = "effect")]
use bevy_mettle_effect::data::EffectData;
#[cfg(feature = "effect")]
use bevy_mettle_effect::{EffectRegistry, EffectScope};
use serde::{Deserialize, Serialize};

/// What data adds to what code defines: tags, curves, effects and abilities.
///
/// Code defines a game's effects, abilities and curves, as types
/// ([`add_effect`](bevy_mettle_effect::code::EffectApp::add_effect),
/// [`add_ability`](bevy_mettle_ability::AbilityApp::add_ability),
/// [`add_curve`](bevy_mettle_core::curve::CurveApp::add_curve)). Data adds
/// more (a mod's, a server's), each by a key code doesn't define, and tunes
/// curves: a curve here replaces the one code defines with its key.
///
/// Mettle doesn't load anything: the game reads its files however it likes
/// and inserts this as a resource, as the app is built or again on a reload.
/// [`MettleContentPlugin`] checks what code defines and this adds, all at
/// once, as the app starts, as this changes, and on a hot patch: if it all
/// holds, the tag tree, the curves and the registries are replaced and
/// [`ContentLoaded`] is triggered; if not, what was loaded before stays,
/// each error is logged, and [`ContentRejected`] is triggered with them.
///
/// ```
/// # use bevy_mettle::prelude::*;
/// # #[derive(Attribute)] struct Life;
/// # #[derive(Tag)] struct Burning;
/// let content = MettleContent {
///     tags: vec![TagKey::named("modded"), TagKey::named("modded.frost")],
///     effects: vec![
///         EffectData::new("frostbite")
///             .lasting(3.0)
///             .tag("modded.frost")
///             .modifier(EffectModifier::named("life".into(), Operation::Flat, -5.0)),
///     ],
///     ..Default::default()
/// };
/// ```
#[derive(Resource, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MettleContent {
    /// The gameplay tags it adds, beside those code adds
    /// ([`add_tag`](bevy_mettle_core::tag::TagApp::add_tag)).
    pub tags: Vec<TagKey>,
    /// The curves it adds, and those it tunes: code's of the same key.
    pub curves: Vec<CurveData>,
    /// The effects it adds.
    #[cfg(feature = "effect")]
    pub effects: Vec<EffectData>,
    /// The abilities it adds.
    #[cfg(feature = "ability")]
    pub abilities: Vec<AbilityData>,
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
    /// Checks it with what `world`'s code defines: the attributes, the
    /// behaviours, the effect code, and the tags, curves, effects and
    /// abilities. What [`MettleContentPlugin`] checks it by, for an answer at
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
        // What abilities pay with: effects, and tags their cooldowns grant.
        #[cfg(feature = "ability")]
        let upkeep = resource::<AbilityRegistry>(world).upkeep(&self.abilities);
        #[cfg(feature = "ability")]
        let (upkeep_tags, upkeep_effects) = (upkeep.tags, upkeep.effects);
        #[cfg(not(feature = "ability"))]
        let upkeep_tags: Vec<TagKey> = Vec::new();
        // The content's tags, with those code adds, and those cooldowns grant.
        let tags = self.tags.iter().copied().chain(
            upkeep_tags
                .into_iter()
                .filter(|tag| !self.tags.contains(tag)),
        );
        let tags = held(resource::<TagRegistry>(world).rebuild(tags), &mut errors);
        let curves = held(
            resource::<CurveRegistry>(world).rebuild(&self.curves),
            &mut errors,
        );
        #[cfg(feature = "ability")]
        let effects: Vec<EffectData> = self.effects.iter().cloned().chain(upkeep_effects).collect();
        #[cfg(feature = "ability")]
        let effects = effects.as_slice();
        #[cfg(all(feature = "effect", not(feature = "ability")))]
        let effects = self.effects.as_slice();
        #[cfg(feature = "effect")]
        let effects = match (&tags, &curves) {
            (Some(tags), Some(curves)) => held(
                resource::<EffectRegistry>(world).rebuild(
                    effects,
                    EffectScope {
                        tags,
                        curves,
                        attributes: &resource::<AttributeRegistry>(world),
                        executions: &resource::<ExecutionRegistry>(world),
                        calculations: &resource::<CalculationRegistry>(world),
                        routes: &resource::<RouteRegistry>(world),
                        context: &resource::<ContextDataRegistry>(world),
                    },
                ),
                &mut errors,
            ),
            _ => None,
        };
        #[cfg(feature = "ability")]
        let abilities = match (&tags, &effects) {
            (Some(tags), Some(effects)) => held(
                resource::<AbilityRegistry>(world).rebuild(
                    &self.abilities,
                    AbilityScope {
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

/// Loads what code defines and [`MettleContent`] adds, or rejects it.
///
/// Checked, then the tag tree, curves and registries replaced, or rejected.
///
/// Loaded in `PreStartup`, for `Startup` systems to grant and apply what's
/// defined, with or without content; again in the next `PreUpdate` as
/// content is inserted or changes, and, with the `hotpatching` feature, as
/// code's hot patched, each type defining itself again.
///
/// In [`MettlePlugins`](crate::MettlePlugins); add it yourself if you add the
/// parts' plugins one by one.
#[derive(Default)]
pub struct MettleContentPlugin;

impl Plugin for MettleContentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Loaded>()
            .add_systems(PreStartup, load)
            .add_systems(PreUpdate, loading());
    }
}

/// Loading in `PreUpdate`: before abilities serve and resolve what arrived,
/// so they do with the content it names. Effects and attributes run after
/// it, in the fixed step.
fn loading() -> bevy::ecs::schedule::ScheduleConfigs<bevy::ecs::system::ScheduleSystem> {
    let load = load.run_if(due);
    #[cfg(feature = "ability")]
    let load = load.before(bevy_mettle_ability::AbilitySystems);
    load.into_configs()
}

/// The change to [`MettleContent`] last loaded or rejected, if there was
/// content: whichever schedule saw it first, once.
#[derive(Resource, Default)]
struct Loaded(Option<Tick>);

/// Whether the content changed since it was last loaded or rejected, or code
/// was hot patched since this last ran: so [`load`], which needs the whole
/// world, runs only then.
fn due(
    content: Option<Res<MettleContent>>,
    loaded: Res<Loaded>,
    #[cfg(feature = "hotpatching")] patched: Option<Res<bevy::ecs::HotPatchChanges>>,
) -> bool {
    #[cfg(feature = "hotpatching")]
    if patched.is_some_and(|patched| patched.is_changed()) {
        return true;
    }
    content.is_some_and(|content| loaded.0 != Some(content.last_changed()))
}

/// Checks what code defines and the content adds, and loads it or rejects
/// it.
fn load(world: &mut World) {
    let content = world
        .get_resource_ref::<MettleContent>()
        .map(|content| (content.last_changed(), MettleContent::clone(&content)));
    let content = match content {
        Some((changed, content)) => {
            world.resource_mut::<Loaded>().0 = Some(changed);
            content
        }
        None => MettleContent::default(),
    };
    match content.checked(world) {
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
