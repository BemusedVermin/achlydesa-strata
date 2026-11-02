//! Map documents, edit commands, validation and the Lua writer.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

mod document;
mod edit;
mod writer;

pub use document::MapDocument;
pub use edit::{Edit, EditError, Metadata};
pub use writer::{WriteError, write_map};
