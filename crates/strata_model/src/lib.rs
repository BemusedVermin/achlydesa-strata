//! Definition structs, game state, events, commands, decisions and values.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

extern crate self as strata_model;

pub mod ability;
pub mod footprint;
pub mod map;
pub mod relic;
pub mod ruleset;
pub mod scenario;
pub mod terrain;
pub mod unit;
pub mod vocab;

pub use ability::{
    AbilityDef, ActionDef, ArchetypeInfo, Descriptor, Logic, ModifierDef, ModifierLayer,
    ReactionDef, ReplacementDef, Scope, ScriptRef, TriggerDef, TriggerLimit,
};
pub use footprint::{FootprintDef, MoveMode};
pub use map::{InventoryDef, MapConstraints, MapDef, PlacedFeature, PlacedPiece};
pub use relic::{RelicDef, RelicGrant, RelicMode};
pub use ruleset::{ConfigField, ModuleRef, RuleModuleDef, RulesetDef, UndoPolicy};
pub use scenario::{
    ArmyEntry, ArmyList, FormatDef, PlayerRange, RelicPlacement, ScenarioDef, ScenarioPool,
    ScenarioSlot, TestDef,
};
pub use terrain::{AbilityRef, FeatureCell, FeatureDef, Segment, TerrainDef, TileDef};
pub use unit::{CardInfo, FigureDef, LosProfile, UnitDef};
pub use vocab::{
    DieDef, DieKind, FactionDef, MarkerDef, SourceLoc, StatDef, Text, TraitKeyDef, TraitsDef,
};
