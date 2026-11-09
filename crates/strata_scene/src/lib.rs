//! Engine-independent scene math: world mapping, the isometric view, ray picking, terrain meshes
//! and figure sprites.
//!
//! Nothing here may import `bevy`; the types are `glam`'s, which Bevy re-exports unchanged.
//! This is presentation code, not a core crate, so floating point is allowed.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub use self::layout::{HEX_WIDTH, LEVEL_HEIGHT, Orientation, surface_point};
pub use self::pick::{SceneBoard, pick};
pub use self::sprite::{
    ANIMATIONS, ASSETS_DIR, Animation, AssetError, FallbackSprite, IDLE, INSET, MAX_FPS,
    MAX_FRAME_PX, MAX_FRAMES, MAX_HEX_WIDTH_PX, MAX_PIXELS_PER_LEVEL, MAX_SHEET_PX, PixelScale,
    Pose, Rgb, SpriteCatalog, SpriteDef, SpriteEntry, billboard_rotation, direction, facing,
    fallback_sprite, foot_point, parse_color, quad_corners, seat_color,
};
pub use self::terrain::{
    ChunkKey, Material, MeshData, MeshOptions, chunk_meshes, chunk_of, chunks, feature_chunks,
};
pub use self::view::IsoView;

mod layout;
mod pick;
mod sprite;
mod terrain;
#[cfg(test)]
mod tests;
mod view;
