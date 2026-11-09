//! The editor's `bevy_egui` panel: the palette with remaining counts, the brush, rotation and
//! level controls, undo and redo, and what the ghost has to say. Every button sends an
//! [`EditorAction`]; the panel changes nothing itself.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use strata_editor::{Brush, FILL_LIMIT, LevelMode, PaletteEntry, PaletteItem, Preview, Problem};

use super::{Editing, EditorAction, EditorCursor, EditorFeedback, EditorTool};

/// A palette button's text: the name, its size, and how many are left when the map has an
/// inventory.
pub fn entry_text(entry: &PaletteEntry, remaining: Option<i64>) -> String {
    let size = match entry.item {
        PaletteItem::Tile(_) => format!("{} hex × {} lv", entry.cells, entry.thickness),
        PaletteItem::Feature(_) => format!("{} hex", entry.cells),
    };
    let name = if entry.label.0.is_empty() {
        entry.full_id.as_str()
    } else {
        entry.label.0.as_str()
    };
    match remaining {
        Some(left) => format!("{name} ({size}) · {left} left"),
        None => format!("{name} ({size})"),
    }
}

/// One line about a problem with the ghost.
pub fn problem_text(problem: &Problem) -> String {
    let at = |hexes: &[strata_kernel::Hex]| {
        let cells: Vec<String> = hexes
            .iter()
            .map(|h| format!("({}, {})", h.q, h.r))
            .collect();
        cells.join(" ")
    };
    match problem {
        Problem::UnknownItem => "The selected item is not defined by any loaded pack.".into(),
        Problem::NotOneHex => "Line and fill paint one-hex tiles only.".into(),
        Problem::Overlap(hexes) => format!("Overlaps terrain at {}.", at(hexes)),
        Problem::Unsupported(hexes) => {
            format!("The map requires support; nothing holds up {}.", at(hexes))
        }
        Problem::OffBoard(hexes) => format!("No terrain to stand on at {}.", at(hexes)),
        Problem::TooHigh(hexes) => format!("Too high to place at {}.", at(hexes)),
        Problem::OutOfStock { remaining } => format!(
            "Over the inventory: {remaining} left after this. \
             The map will fail validation (`strata map check`) until the inventory is fixed."
        ),
        Problem::NotEnclosed => {
            "Not an enclosed area: fill on open ground fills holes inside the terrain only.".into()
        }
        Problem::TooLarge => format!("The region is larger than {FILL_LIMIT} hexes."),
    }
}

/// What to say when a placement went ahead despite warnings, such as running out of stock.
pub fn warning(preview: &Preview) -> Option<String> {
    let lines: Vec<String> = preview.problems.iter().map(problem_text).collect();
    (!lines.is_empty()).then(|| format!("Placed. {}", lines.join(" ")))
}

/// The level the "Lock level" control shows: the locked level, or while snapping the level the
/// ghost snaps to under the cursor, so locking keeps the ghost where it is.
pub fn shown_level(mode: LevelMode, snapped: Option<u8>) -> u8 {
    match mode {
        LevelMode::Locked(level) => level,
        LevelMode::Snap => snapped.unwrap_or(0),
    }
}

/// What to say when releasing placed nothing.
pub fn refusal(preview: &Preview) -> Option<String> {
    let lines: Vec<String> = preview.problems.iter().map(problem_text).collect();
    (!lines.is_empty()).then(|| format!("Nothing placed. {}", lines.join(" ")))
}

/// Draws the panel and sends the actions its buttons ask for.
pub(super) fn show(
    mut contexts: EguiContexts,
    editing: Res<Editing>,
    tool: Res<EditorTool>,
    feedback: Res<EditorFeedback>,
    cursor: Res<EditorCursor>,
    mut actions: MessageWriter<EditorAction>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let tool = &tool.0;
    let occupancy = editing.document.occupancy();
    let snapped = cursor
        .0
        .and_then(|hex| LevelMode::Snap.level(occupancy, hex));
    let mut send = Vec::new();
    egui::Window::new("Map editor").show(ctx, |ui| {
        ui.heading(&editing.id);
        ui.horizontal(|ui| {
            let document = &editing.document;
            if ui
                .add_enabled(document.can_undo(), egui::Button::new("Undo"))
                .clicked()
            {
                send.push(EditorAction::Undo);
            }
            if ui
                .add_enabled(document.can_redo(), egui::Button::new("Redo"))
                .clicked()
            {
                send.push(EditorAction::Redo);
            }
        });
        controls(ui, tool, snapped, &mut send);
        ui.separator();
        palette(ui, &editing, tool, &mut send);
        ui.separator();
        for problem in &feedback.preview.problems {
            let color = if problem.blocks() {
                egui::Color32::LIGHT_RED
            } else {
                egui::Color32::YELLOW
            };
            ui.colored_label(color, problem_text(problem));
        }
        if let Some(message) = &feedback.message {
            ui.label(message);
        }
        if let Err(error) = editing.document.board() {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                format!("The map does not compile: {error}"),
            );
        }
    });
    actions.write_batch(send);
    Ok(())
}

/// The brush, rotation and level controls.
fn controls(
    ui: &mut egui::Ui,
    tool: &strata_editor::Tool,
    snapped: Option<u8>,
    send: &mut Vec<EditorAction>,
) {
    ui.horizontal(|ui| {
        for (brush, name) in [
            (Brush::Single, "Single"),
            (Brush::Line, "Line"),
            (Brush::Fill, "Fill"),
        ] {
            if ui.selectable_label(tool.brush == brush, name).clicked() {
                send.push(EditorAction::Brush(brush));
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label(format!("Rotation {}×60° (R)", tool.rot.steps()));
        if ui.button("⟲").clicked() {
            send.push(EditorAction::Rotate(-1));
        }
        if ui.button("⟳").clicked() {
            send.push(EditorAction::Rotate(1));
        }
    });
    ui.horizontal(|ui| {
        let mut locked = matches!(tool.level, LevelMode::Locked(_));
        let mut level = shown_level(tool.level, snapped);
        let toggled = ui.checkbox(&mut locked, "Lock level").changed();
        let moved = ui
            .add_enabled(locked, egui::DragValue::new(&mut level).range(0..=u8::MAX))
            .changed();
        if toggled || moved {
            let mode = if locked {
                LevelMode::Locked(level)
            } else {
                LevelMode::Snap
            };
            send.push(EditorAction::Level(mode));
        }
    });
}

/// The tiles, then the features, each a button that selects it or, when selected, clears it.
fn palette(
    ui: &mut egui::Ui,
    editing: &Editing,
    tool: &strata_editor::Tool,
    send: &mut Vec<EditorAction>,
) {
    let defs = editing.document.defs();
    let stock = editing.document.stock();
    egui::ScrollArea::vertical()
        .max_height(400.0)
        .show(ui, |ui| {
            let mut heading = None;
            for entry in &editing.palette.entries {
                let kind = match entry.item {
                    PaletteItem::Tile(_) => "Tiles",
                    PaletteItem::Feature(_) => "Features",
                };
                if heading != Some(kind) {
                    ui.strong(kind);
                    heading = Some(kind);
                }
                let remaining = stock.map(|s| s.remaining(&entry.item, defs));
                let selected = tool.item.as_ref() == Some(&entry.item);
                let text = entry_text(entry, remaining);
                if ui
                    .selectable_label(selected, text)
                    .on_hover_text(&entry.full_id)
                    .clicked()
                {
                    let item = (!selected).then(|| entry.item.clone());
                    send.push(EditorAction::Select(item));
                }
            }
        });
}
