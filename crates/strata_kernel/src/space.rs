//! A standable surface: a hex column plus a level (design §7.1).

use serde::{Deserialize, Serialize};

use crate::StableHash;
use crate::hex::Hex;

/// A hex column `(q, r)` at integer level `z`, where one level is one tile thickness.
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
pub struct Space {
    /// Axial column coordinate.
    pub q: i32,
    /// Axial row coordinate.
    pub r: i32,
    /// Level of the surface, at least 0.
    pub z: u32,
}

impl Space {
    /// Creates the space at column `(q, r)` and level `z`.
    pub const fn new(q: i32, r: i32, z: u32) -> Self {
        Self { q, r, z }
    }

    /// Creates the space at `hex` and level `z`.
    pub const fn at(hex: Hex, z: u32) -> Self {
        Self::new(hex.q, hex.r, z)
    }

    /// Returns the column this space sits in.
    pub const fn hex(self) -> Hex {
        Hex::new(self.q, self.r)
    }
}
