//! Definition structs, game state, events, commands, decisions and values.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

extern crate self as strata_model;

pub mod footprint;
pub mod map;
pub mod terrain;
pub mod vocab;

pub use footprint::{FootprintDef, MoveMode};
pub use map::{InventoryDef, MapConstraints, MapDef, PlacedFeature, PlacedPiece};
pub use terrain::{AbilityRef, FeatureCell, FeatureDef, Segment, TerrainDef, TileDef};
pub use vocab::{
    DieDef, DieKind, FactionDef, MarkerDef, SourceLoc, StatDef, Text, TraitKeyDef, TraitsDef,
};
