//! Axial hex coordinates and the integer operations on them (design §7.1).

use serde::{Deserialize, Serialize};

use crate::StableHash;

use crate::rotation::Rotation;

/// The six neighbor offsets, in clockwise order starting east.
///
/// With `r` growing downward, the order is E, SE, SW, W, NW, NE. Rotating a
/// direction by one 60° step moves it to the next entry.
pub const DIRECTIONS: [Hex; 6] = [
    Hex::new(1, 0),
    Hex::new(0, 1),
    Hex::new(-1, 1),
    Hex::new(-1, 0),
    Hex::new(0, -1),
    Hex::new(1, -1),
];

/// A cube axis that a reflection keeps fixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Axis {
    /// Reflect across the line of constant `q`.
    Q,
    /// Reflect across the line of constant `r`.
    R,
    /// Reflect across the line of constant `s`.
    S,
}

/// One hex of the plane in axial coordinates; the cube coordinate is `s = -q - r`.
///
/// Arithmetic is plain `i32`, so coordinates are expected to stay well inside
/// the `i32` range (boards are small).
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    StableHash,
)]
pub struct Hex {
    /// Axial column coordinate.
    pub q: i32,
    /// Axial row coordinate.
    pub r: i32,
}

impl Hex {
    /// The hex at `(0, 0)`.
    pub const ORIGIN: Hex = Hex::new(0, 0);

    /// Creates the hex at `(q, r)`.
    pub const fn new(q: i32, r: i32) -> Self {
        Self { q, r }
    }

    /// Returns the implied third cube coordinate, `-q - r`.
    pub const fn s(self) -> i32 {
        -self.q - self.r
    }

    /// Returns this hex moved by `by`.
    pub const fn add(self, by: Hex) -> Hex {
        Hex::new(self.q + by.q, self.r + by.r)
    }

    /// Returns the offset from `other` to this hex.
    pub const fn sub(self, other: Hex) -> Hex {
        Hex::new(self.q - other.q, self.r - other.r)
    }

    /// Returns this hex's offset multiplied by `k`.
    pub const fn scale(self, k: i32) -> Hex {
        Hex::new(self.q * k, self.r * k)
    }

    /// Returns the neighbor in direction `index` of [`DIRECTIONS`], taken modulo 6.
    pub const fn neighbor(self, index: usize) -> Hex {
        self.add(DIRECTIONS[index % 6])
    }

    /// Returns the six neighbors in [`DIRECTIONS`] order.
    pub const fn neighbors(self) -> [Hex; 6] {
        [
            self.neighbor(0),
            self.neighbor(1),
            self.neighbor(2),
            self.neighbor(3),
            self.neighbor(4),
            self.neighbor(5),
        ]
    }

    /// Returns the number of steps between this hex and `other`.
    pub fn distance(self, other: Hex) -> u32 {
        let d = self.sub(other);
        (d.q.unsigned_abs() + d.r.unsigned_abs() + d.s().unsigned_abs()) / 2
    }

    /// Rotates this hex clockwise about the origin.
    pub const fn rotate(self, rotation: Rotation) -> Hex {
        let (mut q, mut r, mut s) = (self.q, self.r, self.s());
        let mut steps = rotation.steps();
        while steps > 0 {
            (q, r, s) = (-r, -s, -q);
            steps -= 1;
        }
        let _ = s;
        Hex::new(q, r)
    }

    /// Rotates this hex clockwise about `center`.
    pub const fn rotate_about(self, center: Hex, rotation: Rotation) -> Hex {
        center.add(self.sub(center).rotate(rotation))
    }

    /// Reflects this hex across the line through the origin that fixes `axis`.
    pub const fn reflect(self, axis: Axis) -> Hex {
        match axis {
            Axis::Q => Hex::new(self.q, self.s()),
            Axis::R => Hex::new(self.s(), self.r),
            Axis::S => Hex::new(self.r, self.q),
        }
    }

    /// Returns the hexes at exactly `radius` from this one.
    ///
    /// Radius 0 gives just this hex. Otherwise the ring starts at the
    /// north-west corner (direction 4 scaled by `radius`) and walks clockwise.
    pub fn ring(self, radius: u32) -> Vec<Hex> {
        if radius == 0 {
            return vec![self];
        }
        let radius = radius as i32;
        let mut cell = self.add(DIRECTIONS[4].scale(radius));
        let mut cells = Vec::with_capacity(6 * radius as usize);
        for direction in DIRECTIONS {
            for _ in 0..radius {
                cells.push(cell);
                cell = cell.add(direction);
            }
        }
        cells
    }

    /// Returns this hex followed by rings 1 through `radius`, in order.
    pub fn spiral(self, radius: u32) -> Vec<Hex> {
        (0..=radius).flat_map(|k| self.ring(k)).collect()
    }
}
