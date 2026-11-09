//! `EditorPlugin`: the map editor's UI (design §14.2). `strata-client --edit <map.lua>` runs it
//! in place of a match.
//!
//! The palette, the placement ghost and the brushes are decided by `strata_editor`'s Bevy-free
//! [`Tool`]. The systems here turn the cursor and keys into [`EditorAction`]s, apply them, and
//! hand the resulting edits to the [`MapDocument`], so every change can be undone.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::EguiPrimaryContextPass;
use bevy_egui::input::EguiWantsInput;
use strata_editor::{
    Brush, LevelMode, MapDocument, OpenedMap, Palette, PaletteItem, Preview, RegistryDefs, Tool,
};
use strata_kernel::Hex;
use strata_render::{ClientBoard, FeatureNames, IsoCameraMarker};
use strata_scene::{Orientation, SceneBoard, pick, pick_level};

mod ghost;
pub mod panel;

/// The map being edited, and the palette of what can be placed on it.
#[derive(Resource)]
pub struct Editing {
    /// The map's full `pack:name` ID.
    pub id: String,
    /// The document every edit goes through.
    pub document: MapDocument<RegistryDefs>,
    /// The tiles and features of the loaded packs.
    pub palette: Palette,
}

impl Editing {
    /// Starts editing an opened map.
    pub fn new(opened: OpenedMap) -> Self {
        let palette = opened.palette();
        Self {
            id: opened.id.clone(),
            document: opened.into_document(),
            palette,
        }
    }

    /// How the map lays out its hexes.
    pub fn orientation(&self) -> Orientation {
        Orientation::from_name(self.document.map().orientation.as_str())
    }
}

/// The placement tool's state.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorTool(pub Tool);

/// The hex under the cursor: on the terrain the cursor shows, else on open ground at the locked
/// level or level 0.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorCursor(pub Option<Hex>);

/// What the tool would do at the cursor, and what the last action said.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct EditorFeedback {
    /// The ghost at the cursor.
    pub preview: Preview,
    /// The outcome of the last action that changed or refused to change the map.
    pub message: Option<String>,
}

/// Everything the editor UI can ask for. The panel, the mouse and the keys all send these.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum EditorAction {
    /// Selects what to place, or nothing.
    Select(Option<PaletteItem>),
    /// Picks the freeform brush.
    Brush(Brush),
    /// Sets how the ghost picks its level.
    Level(LevelMode),
    /// Turns the ghost by this many 60-degree steps.
    Rotate(i32),
    /// The button went down over this hex.
    Press(Hex),
    /// The button came up over this hex: place what the ghost shows.
    Release(Hex),
    /// Drops the stroke in progress.
    Cancel,
    /// Undoes the latest edit.
    Undo,
    /// Redoes the latest undone edit.
    Redo,
}

/// The map editor.
pub struct EditorPlugin {
    opened: std::sync::Mutex<Option<OpenedMap>>,
}

impl EditorPlugin {
    /// An editor for `opened`.
    pub fn new(opened: OpenedMap) -> Self {
        Self {
            opened: std::sync::Mutex::new(Some(opened)),
        }
    }
}

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        let opened = self.opened.lock().ok().and_then(|mut o| o.take());
        if let Some(opened) = opened {
            app.insert_resource(Editing::new(opened));
        }
        app.init_resource::<EditorTool>()
            .init_resource::<EditorCursor>()
            .init_resource::<EditorFeedback>()
            .init_resource::<FeatureNames>()
            .add_message::<EditorAction>()
            .add_systems(Startup, ghost::make_look)
            .add_systems(
                Update,
                (aim, (mouse, keys), act, show_board, preview, ghost::draw).chain(),
            )
            .add_systems(EguiPrimaryContextPass, panel::show);
    }
}

/// Keeps [`EditorCursor`] on the hex under the mouse.
pub(crate) fn aim(
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<IsoCameraMarker>>,
    editing: Res<Editing>,
    board: Option<Res<ClientBoard>>,
    tool: Res<EditorTool>,
    mut cursor: ResMut<EditorCursor>,
) {
    let hex = (|| {
        let at = window.cursor_position()?;
        let ray = camera.0.viewport_to_world(camera.1, at).ok()?;
        let (origin, direction) = (ray.origin, *ray.direction);
        let orientation = editing.orientation();
        match tool.0.level {
            LevelMode::Locked(z) => pick_level(orientation, origin, direction, z.into()),
            LevelMode::Snap => board
                .and_then(|b| pick(&b.0, origin, direction))
                .map(|space| space.hex())
                .or_else(|| pick_level(orientation, origin, direction, 0)),
        }
    })();
    cursor.set_if_neq(EditorCursor(hex));
}

/// Left button down starts a stroke and up ends it; the right button drops it. Clicks on a panel
/// are the panel's. A click pressed and released within one frame both starts and ends its
/// stroke.
pub(crate) fn mouse(
    buttons: Res<ButtonInput<MouseButton>>,
    egui: Option<Res<EguiWantsInput>>,
    cursor: Res<EditorCursor>,
    tool: Res<EditorTool>,
    mut actions: MessageWriter<EditorAction>,
) {
    let over_panel = egui.is_some_and(|e| e.is_pointer_over_area());
    if buttons.just_pressed(MouseButton::Right) {
        actions.write(EditorAction::Cancel);
    }
    let at = cursor.0.filter(|_| !over_panel);
    let pressed = buttons.just_pressed(MouseButton::Left) && at.is_some();
    if pressed && let Some(hex) = at {
        actions.write(EditorAction::Press(hex));
    }
    if buttons.just_released(MouseButton::Left) {
        match at {
            Some(hex) if pressed || tool.0.drag_start().is_some() => {
                actions.write(EditorAction::Release(hex));
            }
            Some(_) => {}
            None => {
                actions.write(EditorAction::Cancel);
            }
        }
    }
}

