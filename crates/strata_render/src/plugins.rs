//! Empty skeletons of the rendering plugins in design §13.3. Each is filled in by its own task.

use bevy::prelude::*;

use crate::board::BoardPlugin;
use crate::camera::CameraPlugin;
use crate::figure::FigurePlugin;
use crate::input::InputPlugin;

macro_rules! skeleton {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
        $(#[$doc])*
        pub struct $name;

        impl Plugin for $name {
            fn build(&self, _app: &mut App) {}
        }
    )*};
}

skeleton! {
    /// Plays event batches in sequence.
    PlaybackPlugin,
    /// Movement, line-of-sight and combat previews.
    PreviewPlugin,
    /// Silhouettes of figures hidden behind terrain.
    OcclusionPlugin,
}

/// Adds every rendering plugin.
pub struct RenderPlugins;

impl Plugin for RenderPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            BoardPlugin,
            FigurePlugin,
            PlaybackPlugin,
            InputPlugin,
            CameraPlugin,
            PreviewPlugin,
            OcclusionPlugin,
        ));
    }
}
