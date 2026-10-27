//! State deltas (design §5.1). `GameState::apply` is the only mutator.

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, Rotation, Space, StableHash, Sym, Value};

use crate::ids::{FigureId, PlayerId, RelicId, UnitId, VarKey};
use crate::state::Token;

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
    /// A player claimed a relic.
    RelicClaimed {
        /// The relic.
        relic: RelicId,
        /// The claimant.
        by: PlayerId,
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
