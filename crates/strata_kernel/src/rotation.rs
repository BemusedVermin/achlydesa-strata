//! Rotation in 60-degree steps.

use serde::{Deserialize, Serialize};

/// A rotation of `k` × 60° clockwise, with `k` in `0..6`.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(try_from = "u8", into = "u8")]
pub struct Rotation(u8);

impl Rotation {
    /// Number of distinct rotations in a full turn.
    pub const COUNT: u8 = 6;

    /// Returns the rotation of `steps` × 60°, or `None` if `steps` is not in `0..6`.
    pub const fn new(steps: u8) -> Option<Self> {
        if steps < Self::COUNT {
            Some(Self(steps))
        } else {
            None
        }
    }

    /// Returns the rotation equal to `steps` × 60°, for any signed step count.
    pub const fn from_steps(steps: i32) -> Self {
        Self(steps.rem_euclid(Self::COUNT as i32) as u8)
    }

    /// Returns the number of 60° steps, in `0..6`.
    pub const fn steps(self) -> u8 {
        self.0
    }

    /// Returns the rotation that undoes this one.
    pub const fn inverse(self) -> Self {
        Self((Self::COUNT - self.0) % Self::COUNT)
    }

    /// Returns this rotation followed by `other`.
    pub const fn then(self, other: Self) -> Self {
        Self((self.0 + other.0) % Self::COUNT)
    }
}

impl TryFrom<u8> for Rotation {
    type Error = String;

    fn try_from(steps: u8) -> Result<Self, Self::Error> {
        Self::new(steps).ok_or_else(|| format!("rotation {steps} is not in 0..6"))
    }
}

impl From<Rotation> for u8 {
    fn from(rotation: Rotation) -> u8 {
        rotation.0
    }
}
