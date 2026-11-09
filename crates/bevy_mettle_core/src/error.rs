//! What can be wrong with definitions, or a name looked up.

use std::fmt;

use crate::IdTarget;

/// What can be wrong with data: a tag tree, an effect, a modifier.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MettleError {
    /// Two definitions of one kind share an id.
    #[error("there are two {kind}s called `{id}`")]
    Duplicate {
        /// What kind of thing: `tag`, `item`.
        kind: &'static str,
        /// The id they share.
        id: String,
    },
    /// A name that isn't defined.
    #[error("there's no {kind} called `{id}`{}", context(.within.as_deref()))]
    Unknown {
        /// What kind of thing: `tag`, `item`.
        kind: &'static str,
        /// The name.
        id: String,
        /// Where it was named, if in a definition: its id.
        within: Option<String>,
    },
    /// Something code names by type that content doesn't define: an effect
    /// or ability added with `add_effect` or `add_ability`.
    #[error("code names {kind} `{id}`, which content doesn't define")]
    NotInContent {
        /// What kind of thing: `effect`, `ability`.
        kind: &'static str,
        /// The name code gives it.
        id: String,
    },
    /// Something that can't be: a number that isn't finite, a `per` of 0 or
    /// less, a ramp from a value to itself.
    #[error("{what}{}", context(.within.as_deref()))]
    Invalid {
        /// What's wrong, in words.
        what: String,
        /// Where, if in a definition: its id.
        within: Option<String>,
    },
}

impl MettleError {
    /// That there's no `T` called `name`, named within `within` if in a
    /// definition.
    #[must_use]
    pub fn unknown<T: IdTarget>(name: &str, within: Option<&str>) -> Self {
        Self::Unknown {
            kind: T::NAME,
            id: name.to_owned(),
            within: within.map(ToOwned::to_owned),
        }
    }
}

fn context(within: Option<&str>) -> String {
    within
        .map(|stat| format!(" (in `{stat}`)"))
        .unwrap_or_default()
}

/// Everything wrong with definitions, found at once.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub struct MettleErrors(pub Vec<MettleError>);

impl fmt::Display for MettleErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}
