//! Cues: what's shown of effects and abilities, for the game to play.
//!
//! A cue is a gameplay tag, `cue.burning`, `cue.hit.fire`. A *lasting* cue
//! holds while something lasts: an effect while it counts, an ability while
//! it's active, or one the game adds. Each is an entity of its own,
//! [`ActiveCue`] [`CueOf`] its actor, spawned as it starts and despawned as it
//! ends, for the game to hang what it plays on. A *one-shot* cue plays at a
//! moment: an effect executing, or one the game executes.
//!
//! [`CueAdded`], [`CueRemoved`] and [`CueExecuted`] are told on the actor.
//! Handlers added with [`add_cue_handler`](CueApp::add_cue_handler) are run
//! for the most specific tag they're added for: a fire hit runs the
//! `cue.hit.fire` handler if there is one, else the `cue.hit` one, never both.
//!
//! Over a network, the server's lasting cues ([`ServedCues`]) and its recent
//! one-shots ([`CueBursts`]) are replicated with the actor, to whoever sees
//! it: a client shows them next to its own, and plays a one-shot it predicted
//! once.

use std::collections::BTreeSet;

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::ecs::query::SpawnDetails;
use bevy::ecs::system::SystemId;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::registry::{AttributeId, EffectId, TagId};
use bevy_mettle_core::tag::{TagKey, TagRegistry};
use serde::{Deserialize, Serialize};

use crate::context::{ContextDataRegistry, ContextReplica, EffectContext};
use crate::events::Delta;

/// A cue to play: its tag, and what it's played with, such as its magnitude.
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    /// Its tag: `cue.hit.fire`.
    pub tag: TagKey,
    /// How much: what was dealt.
    pub magnitude: f32,
    /// What it changed each attribute's base by.
    pub changes: Vec<Delta>,
    /// The level of what played it.
    pub level: f32,
    /// What played it carried: who caused it, what struck, where.
    pub context: EffectContext,
    /// Whether it's this world's alone: a server doesn't serve it to clients.
    pub local: bool,
}

impl Cue {
    /// The cue `tag`, of no magnitude, at level 1, from no one.
    #[must_use]
    pub fn new(tag: impl Into<TagKey>) -> Self {
        Self {
            tag: tag.into(),
            magnitude: 0.0,
            changes: Vec::new(),
            level: 1.0,
            context: EffectContext::default(),
            local: false,
        }
    }

    /// Of `magnitude`.
    #[must_use]
    pub fn magnitude(mut self, magnitude: f32) -> Self {
        self.magnitude = magnitude;
        self
    }

    /// Having changed `attribute`'s base by `delta`.
    #[must_use]
    pub fn change(mut self, attribute: AttributeId, delta: f32) -> Self {
        self.changes.push(Delta { attribute, delta });
        self
    }

    /// At `level`.
    #[must_use]
    pub fn level(mut self, level: f32) -> Self {
        self.level = level;
        self
    }

    /// Caused by `source`.
    #[must_use]
    pub fn source(mut self, source: Entity) -> Self {
        self.context.source = Some(source);
        self
    }

    /// Played with `context`: who caused it, what struck, where.
    #[must_use]
    pub fn context(mut self, context: EffectContext) -> Self {
        self.context = context;
        self
    }

    /// This world's alone: not served to clients.
    #[must_use]
    pub fn local(mut self) -> Self {
        self.local = true;
        self
    }
}

/// On an actor: what holds each of its lasting cues here, by count.
///
/// An effect counting, an ability active, the game. Those a server serves,
/// and those it keeps to itself.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
#[require(ServedCues)]
pub struct CueSources {
    served: HashMap<TagKey, u32>,
    local: HashMap<TagKey, u32>,
}

impl CueSources {
    /// One more source of `tag`.
    fn add(&mut self, tag: TagKey, local: bool) {
        let counts = if local {
            &mut self.local
        } else {
            &mut self.served
        };
        *counts.entry(tag).or_default() += 1;
    }

