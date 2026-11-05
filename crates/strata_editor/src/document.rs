//! The document being edited: a map, its compiled board and the undo history.

use std::sync::Arc;

use strata_model::MapDef;
use strata_model::board::{BoardDefs, BoardError, CompiledBoard, compile_all};

use crate::defs::EditorDefs;
use crate::edit::{Edit, EditError};
use crate::validate::{Validation, ValidationOptions, validate};

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
    /// The board, or every reason the map does not compile (never an empty list).
    board: Result<CompiledBoard, Vec<BoardError>>,
    /// The latest validation and the options it ran with, until the next change.
    validation: Option<(ValidationOptions, Arc<Validation>)>,
    undo: Vec<Applied>,
    redo: Vec<Applied>,
}

impl<D: BoardDefs> MapDocument<D> {
    /// Opens `map` for editing against `defs`.
    pub fn new(map: MapDef, defs: D) -> Self {
        let board = compile_all(&map, &defs);
        Self {
            map,
            defs,
            board,
            validation: None,
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
        self.board.as_ref().map_err(|errors| &errors[0])
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

    /// Recompiles the board if `edit` could have changed it, and drops the cached validation.
    fn refresh(&mut self, edit: &Edit) {
        if edit.affects_board() {
            self.board = compile_all(&self.map, &self.defs);
        }
        self.validation = None;
    }
}

impl<D: EditorDefs> MapDocument<D> {
    /// Validates the map as edited so far.
    ///
    /// The result is cached: until the next edit, undo or redo, asking again with the same
    /// options returns the same `Arc` without checking anything.
    pub fn validate(&mut self, options: &ValidationOptions) -> Arc<Validation> {
        if let Some((cached, validation)) = &self.validation
            && cached == options
        {
            return Arc::clone(validation);
        }
        let board = self.board.as_ref().map_err(Vec::as_slice);
        let validation = Arc::new(validate(&self.map, board, &self.defs, options));
        self.validation = Some((options.clone(), Arc::clone(&validation)));
        validation
    }
}
