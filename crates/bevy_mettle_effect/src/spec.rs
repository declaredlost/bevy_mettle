//! What's applied: an effect, at a level, with what code set, from whom.

use std::any::TypeId;

use bevy::prelude::*;
use bevy_mettle_core::tag::TagKey;

use crate::EffectRegistry;
use crate::registry::EffectId;

/// An effect code names: a marker type, its content's name on it.
///
/// Derive it, add it to the app, and apply it: the type is its spec, so
/// code names effects without a string, and a misspelled one doesn't compile.
///
/// ```ignore
/// #[derive(Effect)]
/// #[effect(name = "burning")]
/// pub struct Burning;
///
/// app.add_effect::<Burning>();
/// commands.entity(mage).apply_effect(Burning.level(3.0).param(DamageParam, -5.0));
/// ```
///
/// [`add_effect`](crate::code::EffectApp::add_effect) makes content
/// without it fail its check, so its name is checked once, as content loads.
/// Its builders make an [`EffectSpec`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't an effect",
    label = "not an effect",
    note = "derive it: `#[derive(Effect)]`"
)]
pub trait Effect: Send + Sync + 'static {
    /// The name content defines it by: `"burning"`.
    const NAME: &'static str;

    /// It, at level 1, from no one.
    #[must_use]
    fn spec(self) -> EffectSpec
    where
        Self: Sized,
    {
        EffectSpec::of::<Self>()
    }

    /// It, at `level`.
    #[must_use]
    fn level(self, level: f32) -> EffectSpec
    where
        Self: Sized,
    {
        self.spec().level(level)
    }

    /// It, with `param` (a tag: `param.damage`) at `value`.
    #[must_use]
    fn param(self, param: impl Into<TagKey>, value: f32) -> EffectSpec
    where
        Self: Sized,
    {
        self.spec().param(param, value)
    }

    /// It, from `source`.
    #[must_use]
    fn source(self, source: Entity) -> EffectSpec
    where
        Self: Sized,
    {
        self.spec().source(source)
    }
}

pub use bevy_mettle_macros::Effect;

impl<E: Effect> From<E> for EffectSpec {
    fn from(effect: E) -> Self {
        effect.spec()
    }
}

/// Which effect a spec applies: by type, as code names it, or by number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Type { type_id: TypeId, name: &'static str },
    Number(EffectId),
}

/// An effect about to be applied: which, at what level, with what, from whom.
///
/// Unreal's `GameplayEffectSpec`: with what the applying code set. Made from
/// an [`Effect`] type (`Burning.level(3.0)`), or from a number
/// ([`from_id`](Self::from_id)); its effect is resolved to its number as it's
/// applied. An active effect keeps the spec it was applied with.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectSpec {
    effect: Which,
    /// Its level: what curves are read at. 1 unless it says.
    pub level: f32,
    /// What the applying code set, for `Param` magnitudes.
    pub params: Vec<(TagKey, f32)>,
    /// Who applied it: the caster, the attacker.
    pub source: Option<Entity>,
    /// The activation it was applied under, if any: what a rejected prediction
    /// takes back. One the server's own has its top bit set; a client's is
    /// its own, told apart from another client's by the source.
    pub activation: Option<u64>,
}

impl EffectSpec {
    /// `E`, at level 1, from no one.
    #[must_use]
    pub fn of<E: Effect>() -> Self {
        Self::new(Which::Type {
            type_id: TypeId::of::<E>(),
            name: E::NAME,
        })
    }

    /// The effect numbered `effect`, at level 1, from no one: for one code
    /// doesn't name by type, found by its number.
    #[must_use]
    pub fn from_id(effect: EffectId) -> Self {
        Self::new(Which::Number(effect))
    }

    fn new(effect: Which) -> Self {
        Self {
            effect,
            level: 1.0,
            params: Vec::new(),
            source: None,
            activation: None,
        }
    }

    /// At `level`.
    #[must_use]
    pub fn level(mut self, level: f32) -> Self {
        self.level = level;
        self
    }

    /// With `param` (a tag: `param.damage`) at `value`. Setting one again
    /// replaces it.
    #[must_use]
    pub fn param(mut self, param: impl Into<TagKey>, value: f32) -> Self {
        let param = param.into();
        match self.params.iter_mut().find(|(known, _)| *known == param) {
            Some((_, known)) => *known = value,
            None => self.params.push((param, value)),
        }
        self
    }

    /// From `source`.
    #[must_use]
    pub fn source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// Under `activation`.
    #[must_use]
    pub fn activation(mut self, activation: u64) -> Self {
        self.activation = Some(activation);
        self
    }

    /// What `param` was set to, if it was.
    #[must_use]
    pub fn param_value(&self, param: impl Into<TagKey>) -> Option<f32> {
        let param = param.into();
        self.params
            .iter()
            .find(|(known, _)| *known == param)
            .map(|(_, value)| *value)
    }

    /// Its effect's number in `registry`: `None` if there's no such effect, or
    /// its type isn't added.
    #[must_use]
    pub fn effect_id(&self, registry: &EffectRegistry) -> Option<EffectId> {
        match self.effect {
            Which::Type { type_id, .. } => registry.id_of_type(type_id),
            Which::Number(id) => registry.get(id).map(|_| id),
        }
    }

    /// The same, for `effect`: what an effect it chains is applied with.
    pub(crate) fn for_effect(&self, effect: EffectId) -> Self {
        Self {
            effect: Which::Number(effect),
            ..self.clone()
        }
    }

    /// What its effect is called, for a warning: its type's name, or its
    /// number.
    pub(crate) fn label(&self) -> String {
        match self.effect {
            Which::Type { name, .. } => name.to_owned(),
            Which::Number(id) => format!("{id:?}"),
        }
    }
}