    /// One less source of `tag`.
    pub(crate) fn remove(&mut self, tag: TagKey, local: bool) {
        let counts = if local {
            &mut self.local
        } else {
            &mut self.served
        };
        if let Some(count) = counts.get_mut(&tag) {
            *count -= 1;
            if *count == 0 {
                counts.remove(&tag);
            }
        }
    }

    /// The cues held, served or not.
    pub fn iter(&self) -> impl Iterator<Item = TagKey> {
        self.served.keys().chain(self.local.keys()).copied()
    }

    /// The cues held that a server serves.
    fn served(&self) -> BTreeSet<TagKey> {
        self.served.keys().copied().collect()
    }
}

/// On an actor: the lasting cues its server holds on it, replicated.
///
/// Replicated to the clients that see it. By their numbers in the
/// [`TagRegistry`]: a cue the tree hasn't isn't served.
#[derive(
    Component,
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize
)]
pub struct ServedCues(pub BTreeSet<TagId>);

/// On a lasting cue's entity: which cue it is, its tag, while it plays.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct ActiveCue {
    /// Its tag.
    pub tag: TagKey,
}

/// On a lasting cue's entity: the actor it plays on, as a Bevy relationship.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[relationship(relationship_target = ActiveCues)]
pub struct CueOf(pub Entity);

/// On an actor: its lasting cues' entities, kept by [`CueOf`]; despawned with
/// it.
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = CueOf, linked_spawn)]
pub struct ActiveCues(Vec<Entity>);

impl ActiveCues {
    /// The cues' entities.
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().copied()
    }
}

/// A one-shot cue a server played, as it's replicated in [`CueBursts`].
///
/// Its tag by its number in the [`TagRegistry`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Burst {
    /// Its number, counting up on its actor.
    pub seq: u64,
    /// The server's elapsed seconds as it played: how old it is.
    pub at: f64,
    /// Its tag.
    pub tag: TagId,
    /// How much.
    pub magnitude: f32,
    /// What it changed each attribute's base by.
    pub changes: Vec<Delta>,
    /// The level of what played it.
    pub level: f32,
    /// What played it carried: who caused it, what struck, where.
    pub context: ContextReplica,
    /// What it was played under, if an activation.
    pub activation: Option<u64>,
    /// The effect that played it, if one did.
    pub effect: Option<EffectId>,
}

/// On an actor: the one-shot cues its server played lately, replicated.
///
/// Replicated to the clients that see it, which play those they haven't.
#[derive(Component, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[component(map_entities)]
pub struct CueBursts {
    /// The next one's number.
    pub next: u64,
    /// Those played in the last second, oldest first.
    pub recent: Vec<Burst>,
}

impl MapEntities for CueBursts {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        for burst in &mut self.recent {
            burst.context.map_entities(mapper);
        }
    }
}

/// How long a server keeps a one-shot for its clients, in seconds: what a
/// client coming to see an actor plays of what's just happened on it.
const KEPT: f64 = 0.5;

/// How long a client remembers a one-shot it predicted, for the server's
/// copy, in seconds: past what the server keeps it for, it won't come.
const REMEMBERED: f64 = 2.0 * KEPT;

/// On a client's actor: the next one-shot it hasn't played.
#[derive(Component, Debug, Clone, Copy, Default)]
pub(crate) struct Played(u64);

/// On a client: the one-shots it played under its own activations, so the
/// server's copies aren't played again; each as often as it was played, with
/// the client's seconds it was played at.
#[derive(Resource, Debug, Default)]
pub(crate) struct Predicted(HashMap<(Entity, u64, TagKey), Vec<f64>>);

impl Predicted {
    /// Notes `tag` played on `actor` under `activation` at `at`.
    fn note(&mut self, actor: Entity, activation: u64, tag: TagKey, at: f64) {
        self.0.entry((actor, activation, tag)).or_default().push(at);
    }

