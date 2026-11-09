//! The placement tool behind the editor UI: the selected item, brush, rotation and level mode,
//! and the drag in progress (design §14.2).
//!
//! The UI forwards the cursor's hex and button presses here and applies the edit that comes
//! back to the [`MapDocument`], so every change it makes can be undone.

use strata_kernel::{Hex, Rotation};

use crate::brush::{Brush, fill, line};
use crate::defs::EditorDefs;
use crate::document::MapDocument;
use crate::palette::PaletteItem;
use crate::place::{LevelMode, Preview, Site};

/// The editor's placement tool.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tool {
    /// What a click places, if anything is selected.
    pub item: Option<PaletteItem>,
    /// The freeform brush.
    pub brush: Brush,
    /// The rotation of the next placement.
    pub rot: Rotation,
    /// How the next placement picks its level.
    pub level: LevelMode,
    /// Where the line being dragged started.
    drag: Option<Hex>,
}

impl Tool {
    /// Turns the next placement by `steps` 60-degree steps; negative turns the other way.
    pub fn rotate(&mut self, steps: i32) {
        self.rot = self.rot.then(Rotation::from_steps(steps));
    }

    /// Starts a stroke with the button going down at `at`.
    pub fn press(&mut self, at: Hex) {
        self.drag = Some(at);
    }

    /// Abandons the stroke in progress.
    pub fn cancel(&mut self) {
        self.drag = None;
    }

    /// Where the stroke in progress started.
    pub fn drag_start(&self) -> Option<Hex> {
        self.drag
    }

    /// What releasing the button with the cursor at `cursor` would place.
    pub fn preview<D: EditorDefs>(&self, document: &MapDocument<D>, cursor: Hex) -> Preview {
        let Some(item) = &self.item else {
            return Preview::default();
        };
        let site = Site::new(document.map(), document.defs());
        match self.brush {
            Brush::Single => site.place(item, cursor, self.rot, self.level),
            Brush::Line => {
                let cells = line(self.drag.unwrap_or(cursor), cursor);
                site.stroke(item, &cells, self.level)
            }
            Brush::Fill => site.stroke(item, &fill(&site.occupancy, cursor), self.level),
        }
    }

    /// Ends the stroke at `cursor` and returns what it places. The caller applies
    /// [`Preview::edit`] to `document`; the preview's problems explain anything left out.
    pub fn release<D: EditorDefs>(&mut self, document: &MapDocument<D>, cursor: Hex) -> Preview {
        let preview = self.preview(document, cursor);
        self.drag = None;
        preview
    }
}
