//! Player intents (design §5.1).

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, StableHash, Sym, Value};

use crate::event::Visibility;
use crate::ids::{FigureId, UnitId};

/// One token put on a unit's card.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct TokenPlacement {
    /// The unit receiving the token.
    pub unit: UnitId,
    /// The kind of token.
    pub token: DefId,
}

/// A player's intent. The command log is the save file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub enum Command {
    /// Put tokens from hand onto unit cards, face down.
    PlaceTokens(Vec<TokenPlacement>),
    /// Begin a unit's activation.
    ActivateUnit(UnitId),
    /// Take an action with a figure.
    Action {
        /// The acting figure.
        figure: FigureId,
        /// The action taken.
        action: DefId,
        /// Action arguments chosen by the player.
        args: Value,
    },
    /// Finish the current activation.
    EndActivation,
    /// A command declared by a pack.
    Custom {
        /// The command's name.
        id: Sym,
        /// Pack-defined data.
        payload: Value,
    },
}

impl Command {
    /// Who may see this command: token placements are hidden from other players.
    pub fn visibility(&self) -> Visibility {
        match self {
            Command::PlaceTokens(_) => Visibility::Hidden,
            _ => Visibility::Public,
        }
    }
}