    /// Takes one play of `tag` on `actor` under `activation`: whether there
    /// was one, the server's copy of it.
    fn take(&mut self, actor: Entity, activation: u64, tag: TagKey) -> bool {
        let key = (actor, activation, tag);
        let Some(plays) = self.0.get_mut(&key) else {
            return false;
        };
        plays.remove(0);
        if plays.is_empty() {
            self.0.remove(&key);
        }
        true
    }

    /// Forgets what was played before `before`: its copy won't come now.
    fn forget(&mut self, before: f64) {
        self.0.retain(|_, plays| {
            plays.retain(|at| *at >= before);
            !plays.is_empty()
        });
    }

    /// Forgets what was played under `activation`: the server rejected it,
    /// and plays none of it.
    pub(crate) fn forget_activation(&mut self, activation: u64) {
        self.0.retain(|(_, under, _), _| *under != activation);
    }
}

/// Told on an actor as a lasting cue starts on it, or comes into view.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct CueAdded {
    /// The actor.
    pub entity: Entity,
    /// The cue's entity.
    pub cue: Entity,
    /// Its tag.
    pub tag: TagKey,
    /// Whether it was going already as this world came to see the actor: a
    /// client the actor came into view of, rather than one that saw it start.
    pub joined: bool,
}

/// Told on an actor as a lasting cue ends on it, its entity still there.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct CueRemoved {
    /// The actor.
    pub entity: Entity,
    /// The cue's entity, despawned after.
    pub cue: Entity,
    /// Its tag.
    pub tag: TagKey,
}

/// Told on an actor as a one-shot cue plays on it, with what it dealt.
#[derive(EntityEvent, Debug, Clone, PartialEq)]
pub struct CueExecuted {
    /// The actor.
    pub entity: Entity,
    /// Its tag.
    pub tag: TagKey,
    /// How much: what was dealt to the first attribute it changed, the one
    /// it's about (a fireball's life).
    pub magnitude: f32,
    /// What it changed each attribute's base by, in the order it first
    /// changed it.
    pub changes: Vec<Delta>,
    /// The level of what played it.
    pub level: f32,
    /// What played it carried: who caused it, what struck, where.
    pub context: EffectContext,
    /// What it was played under, if an activation.
    pub activation: Option<u64>,
    /// The effect that played it, if one did.
    pub effect: Option<EffectId>,
}

impl CueExecuted {
    /// Where it hit, if what played it said: where its spark goes.
    #[must_use]
    pub fn location(&self) -> Option<Vec3> {
        self.context.hit.map(|hit| hit.point)
    }

    /// The surface's normal where it hit, if what played it said: which way
    /// its spark faces.
    #[must_use]
    pub fn normal(&self) -> Option<Vec3> {
        self.context.hit.map(|hit| hit.normal)
    }
}

/// What a cue handler is run with: a cue started, ended, or played.
#[derive(Debug, Clone, PartialEq)]
pub enum CueEvent {
    /// A lasting cue started.
    Added(CueAdded),
    /// A lasting cue ended.
    Removed(CueRemoved),
    /// A one-shot cue played.
    Executed(CueExecuted),
}

impl CueEvent {
    /// Its cue's tag.
    #[must_use]
    pub fn tag(&self) -> TagKey {
        match self {
            Self::Added(added) => added.tag,
            Self::Removed(removed) => removed.tag,
            Self::Executed(executed) => executed.tag,
        }
    }
}

/// The handlers there are, by the tag they're for.
#[derive(Resource, Debug, Default)]
pub(crate) struct CueHandlers(HashMap<TagKey, SystemId<In<CueEvent>>>);

/// The system set cues are shown and played in, in `PostUpdate`.
///
/// Lasting cues are shown in it, and one-shots from the server played.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CueSystems;

/// Adding cue handlers to an app: systems run for the cues under a tag.
pub trait CueApp {
    /// Runs `handler`, a system, for each cue tagged `tag`, or under it, that
    /// no handler for a more specific tag takes: as it starts, ends, or plays.
    fn add_cue_handler<M>(
        &mut self,
        tag: impl Into<TagKey>,
        handler: impl IntoSystem<In<CueEvent>, (), M> + 'static,
    ) -> &mut Self;
}

