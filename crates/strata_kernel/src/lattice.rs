//! The integer lattice that line-of-sight geometry runs on (design §7.5, §7.6).

use crate::hex::Hex;

/// Lattice units per axial unit.
///
/// Hex centers are whole axial units. Hex corners are centroids of three
/// mutually adjacent centers, so they sit at thirds. The 80% inset hexes of
/// §7.6 put their corners at 4/5 of a third, which is 4/15. The least common
/// multiple of 3 and 15 is 15, so at this scale centers, corners and inset
/// corners are all integer points.
pub const LATTICE: i64 = 15;

/// Numerator of the inset factor applied to figure volumes.
pub const INSET_NUM: i64 = 4;

/// Denominator of the inset factor applied to figure volumes.
pub const INSET_DEN: i64 = 5;

/// A point in axial coordinates scaled by [`LATTICE`].
///
/// The axial-to-plane map is affine, so straight segments between lattice
/// points are straight in both spaces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    /// Scaled axial `q` coordinate.
    pub q: i64,
    /// Scaled axial `r` coordinate.
    pub r: i64,
}

impl Point {
    /// Creates the lattice point `(q, r)`.
    pub const fn new(q: i64, r: i64) -> Point {
        Point { q, r }
    }

    /// Returns the center of `hex`.
    pub const fn center(hex: Hex) -> Point {
        Point::new(hex.q as i64 * LATTICE, hex.r as i64 * LATTICE)
    }

    /// Returns the offset from `other` to this point.
    pub const fn sub(self, other: Point) -> Point {
        Point::new(self.q - other.q, self.r - other.r)
    }

    /// Returns twice the Euclidean dot product of `self` with a neighbor offset.
    ///
    /// A point relative to a hex center is on the near side of that neighbor's
    /// bisector exactly when this value is below the hex limit. Doubling keeps
    /// the cross term integral.
    pub const fn twice_dot(self, direction: Hex) -> i64 {
        let (dq, dr) = (direction.q as i64, direction.r as i64);
        2 * self.q * dq + self.q * dr + self.r * dq + 2 * self.r * dr
    }
}
