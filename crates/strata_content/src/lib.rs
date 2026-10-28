//! Pack resolution, load-stage orchestration, freezing and the content registry.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

mod discover;
mod error;
mod manifest;
mod resolve;

pub use discover::discover_packs;
pub use error::PackError;
pub use manifest::{LoadedPack, PackManifest, Visibility, load_manifest};
pub use resolve::resolve_load_order;

/// The engine version that pack `engine` ranges are checked against.
pub const ENGINE_VERSION: &str = "0.1.0";