impl CueApp for App {
    fn add_cue_handler<M>(
        &mut self,
        tag: impl Into<TagKey>,
        handler: impl IntoSystem<In<CueEvent>, (), M> + 'static,
    ) -> &mut Self {
        let id = self.world_mut().register_system(handler);
        self.world_mut()
            .get_resource_or_init::<CueHandlers>()
            .0
            .insert(tag.into(), id);
        self
    }
}

/// Playing cues on actors, as commands: one-shots, and lasting cues held.
pub trait CueCommands {
    /// Plays the one-shot `cue` on this actor.
    fn execute_cue(&mut self, cue: Cue) -> &mut Self;

    /// Holds the lasting `cue` on this actor, once more: until it's removed
    /// as often.
    fn add_cue(&mut self, cue: Cue) -> &mut Self;

    /// Holds the lasting `cue` on this actor once less.
    fn remove_cue(&mut self, cue: Cue) -> &mut Self;
}

impl CueCommands for EntityCommands<'_> {
    fn execute_cue(&mut self, cue: Cue) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            execute(entity.into_world_mut(), actor, &cue, None, None);
        })
    }

    fn add_cue(&mut self, cue: Cue) -> &mut Self {
        self.queue(move |mut entity: EntityWorldMut| {
            entity
                .entry::<CueSources>()
                .or_default()
                .get_mut()
                .add(cue.tag, cue.local);
        })
    }

    fn remove_cue(&mut self, cue: Cue) -> &mut Self {
        self.queue(move |mut entity: EntityWorldMut| {
            if let Some(mut sources) = entity.get_mut::<CueSources>() {
                sources.remove(cue.tag, cue.local);
            }
        })
    }
}

/// Playing cues on actors on the world at once, not as commands.
pub trait CueWorld {
    /// Plays the one-shot `cue` on `actor`.
    fn execute_cue(&mut self, actor: Entity, cue: &Cue);

    /// Holds the lasting `cue` on `actor`, once more.
    fn add_cue(&mut self, actor: Entity, cue: &Cue);

    /// Holds the lasting `cue` on `actor` once less.
    fn remove_cue(&mut self, actor: Entity, cue: &Cue);
}

impl CueWorld for World {
    fn execute_cue(&mut self, actor: Entity, cue: &Cue) {
        execute(self, actor, cue, None, None);
    }

    fn add_cue(&mut self, actor: Entity, cue: &Cue) {
        add(self, actor, cue.tag, cue.local);
    }

    fn remove_cue(&mut self, actor: Entity, cue: &Cue) {
        remove(self, actor, cue.tag, cue.local);
    }
}

/// One more source of the lasting `tag` on `actor`.
pub(crate) fn add(world: &mut World, actor: Entity, tag: TagKey, local: bool) {
    if let Ok(mut entity) = world.get_entity_mut(actor) {
        entity
            .entry::<CueSources>()
            .or_default()
            .get_mut()
            .add(tag, local);
    }
}

/// One less source of the lasting `tag` on `actor`.
pub(crate) fn remove(world: &mut World, actor: Entity, tag: TagKey, local: bool) {
    if let Some(mut sources) = world.get_mut::<CueSources>(actor) {
        sources.remove(tag, local);
    }
}

/// Plays the one-shot `cue` on `actor`, under `activation` and from `effect`
/// if so: told here; on a server, kept for its clients unless it's local; on
/// a client, under its own activation, noted so the server's isn't played
/// again.
pub(crate) fn execute(
    world: &mut World,
    actor: Entity,
    cue: &Cue,
    activation: Option<u64>,
    effect: Option<EffectId>,
) {
    if world.get_entity(actor).is_err() {
        return;
    }
    let role = world.get_resource::<NetRole>().copied().unwrap_or_default();
    if !cue.local {
        match (role, activation) {
            (NetRole::Server, _) => burst(world, actor, cue, activation, effect),
            (NetRole::Client, Some(activation)) => {
                let at = world
                    .get_resource::<Time>()
                    .map_or(0.0, Time::elapsed_secs_f64);
                world
                    .get_resource_or_init::<Predicted>()
                    .note(actor, activation, cue.tag, at);
            }
            _ => {}
        }
    }
    tell_executed(
        world,
        CueExecuted {
            entity: actor,
            tag: cue.tag,
            magnitude: cue.magnitude,
            changes: cue.changes.clone(),
            level: cue.level,
            context: cue.context.clone(),
            activation,
            effect,
        },
    );
}

