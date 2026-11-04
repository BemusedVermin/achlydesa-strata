//! Engine-independent scene math: world mapping, the isometric view and ray picking.
//!
//! Nothing here may import `bevy`; the types are `glam`'s, which Bevy re-exports unchanged.

pub use self::pick::{SceneBoard, pick};
pub use self::view::IsoView;

mod layout;
mod pick;
#[cfg(test)]
mod tests;
mod view;
