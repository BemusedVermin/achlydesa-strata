//! The Bevy plugins that draw and drive the 2.5D scene (design §13.3).
//!
//! The math behind them lives in `strata_scene`; this crate only wires it to Bevy.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod board;
mod camera;
mod input;
mod marker;
mod plugins;

pub use board::{
    BoardPlugin, DirtyChunks, FeatureNames, LevelSlice, RemovedFeatures, TerrainChunk,
};
pub use camera::{CameraPlugin, ClientBoard, HoveredSpace, IsoCamera, IsoCameraMarker};
pub use input::{BoardHighlights, InputPlugin, SpaceClicked};
pub use marker::{marker_material, marker_mesh, marker_transform};
pub use plugins::{FigurePlugin, OcclusionPlugin, PlaybackPlugin, PreviewPlugin, RenderPlugins};
