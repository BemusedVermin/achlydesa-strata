//! The Bevy plugins that draw and drive the 2.5D scene (design §13.3).
//!
//! The math behind them lives in `strata_scene`; this crate only wires it to Bevy.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod camera;
mod input;
mod plugins;

pub use camera::{CameraPlugin, ClientBoard, HoveredSpace, IsoCamera, IsoCameraMarker};
pub use input::{BoardHighlights, InputPlugin, SpaceClicked};
pub use plugins::{
    BoardPlugin, FigurePlugin, OcclusionPlugin, PlaybackPlugin, PreviewPlugin, RenderPlugins,
};
