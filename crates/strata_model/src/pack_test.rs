//! The data of a pack test: where figures start and the steps that check the rules (design §16.4).

use serde::{Deserialize, Serialize};
use strata_kernel::{Space, StableHash, Sym, Value};

use crate::ids::PlayerId;

/// Figures put on the board directly, skipping deployment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    /// Name of the unit, bare or `pack:name`.
    pub unit: Sym,
    /// The player who owns it.
    pub player: PlayerId,
    /// One space per figure, in the order the unit lists its figures.
    pub at: Vec<Space>,
}

/// One step of a test: a command to submit or a check to make.
///
/// Figures are named by handle, such as `p0.soldier.trooper#2`: the player, the unit, the figure
/// entry and its one-based position among the figures with that name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub enum Step {
    /// Submits a pack-declared command.
    Custom {
        /// The player submitting it.
        player: PlayerId,
        /// The command's name.
        id: Sym,
        /// Pack-defined data.
        #[serde(default)]
        payload: Value,
    },
    /// Checks a figure's stat after every modifier.
    ExpectStat {
        /// The figure's handle.
        figure: String,
        /// The stat.
        stat: Sym,
        /// The value it must have.
        value: i64,
    },
    /// Checks that the latest command emitted an event of this kind.
    ExpectEvent {
        /// The event's kind, such as `figure_destroyed`.
        kind: Sym,
    },
}
