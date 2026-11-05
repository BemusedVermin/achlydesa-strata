//! Scenarios, formats, army lists and pack tests (design §11.2–11.4, §16.4).
//!
//! These are plain data resolved against loaded definitions when a match is built, so they refer
//! to maps, relics, rulesets and units by authored name.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, PackId, StableHash, Sym, Value};

use crate::ability::{Logic, ScriptRef};
use crate::pack_test::{Placement, Step};
use crate::ruleset::ModuleRef;
use crate::vocab::{SourceLoc, Text};

/// How many players a scenario takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct PlayerRange {
    /// Fewest players.
    pub min: u8,
    /// Most players.
    pub max: u8,
}

/// A player seat and where it deploys.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct ScenarioSlot {
    /// The seat's name.
    pub id: Sym,
    /// Name of the map zone the seat starts in.
    pub start: Sym,
    /// Position in the deployment order.
    pub deploy_order: u8,
}

/// A relic put on a map's relic spot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct RelicPlacement {
    /// Name of the relic.
    pub relic: Sym,
    /// Name of the map's relic spot.
    pub spot: Sym,
    /// Whether it starts face down.
    #[serde(default)]
    pub face_down: bool,
}

/// A map plus deployment, objectives and scripted events.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct ScenarioDef {
    /// The scenario's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// Name of the map.
    pub map: Sym,
    /// How many players it takes.
    pub players: PlayerRange,
    /// Player seats.
    pub slots: Vec<ScenarioSlot>,
    /// Army rules, which add to the format's.
    #[serde(default)]
    pub army: Vec<Logic>,
    /// Relics placed on the map.
    #[serde(default)]
    pub relics: Vec<RelicPlacement>,
    /// Victory conditions; the first listed wins ties.
    #[serde(default)]
    pub victory: Vec<Logic>,
    /// Add-on modules for this scenario.
    #[serde(default)]
    pub addons: Vec<ModuleRef>,
    /// Scripted handlers keyed by event.
    #[serde(default)]
    pub after: BTreeMap<Sym, ScriptRef>,
    /// Free-form tags.
    #[serde(default)]
    pub tags: BTreeSet<Sym>,
    /// Data the engine ignores and tools may use.
    #[serde(default)]
    pub meta: BTreeMap<Sym, Value>,
    /// Where the definition was authored.
    #[serde(default)]
    pub source: SourceLoc,
}

/// The scenarios a format allows and how one is chosen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct ScenarioPool {
    /// Names of the allowed scenarios.
    pub pool: Vec<Sym>,
    /// How one is picked, such as `alternate_veto`.
    pub pick: Sym,
}

/// A competitive envelope: ruleset, army rules and scenarios.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct FormatDef {
    /// The format's handle.
    pub id: DefId,
    /// Display name.
    pub name: Text,
    /// Name of the ruleset.
    pub ruleset: Sym,
    /// Army rules.
    #[serde(default)]
    pub army: Vec<Logic>,
    /// How armies are chosen, such as `build` or `draft_snake`.
    pub selection: Sym,
    /// The scenarios allowed.
    pub scenarios: ScenarioPool,
    /// Rounds before the game ends.
    #[serde(default)]
    pub round_limit: Option<u8>,
    /// Minutes on each player's clock.
    #[serde(default)]
    pub clock_minutes: Option<u16>,
    /// Tiebreakers in order.
    #[serde(default)]
    pub tiebreakers: Vec<Sym>,
    /// Free-form tags.
    #[serde(default)]
    pub tags: BTreeSet<Sym>,
    /// Data the engine ignores and tools may use.
    #[serde(default)]
    pub meta: BTreeMap<Sym, Value>,
    /// Where the definition was authored.
    #[serde(default)]
    pub source: SourceLoc,
}

/// A unit picked for an army, possibly several times.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct ArmyEntry {
    /// Name of the unit, possibly qualified by pack.
    pub unit: Sym,
    /// Number of copies.
    pub copies: u8,
}

/// One player's picks. Point totals are computed, never stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct ArmyList {
    /// Display name.
    pub name: Text,
    /// Name of the format, possibly qualified by pack.
    pub format: Sym,
    /// Version of each pack the list was built against.
    pub packs: BTreeMap<PackId, String>,
    /// The picks.
    pub units: Vec<ArmyEntry>,
}

/// A pack test: a ruleset and map, figures placed on it, and the steps that check the rules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct TestDef {
    /// What the test checks.
    pub name: Text,
    /// Name of the ruleset.
    #[serde(default)]
    pub ruleset: Option<Sym>,
    /// Name of the map.
    #[serde(default)]
    pub map: Option<Sym>,
    /// Name of a scenario whose relics are put on its map's spots before the first step.
    #[serde(default)]
    pub scenario: Option<Sym>,
    /// Figures put on the board before the first step.
    #[serde(default)]
    pub place: Vec<Placement>,
    /// What the test does and checks, in order.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// Where the definition was authored.
    #[serde(default)]
    pub source: SourceLoc,
}
