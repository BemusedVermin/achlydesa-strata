//! Runtime game state (design §12).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, PackId, Rotation, Space, StableHash, Sym, Value};

use crate::decision::PendingDecision;
use crate::event::Visibility;
use crate::ids::{FigureId, PlayerId, RelicId, UnitId, VarKey};
use crate::scenario::ArmyList;

/// A content pack as it was loaded.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct PackIdentity {
    /// The pack's ID.
    pub id: PackId,
    /// Its version.
    pub version: String,
    /// Hash of its content.
    pub hash: u64,
}

/// Where the game seed came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub enum SeedSource {
    /// A seed chosen locally.
    Plain(u64),
}

impl Default for SeedSource {
    fn default() -> Self {
        SeedSource::Plain(0)
    }
}

/// One seat at the table.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct PlayerSetup {
    /// The player's ID.
    pub id: PlayerId,
    /// The scenario slot they fill.
    pub slot: Sym,
    /// Their team.
    pub team: u8,
    /// Their army list, if the format uses one.
    pub army: Option<ArmyList>,
}

/// The inputs fixed before the first command. With the command log they determine the game.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct MatchSetup {
    /// The packs in play.
    pub packs: Vec<PackIdentity>,
    /// The ruleset.
    pub ruleset: DefId,
    /// The format, if any.
    pub format: Option<DefId>,
    /// The scenario.
    pub scenario: DefId,
    /// The seed.
    pub seed: SeedSource,
    /// The seats.
    pub players: Vec<PlayerSetup>,
}

/// A token in a player's hand or on a unit's card.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct Token {
    /// The kind of token.
    pub kind: DefId,
    /// Who can see which kind it is.
    pub visibility: Visibility,
}

/// Per-player state.
#[derive(Clone, Debug, Default, PartialEq, Eq, StableHash)]
pub struct PlayerState {
    /// Their team.
    pub team: u8,
    /// Token kinds in hand.
    pub hand: Vec<DefId>,
}

/// A unit's runtime state.
#[derive(Clone, Debug, Default, PartialEq, Eq, StableHash)]
pub struct UnitState {
    /// The unit definition.
    pub def: DefId,
    /// The owning player.
    pub owner: PlayerId,
    /// Which copy of the definition this is within the army.
    pub copy: u8,
    /// The unit's figures.
    pub figures: Vec<FigureId>,
    /// Tokens on its card, in placement order.
    pub tokens: Vec<Token>,
}

/// Which figure entry of which unit definition a figure is.
#[derive(Clone, Debug, Default, PartialEq, Eq, StableHash)]
pub struct FigureDefRef {
    /// The unit definition.
    pub unit: DefId,
    /// The figure's name within the unit.
    pub figure: Sym,
}

/// Where a figure is in its life.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, StableHash)]
pub enum FigureStatus {
    /// Waiting off the board.
    #[default]
    Reserve,
    /// Standing on the board.
    OnBoard,
    /// Killed during play.
    Destroyed,
    /// Taken out of play without being killed.
    Removed,
}

/// A wound or status on a figure.
#[derive(Clone, Debug, PartialEq, Eq, StableHash)]
pub struct Marker {
    /// The marker definition.
    pub def: DefId,
    /// How many are stacked; always above zero.
    pub count: u32,
    /// The figure that caused it.
    pub source: Option<FigureId>,
    /// The round after which it lapses.
    pub expiry: Option<u32>,
}

/// A figure's runtime state.
#[derive(Clone, Debug, Default, PartialEq, Eq, StableHash)]
pub struct FigureState {
    /// The owning unit.
    pub unit: UnitId,
    /// The definition entry.
    pub def: FigureDefRef,
    /// Anchor space; `None` when off the board.
    pub location: Option<Space>,
    /// Facing.
    pub rotation: Rotation,
    /// Life status.
    pub status: FigureStatus,
    /// The player in control, which may differ from the unit's owner.
    pub controller: PlayerId,
    /// Wounds and statuses.
    pub markers: Vec<Marker>,
}

/// A relic's runtime state.
#[derive(Clone, Debug, Default, PartialEq, Eq, StableHash)]
pub struct RelicState {
    /// The relic definition.
    pub def: DefId,
    /// Where it sits, if on the board.
    pub location: Option<Space>,
    /// Whether it is face down.
    pub face_down: bool,
    /// The player who claimed it.
    pub holder: Option<PlayerId>,
}

/// What an ability instance is attached to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, StableHash)]
pub enum AbilityOwner {
    /// A unit.
    Unit(UnitId),
    /// A player.
    Player(PlayerId),
    /// A relic.
    Relic(RelicId),
}

/// An ability attached to something, with its resolved parameters.
#[derive(Clone, Debug, PartialEq, Eq, StableHash)]
pub struct AbilityInstance {
    /// The ability definition.
    pub def: DefId,
    /// What it is attached to.
    pub owner: AbilityOwner,
    /// Parameters resolved at attach time.
    pub params: BTreeMap<Sym, Value>,
}

/// One phase on the phase stack.
#[derive(Clone, Debug, Default, PartialEq, Eq, StableHash)]
pub struct PhaseFrame {
    /// The phase.
    pub phase: DefId,
    /// Data private to the phase.
    pub data: BTreeMap<Sym, Value>,
}

/// A unit's current activation.
#[derive(Clone, Debug, Default, PartialEq, Eq, StableHash)]
pub struct Activation {
    /// The activating unit.
    pub unit: UnitId,
    /// The current step.
    pub step: Sym,
    /// Remaining action slots per figure.
    pub slots: BTreeMap<FigureId, u8>,
}

/// Where the game is in its turn structure.
#[derive(Clone, Debug, Default, PartialEq, Eq, StableHash)]
pub struct FlowState {
    /// Active phases, outermost first.
    pub phases: Vec<PhaseFrame>,
    /// The current round.
    pub round: u32,
    /// The turn within the round.
    pub turn: u32,
    /// The player whose turn it is.
    pub active: PlayerId,
    /// The current activation, if any.
    pub activation: Option<Activation>,
}

/// Changes to the static board during play.
#[derive(Clone, Debug, Default, PartialEq, Eq, StableHash)]
pub struct BoardChanges {
    /// Indices into the map's placed features that have been removed.
    pub removed_features: BTreeSet<u32>,
    /// Which figure stands on each anchor space.
    pub occupancy: BTreeMap<Space, FigureId>,
}

/// Everything that changes during a match.
#[derive(Clone, Debug, Default, PartialEq, Eq, StableHash)]
pub struct GameState {
    /// The immutable inputs.
    pub setup: Arc<MatchSetup>,
    /// Removed features and the occupancy index.
    pub board_changes: BoardChanges,
    /// Number of commands committed so far.
    pub command_index: u32,
    /// Players, indexed by `PlayerId`.
    pub players: Vec<PlayerState>,
    /// Units, indexed by `UnitId`.
    pub units: Vec<UnitState>,
    /// Figures, indexed by `FigureId`.
    pub figures: Vec<FigureState>,
    /// Relics, indexed by `RelicId`.
    pub relics: Vec<RelicState>,
    /// Attached ability instances.
    pub abilities: Vec<AbilityInstance>,
    /// Turn structure.
    pub flow: FlowState,
    /// Module and ability variables.
    pub vars: BTreeMap<VarKey, Value>,
    /// The decision awaiting an answer, if any.
    pub pending: Option<PendingDecision>,
}

impl GameState {
    /// The stable hash of the whole state.
    pub fn hash(&self) -> u64 {
        StableHash::stable_hash_value(self)
    }
}
