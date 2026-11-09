//! Which footprint shapes the engine accepts (design §7.3).
//!
//! A footprint must cover its anchor cell `(0, 0)`, and its cells must form one connected shape
//! under hex adjacency. A `snake` footprint may have at most two cells: movement turns a snake by
//! a fixed rotation of its offsets, which only lets a two-cell body follow its leader.

use std::collections::BTreeSet;

use strata_kernel::Hex;

use super::{FootprintDef, MoveMode};

#[cfg(test)]
mod tests;

/// The most cells a `snake` footprint may have.
pub const MAX_SNAKE_CELLS: usize = 2;

/// Why a footprint cannot be used.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FootprintError {
    /// The footprint lists no cells.
    #[error("a footprint needs at least one cell, the anchor `{{ q = 0, r = 0 }}`")]
    Empty,
    /// The anchor cell `(0, 0)` is not one of the cells.
    #[error("a footprint must include its anchor cell `{{ q = 0, r = 0 }}`")]
    NoAnchor,
    /// Some cell cannot be reached from the anchor through adjacent cells.
    #[error(
        "the cells must be connected, but `{{ q = {}, r = {} }}` does not touch the rest of the shape",
        cell.q,
        cell.r
    )]
    Disconnected {
        /// The first cell, in offset order, not connected to the anchor.
        cell: Hex,
    },
    /// A `snake` footprint has more than two cells.
    #[error(
        "`move = \"snake\"` supports at most {MAX_SNAKE_CELLS} cells, but this footprint has \
         {cells}; use `move = \"rigid\"` instead"
    )]
    LongSnake {
        /// How many cells the footprint has.
        cells: usize,
    },
}

impl FootprintError {
    /// The field the error is about, as a freeze route.
    pub fn field(&self) -> &'static str {
        match self {
            FootprintError::LongSnake { .. } => "move",
            _ => "cells",
        }
    }
}

impl FootprintDef {
    /// Checks that the shape is usable: it is non-empty, covers the anchor, is connected, and is
    /// no longer than two cells if it moves as a `snake`.
    pub fn check(&self) -> Result<(), FootprintError> {
        let cells = self.cells.cells();
        if cells.is_empty() {
            return Err(FootprintError::Empty);
        }
        if !cells.contains(&Hex::ORIGIN) {
            return Err(FootprintError::NoAnchor);
        }
        let reached = connected(cells);
        if let Some(&cell) = cells.iter().find(|c| !reached.contains(c)) {
            return Err(FootprintError::Disconnected { cell });
        }
        if self.move_mode == MoveMode::Snake && cells.len() > MAX_SNAKE_CELLS {
            return Err(FootprintError::LongSnake { cells: cells.len() });
        }
        Ok(())
    }
}

/// The cells reachable from the anchor by steps between adjacent cells.
fn connected(cells: &[Hex]) -> BTreeSet<Hex> {
    let mut reached = BTreeSet::from([Hex::ORIGIN]);
    let mut frontier = vec![Hex::ORIGIN];
    while let Some(at) = frontier.pop() {
        for next in at.neighbors() {
            if cells.contains(&next) && reached.insert(next) {
                frontier.push(next);
            }
        }
    }
    reached
}
