//! Reading attributes in context, summing them apart, and guards on them.

mod context;
mod guards;
mod reader;

pub use self::context::AttributeContext;
pub(crate) use self::context::Read;
pub use self::guards::{attribute_above, attribute_below};
#[doc(hidden)]
pub use self::reader::NumberedAttributes;
pub use self::reader::{AttributeReader, AttributeWorld};
pub use crate::registry::state::Breakdown;
