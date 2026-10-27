//! Named footprint shapes, all anchored at the origin.

use crate::hex::{DIRECTIONS, Hex};
use crate::shape::Shape;

/// A single cell.
pub fn one() -> Shape {
    Shape::new([Hex::ORIGIN])
}

/// Two adjacent cells: the anchor and its east neighbor.
pub fn two_line() -> Shape {
    Shape::new([Hex::ORIGIN, DIRECTIONS[0]])
}

/// Three cells in a straight line running east from the anchor.
pub fn three_line() -> Shape {
    Shape::new([Hex::ORIGIN, DIRECTIONS[0], DIRECTIONS[0].scale(2)])
}

/// Three mutually adjacent cells: the anchor, its east and its south-east neighbors.
pub fn triangle() -> Shape {
    Shape::new([Hex::ORIGIN, DIRECTIONS[0], DIRECTIONS[1]])
}

/// Seven cells: the anchor and all six of its neighbors.
pub fn flower() -> Shape {
    Shape::new(Hex::ORIGIN.spiral(1))
}
