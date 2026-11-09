//! Map documents, edit commands, validation and the Lua writer.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

mod defs;
mod document;
mod edit;
mod fairness;
mod finding;
mod reach;
mod standing;
mod validate;
mod writer;

pub use defs::EditorDefs;
pub use document::MapDocument;
pub use edit::{Edit, EditError, Metadata};
pub use fairness::{Fairness, Objective};
pub use finding::{Finding, FindingKind, Severity};
pub use strata_content::RegistryDefs;
pub use validate::{START_ZONE_PREFIX, Validation, ValidationOptions};
pub use writer::{WriteError, write_map};
