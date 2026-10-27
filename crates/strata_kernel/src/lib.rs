//! Hex math, exact line-of-sight geometry, PCG32, hashing and typed IDs.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

extern crate self as strata_kernel;

pub mod frac;
pub mod hash;
pub mod hex;
pub mod id;
pub mod lattice;
pub mod occlusion;
pub mod pcg;
pub mod rng;
pub mod rotation;
pub mod shape;
pub mod space;
pub mod traverse;
pub mod value;

pub use frac::Frac;
pub use hash::{StableHash, StableHasher};
pub use hex::{Axis, DIRECTIONS, Hex};
pub use id::{DefId, ModuleId, PackId, SlotId, Sym, WindowId};
pub use lattice::{LATTICE, Point};
pub use occlusion::{
    Blocker, BlockerFootprint, ClearCount, Point3, SUB_LEVELS, Seg3, blocked, clear_count,
};
pub use pcg::Pcg32;
pub use rng::CommandRng;
pub use rotation::Rotation;
pub use shape::Shape;
pub use space::Space;
pub use strata_derive::StableHash;
pub use traverse::{Crossing, clip_inset, traverse};
pub use value::{Value, ValueError};
