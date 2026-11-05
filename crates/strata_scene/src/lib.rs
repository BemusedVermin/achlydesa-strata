//! Engine-independent scene math: world mapping, the isometric view and ray picking.
//!
//! Nothing here may import `bevy`; the types are `glam`'s, which Bevy re-exports unchanged.
//! This is presentation code, not a core crate, so floating point is allowed.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub use self::layout::{LEVEL_HEIGHT, Orientation, surface_point};
pub use self::pick::{SceneBoard, pick};
pub use self::terrain::{ChunkKey, MeshData, chunk_meshes, chunk_of, chunks};
pub use self::view::IsoView;

mod layout;
mod pick;
mod terrain;
#[cfg(test)]
mod tests;
mod view;
