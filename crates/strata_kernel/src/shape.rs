//! Footprint shapes: sets of axial offsets from an anchor (design §7.3).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::hex::Hex;
use crate::rotation::Rotation;

pub mod common;

#[cfg(test)]
mod tests;

/// A set of axial offsets from an anchor cell.
///
/// Offsets are kept sorted and unique, so equal shapes compare equal and the
/// cell count is the number of distinct cells.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(from = "Vec<Hex>", into = "Vec<Hex>")]
pub struct Shape {
    cells: Vec<Hex>,
}

impl Shape {
    /// Creates a shape from `cells`, sorting them and dropping duplicates.
    pub fn new(cells: impl IntoIterator<Item = Hex>) -> Self {
        let mut cells: Vec<Hex> = cells.into_iter().collect();
        cells.sort();
        cells.dedup();
        Self { cells }
    }

    /// Returns the offsets in ascending order.
    pub fn cells(&self) -> &[Hex] {
        &self.cells
    }

    /// Returns the number of cells.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Returns true if the shape has no cells.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Returns true if the shape contains the offset `cell`.
    pub fn contains(&self, cell: Hex) -> bool {
        self.cells.binary_search(&cell).is_ok()
    }

    /// Returns the shape rotated clockwise about its anchor, the origin.
    pub fn rotated(&self, rotation: Rotation) -> Shape {
        Shape::new(self.cells.iter().map(|c| c.rotate(rotation)))
    }

    /// Returns the shape with every cell moved by `by`.
    pub fn translated(&self, by: Hex) -> Shape {
        Shape::new(self.cells.iter().map(|c| c.add(by)))
    }

    /// Returns true if every cell can reach every other through adjacent cells.
    ///
    /// An empty shape is not connected: a footprint needs at least one cell.
    pub fn is_connected(&self) -> bool {
        let Some(&start) = self.cells.first() else {
            return false;
        };
        let mut seen = BTreeSet::from([start]);
        let mut frontier = vec![start];
        while let Some(cell) = frontier.pop() {
            for next in cell.neighbors() {
                if self.contains(next) && seen.insert(next) {
                    frontier.push(next);
                }
            }
        }
        seen.len() == self.cells.len()
    }
}

impl From<Vec<Hex>> for Shape {
    fn from(cells: Vec<Hex>) -> Self {
        Shape::new(cells)
    }
}

impl From<Shape> for Vec<Hex> {
    fn from(shape: Shape) -> Self {
        shape.cells
    }
}
