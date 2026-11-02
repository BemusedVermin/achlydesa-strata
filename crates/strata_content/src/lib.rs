//! Pack resolution, load-stage orchestration, freezing and the content registry.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

mod discover;
mod error;
mod identity;
mod intern;
mod load;
mod manifest;
mod registry;
mod resolve;
mod stages;

pub use discover::discover_packs;
pub use error::PackError;
pub use identity::{ContentHash, PackIdentity};
pub use load::load;
pub use manifest::{LoadedPack, PackManifest, Visibility, load_manifest};
pub use registry::ContentRegistry;
pub use resolve::resolve_load_order;
pub use stages::run_data_stages;

/// The engine version that pack `engine` ranges are checked against.
pub const ENGINE_VERSION: &str = "0.1.0";
