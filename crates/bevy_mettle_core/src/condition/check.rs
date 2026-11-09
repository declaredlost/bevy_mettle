//! Checking an expression against the tag tree.

use super::Condition;
use crate::MettleError;
use crate::tag::{TagKey, TagRegistry};

impl Condition {
    /// Checks every tag in it is defined in `tags`, and its numbers can be:
    /// named within `within` (what holds it) if it's in something.
    ///
    /// # Errors
    ///
    /// The first tag that isn't defined, or number that can't be.
    pub fn check(&self, tags: &TagRegistry, within: Option<&str>) -> Result<(), MettleError> {
        self.walk(Some(tags), within)
    }

    /// Checks its numbers can be, not its tags: what can be checked without
    /// the tag tree.
    ///
    /// # Errors
    ///
    /// The first number that can't be.
    pub fn check_values(&self, within: Option<&str>) -> Result<(), MettleError> {
        self.walk(None, within)
    }

    /// Checks its numbers, and its tags against `tags` if they're given.
    fn walk(&self, tags: Option<&TagRegistry>, within: Option<&str>) -> Result<(), MettleError> {
        let invalid = |what: String| MettleError::Invalid {
            what,
            within: within.map(ToOwned::to_owned),
        };
        let known = |tag: &TagKey| tags.map_or(Ok(()), |tags| tags.known(*tag, within));
        match self {
            Self::All(terms) | Self::Any(terms) => {
                terms.iter().try_for_each(|term| term.walk(tags, within))
            }
            Self::Not(term) => term.walk(tags, within),
            Self::ReadTagged(tag)
            | Self::OwnerHas(tag)
            | Self::TargetHas(tag)
            | Self::SourceHas(tag)
            | Self::FactSet(tag) => known(tag),
            Self::FactAbove { fact, value } | Self::FactBelow { fact, value } => {
                known(fact)?;
                if value.is_finite() {
                    Ok(())
                } else {
                    Err(invalid(format!("a comparison of `{fact}` with {value}")))
                }
            }
            Self::FactRamp { fact, full, none } => {
                known(fact)?;
                if full.is_finite() && none.is_finite() && (none - full).abs() > 0.0 {
                    Ok(())
                } else {
                    Err(invalid(format!(
                        "a ramp of `{fact}` from {full} to {none}: they have to differ"
                    )))
                }
            }
            Self::FactEvery { fact, n } => {
                known(fact)?;
                if *n > 0 {
                    Ok(())
                } else {
                    Err(invalid(format!("every 0 of `{fact}`")))
                }
            }
        }
    }
}
