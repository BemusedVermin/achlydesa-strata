//! Engine-independent scene math: world mapping, the isometric view, ray picking, and the
//! geometry and exact odds the previews draw.
//!
//! Nothing here may import `bevy`; the types are `glam`'s, which Bevy re-exports unchanged.
//! This is presentation code, not a core crate, so floating point is allowed.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub use self::layout::{LEVEL_HEIGHT, Orientation, surface_point};
pub use self::odds::{DicePool, WoundOdds, percent};
pub use self::outline::{LIFT, Line, lattice_point, ring, route, segment, volume_outline};
pub use self::pick::{SceneBoard, pick};
pub use self::terrain::{
    ChunkKey, Material, MeshData, MeshOptions, chunk_meshes, chunk_of, chunks, feature_chunks,
};
pub use self::view::IsoView;

mod layout;
mod odds;
mod outline;
mod pick;
mod terrain;
#[cfg(test)]
mod tests;
mod view;