/// Tells `executed`, and runs its handler.
fn tell_executed(world: &mut World, executed: CueExecuted) {
    world.trigger(executed.clone());
    route(world, CueEvent::Executed(executed));
}

/// Keeps `cue` on `actor` for the server's clients, and lets go of those
/// older than a second.
fn burst(
    world: &mut World,
    actor: Entity,
    cue: &Cue,
    activation: Option<u64>,
    effect: Option<EffectId>,
) {
    let Some(tag) = world
        .get_resource::<TagRegistry>()
        .and_then(|tags| tags.id(cue.tag))
    else {
        warn!("cue `{}` isn't in the tag tree: not replicated", cue.tag);
        return;
    };
    let data = world
        .get_resource::<ContextDataRegistry>()
        .cloned()
        .unwrap_or_default();
    let at = world
        .get_resource::<Time>()
        .map_or(0.0, Time::elapsed_secs_f64);
    let Ok(mut entity) = world.get_entity_mut(actor) else {
        return;
    };
    let mut bursts = entity.entry::<CueBursts>().or_default();
    let mut bursts = bursts.get_mut();
    bursts.recent.retain(|burst| at - burst.at <= KEPT);
    let seq = bursts.next;
    bursts.next += 1;
    bursts.recent.push(Burst {
        seq,
        at,
        tag,
        magnitude: cue.magnitude,
        changes: cue.changes.clone(),
        level: cue.level,
        context: ContextReplica::of(&cue.context, &data),
        activation,
        effect,
    });
}

/// Actors whose lasting cues may have changed.
type Restated = Or<(Changed<CueSources>, Changed<ServedCues>)>;

/// An actor as its lasting cues are shown.
type Showing = (
    Entity,
    Option<&'static CueSources>,
    Option<&'static mut ServedCues>,
    Option<&'static ActiveCues>,
    SpawnDetails,
);

/// Shows each actor's lasting cues: those held here and those served, as
/// entities, started and ended as they come and go; on a server, served to
/// its clients.
pub(crate) fn show(
    mut actors: Query<Showing, Restated>,
    cues: Query<&ActiveCue>,
    role: Res<NetRole>,
    tree: Option<Res<TagRegistry>>,
    handlers: Res<CueHandlers>,
    mut commands: Commands,
) {
    let tree = tree.as_deref().cloned().unwrap_or_default();
    let serving = *role == NetRole::Server;
    for (actor, sources, mut served, shown, spawned) in &mut actors {
        // A client just come to see the actor joins the cues it's served.
        let joining = spawned.is_spawned() && !serving;
        // Served at once, in place: replicated the same frame.
        if serving && let (Some(sources), Some(served)) = (sources, served.as_mut()) {
            let now: BTreeSet<TagId> = sources
                .served()
                .into_iter()
                .filter_map(|tag| {
                    let id = tree.id(tag);
                    if id.is_none() {
                        warn!("cue `{tag}` isn't in the tag tree: not served");
                    }
                    id
                })
                .collect();
            if served.0 != now {
                served.0 = now;
            }
        }
        let served: BTreeSet<TagKey> = served
            .as_deref()
            .into_iter()
            .flat_map(|served| &served.0)
            .filter_map(|&id| tree.key(id))
            .collect();
        let mut wanted: BTreeSet<TagKey> = sources.into_iter().flat_map(CueSources::iter).collect();
        wanted.extend(served.iter().copied());
        let shown: Vec<(Entity, TagKey)> = shown
            .into_iter()
            .flat_map(ActiveCues::iter)
            .filter_map(|cue| Some((cue, cues.get(cue).ok()?.tag)))
            .collect();
        for &(cue, tag) in &shown {
            if wanted.contains(&tag) {
                continue;
            }
            let removed = CueRemoved {
                entity: actor,
                cue,
                tag,
            };
            commands.trigger(removed.clone());
            run(&mut commands, &handlers, CueEvent::Removed(removed));
            commands.entity(cue).despawn();
        }
        for tag in wanted {
            if shown.iter().any(|(_, shown)| *shown == tag) {
                continue;
            }
            let cue = commands.spawn((ActiveCue { tag }, CueOf(actor))).id();
            let added = CueAdded {
                entity: actor,
                cue,
                joined: joining && served.contains(&tag),
                tag,
            };
            commands.trigger(added.clone());
            run(&mut commands, &handlers, CueEvent::Added(added));
        }
    }
}

