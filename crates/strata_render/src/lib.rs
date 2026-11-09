//! The Bevy plugins that draw and drive the 2.5D scene (design §13.3).
//!
//! The math behind them lives in `strata_scene`; this crate only wires it to Bevy.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod board;
mod camera;
mod figure;
mod input;
mod plugins;

pub use board::{
    BoardPlugin, DirtyChunks, FeatureNames, LevelSlice, RemovedFeatures, TerrainChunk,
};
pub use camera::{CameraPlugin, ClientBoard, HoveredSpace, IsoCamera, IsoCameraMarker};
pub use figure::{
    FigureAnimation, FigureBase, FigurePlugin, FigurePose, FigurePoses, FigureSprite,
    FigureSprites, FigureView, LookKey,
};
pub use input::{BoardHighlights, InputPlugin, SpaceClicked};
pub use plugins::{OcclusionPlugin, PlaybackPlugin, PreviewPlugin, RenderPlugins};
