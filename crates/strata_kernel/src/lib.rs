//! Hex math, exact line-of-sight geometry, PCG32, hashing and typed IDs.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

pub mod hex;
pub mod rotation;
pub mod shape;
pub mod space;

pub use hex::{Axis, DIRECTIONS, Hex};
pub use rotation::Rotation;
pub use shape::Shape;
pub use space::Space;