/// On a client: plays the one-shots the server played on each actor that it
/// hasn't, but those it predicted itself. Coming to see an actor, it plays
/// what's in its buffer: the server keeps only what's just happened.
pub(crate) fn play(
    mut actors: Query<(Entity, &CueBursts, Option<&mut Played>), Changed<CueBursts>>,
    mut predicted: ResMut<Predicted>,
    tree: Option<Res<TagRegistry>>,
    data: Option<Res<ContextDataRegistry>>,
    handlers: Res<CueHandlers>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let tree = tree.as_deref().cloned().unwrap_or_default();
    let data = data.as_deref().cloned().unwrap_or_default();
    predicted.forget(time.elapsed_secs_f64() - REMEMBERED);
    for (actor, bursts, played) in &mut actors {
        let from = if let Some(mut played) = played {
            std::mem::replace(&mut played.0, bursts.next)
        } else {
            commands.entity(actor).insert(Played(bursts.next));
            0
        };
        for burst in bursts.recent.iter().filter(|burst| burst.seq >= from) {
            let Some(tag) = tree.key(burst.tag) else {
                warn!("cue {:?} isn't in the tag tree: not played", burst.tag);
                continue;
            };
            if let Some(activation) = burst.activation
                && predicted.take(actor, activation, tag)
            {
                continue;
            }
            let executed = CueExecuted {
                entity: actor,
                tag,
                magnitude: burst.magnitude,
                changes: burst.changes.clone(),
                level: burst.level,
                context: burst.context.context(&data),
                activation: burst.activation,
                effect: burst.effect,
            };
            commands.trigger(executed.clone());
            run(&mut commands, &handlers, CueEvent::Executed(executed));
        }
    }
}

/// On a server: lets go of the one-shots older than it keeps them for, so an
/// actor's buffer holds only what's just happened.
pub(crate) fn prune(mut actors: Query<&mut CueBursts>, time: Res<Time>) {
    let now = time.elapsed_secs_f64();
    for mut bursts in &mut actors {
        if bursts.recent.iter().any(|burst| now - burst.at > KEPT) {
            bursts.recent.retain(|burst| now - burst.at <= KEPT);
        }
    }
}

/// The most specific handler for `tag`, if there's one.
fn handler(handlers: &CueHandlers, tag: TagKey) -> Option<SystemId<In<CueEvent>>> {
    tag.lineage().find_map(|tag| handlers.0.get(&tag).copied())
}

/// Runs the most specific handler for `event`'s tag, if there's one, as
/// commands are applied.
fn run(commands: &mut Commands, handlers: &CueHandlers, event: CueEvent) {
    if let Some(handler) = handler(handlers, event.tag()) {
        commands.run_system_with(handler, event);
    }
}

/// Runs the most specific handler for `event`'s tag, if there's one, at once.
fn route(world: &mut World, event: CueEvent) {
    let Some(handler) = world
        .get_resource::<CueHandlers>()
        .and_then(|handlers| handler(handlers, event.tag()))
    else {
        return;
    };
    if let Err(error) = world.run_system_with(handler, event) {
        warn!("a cue handler failed to run: {error}");
    }
}
