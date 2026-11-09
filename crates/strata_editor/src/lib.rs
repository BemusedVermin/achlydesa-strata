//! Map documents, edit commands, validation, the Lua writer, and the placement tool the editor
//! UI drives.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

mod brush;
mod defs;
mod document;
mod edit;
mod fairness;
mod finding;
mod occupancy;
mod open;
mod palette;
mod place;
mod reach;
mod standing;
mod tool;
mod validate;
mod writer;

pub use brush::{Brush, FILL_LIMIT, fill, line};
pub use defs::EditorDefs;
pub use document::MapDocument;
pub use edit::{Edit, EditError, Metadata};
pub use fairness::{Fairness, Objective};
pub use finding::{Finding, FindingKind, Severity};
pub use occupancy::{Occupancy, Span};
pub use open::{OpenError, OpenedMap};
pub use palette::{Palette, PaletteEntry, PaletteItem, Stock};
pub use place::{LevelMode, Mark, Preview, Problem};
pub use strata_content::RegistryDefs;
pub use tool::Tool;
pub use validate::{START_ZONE_PREFIX, Validation, ValidationOptions};
pub use writer::{WriteError, write_map};
