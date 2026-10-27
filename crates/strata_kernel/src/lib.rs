//! Hex math, exact line-of-sight geometry, PCG32, hashing and typed IDs.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

extern crate self as strata_kernel;

pub mod hash;
pub mod hex;
pub mod id;
pub mod pcg;
pub mod rng;
pub mod rotation;
pub mod shape;
pub mod space;
pub mod value;

pub use hash::{StableHash, StableHasher};
pub use hex::{Axis, DIRECTIONS, Hex};
pub use id::{DefId, ModuleId, PackId, SlotId, Sym, WindowId};
pub use pcg::Pcg32;
pub use rng::CommandRng;
pub use rotation::Rotation;
pub use shape::Shape;
pub use space::Space;
pub use strata_derive::StableHash;
pub use value::{Value, ValueError};
