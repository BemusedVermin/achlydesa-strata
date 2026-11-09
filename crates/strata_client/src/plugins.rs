//! The client-shell plugins of design §13.3, some still empty skeletons filled in by their own
//! tasks.
//!
//! The rendering plugins live in `strata_render`.

use bevy::prelude::*;
use strata_render::RenderPlugins;

use crate::dev::DevPlugin;
use crate::input::DecisionInputPlugin;
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
}

/// Adds the rendering plugins and every shell plugin.
pub struct ClientPlugins;

impl Plugin for ClientPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            RenderPlugins,
            UiPlugin,
            DecisionInputPlugin,
            EditorPlugin,
            AssetsPlugin,
            DevPlugin,
        ));
    }
}
