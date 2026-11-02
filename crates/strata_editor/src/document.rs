//! The document being edited: a map, its compiled board and the undo history.

use strata_model::MapDef;
use strata_model::board::{BoardDefs, BoardError, CompiledBoard, compile};

use crate::edit::{Edit, EditError};

/// An applied edit and the edit that undoes it.
#[derive(Clone, Debug)]
struct Applied {
    edit: Edit,
    inverse: Edit,
}

/// A map under edit, with a board recompiled after every change and unlimited undo and redo.
pub struct MapDocument<D: BoardDefs> {
    map: MapDef,
    defs: D,
    board: Result<CompiledBoard, BoardError>,
    undo: Vec<Applied>,
    redo: Vec<Applied>,
}

impl<D: BoardDefs> MapDocument<D> {
    /// Opens `map` for editing against `defs`.
    pub fn new(map: MapDef, defs: D) -> Self {
        let board = compile(&map, &defs);
        Self {
            map,
            defs,
            board,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Returns the map as edited so far.
    pub fn map(&self) -> &MapDef {
        &self.map
    }

    /// Returns the compiled board, or why the current map does not compile.
    ///
    /// A map mid-edit may overlap or name a missing tile, so this is a `Result` rather than a
    /// reason to reject the edit.
    pub fn board(&self) -> Result<&CompiledBoard, &BoardError> {
        self.board.as_ref()
    }

    /// Returns whether there is an edit to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Returns whether there is an undone edit to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Applies `edit`, discarding any redo history.
    pub fn apply(&mut self, edit: Edit) -> Result<(), EditError> {
        let inverse = edit.clone().apply(&mut self.map)?;
        self.refresh(&edit);
        self.undo.push(Applied { edit, inverse });
        self.redo.clear();
        Ok(())
    }

    /// Undoes the latest edit. Returns `false` if there was nothing to undo.
    pub fn undo(&mut self) -> Result<bool, EditError> {
        let Some(applied) = self.undo.pop() else {
            return Ok(false);
        };
        applied.inverse.clone().apply(&mut self.map)?;
        self.refresh(&applied.inverse);
        self.redo.push(applied);
        Ok(true)
    }

    /// Redoes the latest undone edit. Returns `false` if there was nothing to redo.
    pub fn redo(&mut self) -> Result<bool, EditError> {
        let Some(applied) = self.redo.pop() else {
            return Ok(false);
        };
        applied.edit.clone().apply(&mut self.map)?;
        self.refresh(&applied.edit);
        self.undo.push(applied);
        Ok(true)
    }

    /// Recompiles the board if `edit` could have changed it.
    fn refresh(&mut self, edit: &Edit) {
        if edit.affects_board() {
            self.board = compile(&self.map, &self.defs);
        }
    }
}
