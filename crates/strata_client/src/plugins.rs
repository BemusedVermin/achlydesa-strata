//! Empty skeletons of the plugins in design §13.3. Each is filled in by its own task.

use bevy::prelude::*;

use crate::camera::CameraPlugin;

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
    /// Builds terrain meshes from the compiled board.
    BoardPlugin,
    /// Spawns figure sprites as camera-facing billboards.
    FigurePlugin,
    /// Plays event batches in sequence.
    PlaybackPlugin,
    /// Turns clicks into commands.
    InputPlugin,
    /// The `bevy_egui` panels.
    UiPlugin,
    /// Movement, line-of-sight and combat previews.
    PreviewPlugin,
    /// The map editor.
    EditorPlugin,
    /// Resolves visual keys to files, with fallbacks.
    AssetsPlugin,
    /// Rules inspector, error overlay and hot reload.
    DevPlugin,
    /// Silhouettes of figures hidden behind terrain.
    OcclusionPlugin,
}

/// Adds every skeleton plugin.
pub struct ClientPlugins;

impl Plugin for ClientPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            BoardPlugin,
            FigurePlugin,
            PlaybackPlugin,
            InputPlugin,
            CameraPlugin,
            UiPlugin,
            PreviewPlugin,
            EditorPlugin,
            AssetsPlugin,
            DevPlugin,
            OcclusionPlugin,
        ));
    }
}
