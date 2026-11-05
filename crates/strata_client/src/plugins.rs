//! Empty skeletons of the client-shell plugins in design §13.3. Each is filled in by its own task.
//!
//! The rendering plugins live in `strata_render`.

use bevy::prelude::*;
use strata_render::RenderPlugins;

use crate::input::CommandInputPlugin;
use crate::ui::UiPlugin;

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
    /// The map editor.
    EditorPlugin,
    /// Resolves visual keys to files, with fallbacks.
    AssetsPlugin,
    /// Rules inspector, error overlay and hot reload.
    DevPlugin,
}

/// Adds the rendering plugins and every skeleton plugin of the shell.
pub struct ClientPlugins;

impl Plugin for ClientPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            RenderPlugins,
            UiPlugin,
            CommandInputPlugin,
            EditorPlugin,
            AssetsPlugin,
            DevPlugin,
        ));
    }
}
