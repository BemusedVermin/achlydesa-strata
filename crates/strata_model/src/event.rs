//! State deltas (design §5.1). `GameState::apply` is the only mutator.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, Rotation, Space, StableHash, Sym, Value};

use crate::ability_key::AbilityKey;
use crate::ids::{FigureId, PlayerId, RelicId, UnitId, VarKey};
use crate::state::{AbilityOwner, Token};

/// Who may see a command, event or token.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, StableHash,
)]
pub enum Visibility {
    /// Every player.
    #[default]
    Public,
    /// Only the player it belongs to; others see a redacted form.
    Hidden,
}

/// A change to the game state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub enum Event {
    /// A figure was placed or moved onto the board.
    FigureMoved {
        /// The figure.
        figure: FigureId,
        /// Its new anchor space.
        to: Space,
        /// Its new rotation.
        rotation: Rotation,
        /// The spaces walked, excluding the start and ending at `to`; empty for a placement. A
        /// pivot in place repeats the space before it.
        #[serde(default)]
        path: Vec<Space>,
        /// Every space the figure's footprint covers at `to`, anchor included, in offset order;
        /// `[to]` for a one-cell figure. Every emitter lists them all: `apply` refuses a list
        /// without the anchor, so an empty one too, and one that changes how many spaces a figure
        /// on the board covers. The serde default only lets a log written before this field
        /// existed be read; such a move cannot be applied, and replays that old are refused by
        /// their format version.
        #[serde(default)]
        cells: Vec<Space>,
    },
    /// Dice were rolled. Presentation and triggers only; no state changes.
    DiceRolled {
        /// The die that was rolled.
        die: DefId,
        /// One result per die.
        results: Vec<u32>,
    },
    /// Wound markers were added to a figure.
    WoundsApplied {
        /// The wounded figure.
        figure: FigureId,
        /// The wound marker.
        wound: DefId,
        /// How many wounds.
        amount: u32,
    },
    /// A figure was destroyed and left the board.
    FigureDestroyed(FigureId),
    /// A marker count on a figure changed.
    MarkerChanged {
        /// The figure.
        figure: FigureId,
        /// The marker.
        marker: DefId,
        /// Change in count; a count that reaches zero removes the marker.
        delta: i32,
        /// The figure that caused it, if any.
        source: Option<FigureId>,
        /// The round after which it lapses, if any.
        expiry: Option<u32>,
    },
    /// A token moved from its owner's hand onto a unit's card.
    TokenPlaced {
        /// The receiving unit.
        unit: UnitId,
        /// The token, with its visibility.
        token: Token,
    },
    /// A face-down token on a unit's card was turned face up.
    TokenRevealed {
        /// The unit.
        unit: UnitId,
        /// Position of the token on the card.
        index: u8,
    },
    /// A phase began and was pushed on the phase stack.
    PhaseEntered(DefId),
    /// The top phase ended and was popped.
    PhaseExited(DefId),
    /// A unit began its activation with these slot budgets.
    ActivationStarted {
        /// The activating unit.
        unit: UnitId,
        /// The first step.
        step: Sym,
        /// Each figure's slot budget, by slot name.
        slots: BTreeMap<FigureId, BTreeMap<Sym, u8>>,
    },
    /// A figure's remaining count of one slot was set, by consuming or granting.
    SlotsChanged {
        /// The figure.
        figure: FigureId,
        /// The slot.
        slot: Sym,
        /// Slots left afterwards.
        remaining: u8,
    },
    /// The activation moved to another step.
    StepChanged(Sym),
    /// The current activation ended.
    ActivationEnded,
    /// A player claimed a relic.
    RelicClaimed {
        /// The relic.
        relic: RelicId,
        /// The claimant.
        by: PlayerId,
    },
    /// A relic's holder let go, because they left it or were removed.
    RelicReleased(RelicId),
    /// A relic was turned face up.
    RelicRevealed(RelicId),
    /// An ability instance was attached to a unit, player or relic.
    AbilityAttached {
        /// The ability definition.
        def: AbilityKey,
        /// What it is attached to.
        owner: AbilityOwner,
        /// Parameters resolved at attach time.
        params: BTreeMap<Sym, Value>,
    },
    /// The first matching ability instance was removed from its owner.
    AbilityDetached {
        /// The ability definition.
        def: AbilityKey,
        /// What it was attached to.
        owner: AbilityOwner,
    },
    /// A variable was set; a nil value removes it.
    VarSet {
        /// The variable.
        key: VarKey,
        /// Its new value.
        value: Value,
    },
    /// A figure came under a different player's control.
    ControlChanged {
        /// The figure.
        figure: FigureId,
        /// Its new controller.
        controller: PlayerId,
    },
    /// A player's hand of tokens was set, replacing what they held.
    TokensDealt {
        /// The player.
        player: PlayerId,
        /// The token kinds now in hand.
        tokens: Vec<DefId>,
    },
    /// Every token on a unit's card was taken off it.
    TokensDiscarded(UnitId),
    /// A round began: the round counter is set and the turn counter restarts.
    RoundStarted(u32),
    /// A turn began, and its player became the active player.
    TurnStarted {
        /// The turn within the round, counted from 1.
        turn: u32,
        /// The player whose turn it is.
        active: PlayerId,
    },
    /// An event declared by a pack. Triggers and presentation only; no state changes.
    Custom {
        /// The event's name.
        id: Sym,
        /// Pack-defined data.
        payload: Value,
    },
}

impl Event {
    /// Who may see this event: face-down token placements are hidden.
    pub fn visibility(&self) -> Visibility {
        match self {
            Event::TokenPlaced { token, .. } => token.visibility,
            _ => Visibility::Public,
        }
    }
}
