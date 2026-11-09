//! What can be wrong with content, or a name looked up: `Error`, and every
//! error found at once, `Errors`.

use std::fmt;

use crate::registry::Kind;

/// What can be wrong with content: a tag, an effect, a modifier.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    /// Two definitions of one kind share a name.
    #[error("there are two {kind}s called `{name}`")]
    Duplicate {
        /// What kind of thing: `tag`, `item`.
        kind: &'static str,
        /// The name they share.
        name: String,
    },
    /// A name that isn't defined.
    #[error("there's no {kind} called `{name}`{}", context(.within.as_deref()))]
    Unknown {
        /// What kind of thing: `tag`, `item`.
        kind: &'static str,
        /// The name.
        name: String,
        /// Where it was named, if in a definition: its name.
        within: Option<String>,
    },
    /// Something code names by type that content doesn't define: an effect
    /// or ability added with `add_effect` or `add_ability`.
    #[error("code names {kind} `{name}`, which content doesn't define")]
    NotInContent {
        /// What kind of thing: `effect`, `ability`.
        kind: &'static str,
        /// The name code gives it.
        name: String,
    },
    /// Something that can't be: a number that isn't finite, a `per` of 0 or
    /// less, a ramp from a value to itself.
    #[error("{what}{}", context(.within.as_deref()))]
    Invalid {
        /// What's wrong, in words.
        what: String,
        /// Where, if in a definition: its name.
        within: Option<String>,
    },
}

impl Error {
    /// That there's no `K` called `name`, named within `within` if in a
    /// definition.
    #[must_use]
    pub fn unknown<K: Kind>(name: &str, within: Option<&str>) -> Self {
        Self::Unknown {
            kind: K::NAME,
            name: name.to_owned(),
            within: within.map(ToOwned::to_owned),
        }
    }
}

fn context(within: Option<&str>) -> String {
    within
        .map(|stat| format!(" (in `{stat}`)"))
        .unwrap_or_default()
}

/// Everything wrong with content, found at once: each [`Error`] checking found.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub struct Errors(pub Vec<Error>);

impl fmt::Display for Errors {
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