/// R and Shift+R turn the ghost, Ctrl+Z undoes, Ctrl+Y or Ctrl+Shift+Z redoes, and Escape
/// drops the stroke. Keys typed into a panel are the panel's.
pub(crate) fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    egui: Option<Res<EguiWantsInput>>,
    mut actions: MessageWriter<EditorAction>,
) {
    if egui.is_some_and(|e| e.wants_any_keyboard_input()) {
        return;
    }
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    if keys.just_pressed(KeyCode::KeyR) {
        actions.write(EditorAction::Rotate(if shift { -1 } else { 1 }));
    }
    if ctrl && keys.just_pressed(KeyCode::KeyZ) {
        actions.write(if shift {
            EditorAction::Redo
        } else {
            EditorAction::Undo
        });
    }
    if ctrl && keys.just_pressed(KeyCode::KeyY) {
        actions.write(EditorAction::Redo);
    }
    if keys.just_pressed(KeyCode::Escape) {
        actions.write(EditorAction::Cancel);
    }
}

/// Applies each action: tool settings change the tool, and everything that changes the map goes
/// through the document as an edit. [`Editing`] is only marked changed when the map changed, so
/// a refused placement or an undo with nothing to undo does not redraw the board.
pub(crate) fn act(
    mut actions: MessageReader<EditorAction>,
    mut editing: ResMut<Editing>,
    mut editor_tool: ResMut<EditorTool>,
    mut feedback: ResMut<EditorFeedback>,
) {
    for action in actions.read() {
        let tool = &mut editor_tool.0;
        let message = match action {
            EditorAction::Select(item) => {
                tool.item.clone_from(item);
                None
            }
            EditorAction::Brush(brush) => {
                tool.brush = *brush;
                None
            }
            EditorAction::Level(level) => {
                tool.level = *level;
                None
            }
            EditorAction::Rotate(steps) => {
                tool.rotate(*steps);
                None
            }
            EditorAction::Press(hex) => {
                tool.press(*hex);
                None
            }
            EditorAction::Cancel => {
                tool.cancel();
                None
            }
            EditorAction::Release(hex) => {
                let placed = tool.release(&editing.document, *hex);
                match placed.edit() {
                    Some(edit) => {
                        let applied = editing.bypass_change_detection().document.apply(edit);
                        match applied {
                            Ok(()) => {
                                editing.set_changed();
                                panel::warning(&placed)
                            }
                            Err(error) => Some(error.to_string()),
                        }
                    }
                    None => panel::refusal(&placed),
                }
            }
            EditorAction::Undo => history(&mut editing, MapDocument::undo, "Nothing to undo."),
            EditorAction::Redo => history(&mut editing, MapDocument::redo, "Nothing to redo."),
        };
        if message.is_some() || matches!(action, EditorAction::Release(_)) {
            feedback.message = message;
        }
    }
}

/// Undoes or redoes with `step`, marking [`Editing`] changed only when the map changed, and
/// returns what to say: nothing when it worked.
fn history(
    editing: &mut ResMut<Editing>,
    step: fn(&mut MapDocument<RegistryDefs>) -> Result<bool, strata_editor::EditError>,
    nothing: &str,
) -> Option<String> {
    match step(&mut editing.bypass_change_detection().document) {
        Ok(true) => {
            editing.set_changed();
            None
        }
        Ok(false) => Some(nothing.to_owned()),
        Err(error) => Some(error.to_string()),
    }
}

/// Shows the edited map's board whenever the map changes. While the map does not compile, the
/// last board that did stays on screen with the feature names it was drawn with, and the panel
/// says why.
pub(crate) fn show_board(
    mut commands: Commands,
    editing: Res<Editing>,
    mut names: ResMut<FeatureNames>,
) {
    if !editing.is_changed() {
        return;
    }
    if let Ok(board) = editing.document.board() {
        let map = editing.document.map();
        names.0 = map.features.iter().map(|f| f.feature.clone()).collect();
        commands.insert_resource(ClientBoard(SceneBoard {
            board: board.clone(),
            orientation: editing.orientation(),
        }));
    }
}

/// Keeps the ghost current with the cursor, the tool and the map.
pub(crate) fn preview(
    editing: Res<Editing>,
    tool: Res<EditorTool>,
    cursor: Res<EditorCursor>,
    mut feedback: ResMut<EditorFeedback>,
) {
    if !(editing.is_changed() || tool.is_changed() || cursor.is_changed()) {
        return;
    }
    let preview = cursor
        .0
        .map(|hex| tool.0.preview(&editing.document, hex))
        .unwrap_or_default();
    if feedback.preview != preview {
        feedback.preview = preview;
    }
}

#[cfg(test)]
mod tests;
