//! The data of a pack test: where figures start and the steps that check the rules (design §16.4).

use serde::{Deserialize, Serialize};
use strata_kernel::{Space, StableHash, Sym, Value};

use crate::decision::Answer;
use crate::ids::PlayerId;

/// Figures put on the board directly, skipping deployment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    /// Name of the unit, bare or `pack:name`.
    pub unit: Sym,
    /// The player who owns it.
    pub player: PlayerId,
    /// One space per figure, in the order the unit lists its figures; none leaves them in reserve. none leaves them in reserve.
    pub at: Vec<Space>,
}

/// A token placed on the unit that a figure belongs to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
#[serde(deny_unknown_fields)]
pub struct TokenAt {
    /// The handle of any figure of the unit.
    pub figure: String,
    /// The token's kind, as the module that deals it numbers them.
    pub kind: u32,
}

/// One step of a test: a command to submit or a check to make.
///
/// Figures are named by handle, such as `p0.soldier.trooper#2`: the player, the unit, the figure
/// entry and its one-based position among the figures with that name. `rig` forces the faces of
/// each dice roll the command makes, one list per roll.
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
        /// The faces to force.
        #[serde(default)]
        rig: Vec<Vec<Sym>>,
    },
    /// A figure takes an action, submitted by the player who controls it.
    Action {
        /// The figure's handle.
        figure: String,
        /// The action's name.
        action: Sym,
        /// The action's arguments.
        #[serde(default)]
        args: Value,
        /// The faces to force.
        #[serde(default)]
        rig: Vec<Vec<Sym>>,
    },
    /// Answers the decision the last command stopped at.
    Answer(Answer),
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
        /// If set, the event must be about this figure.
        #[serde(default)]
        figure: Option<String>,
    },
    /// Checks where a figure is in its life: `on_board`, `destroyed`, `removed` or `reserve`.
    ExpectFigure {
        /// The figure's handle.
        figure: String,
        /// The status it must have.
        status: Sym,
    },
    /// Checks that a decision with this prompt is waiting for an answer.
    ExpectDecision {
        /// The decision's prompt.
        prompt: Sym,
    },
    /// Checks how the match ended, through the `victory` slot's module.
    ExpectResult {
        /// The condition that ended it; `None` if the match must still be going on.
        #[serde(default)]
        condition: Option<Sym>,
        /// Every winning player; empty for a draw.
        #[serde(default)]
        winners: Vec<PlayerId>,
    },
    /// Checks an army list against a format's and a scenario's army rules, through the `army`
    /// slot's module.
    ExpectArmy {
        /// Name of the army list.
        army: Sym,
        /// Name of the format whose rules apply.
        #[serde(default)]
        format: Option<Sym>,
        /// Name of the scenario whose rules apply.
        #[serde(default)]
        scenario: Option<Sym>,
        /// The rule each violation came from, in order: the format's, then the scenario's.
        #[serde(default)]
        violations: Vec<Sym>,
    },
    /// A player secretly puts tokens from their hand on unit cards.
    PlaceTokens {
        /// The player placing them.
        player: PlayerId,
        /// Which token goes on which unit.
        placements: Vec<TokenAt>,
    },
    /// A player starts activating the unit that a figure belongs to.
    Activate {
        /// The player activating.
        player: PlayerId,
        /// The handle of any figure of the unit.
        figure: String,
    },
    /// A player ends their current activation, whatever slots are left.
    EndActivation {
        /// The player ending it.
        player: PlayerId,
    },
    /// The attacker takes the ruleset's `attack` action against the target, submitted by the
    /// player who controls the attacker.
    Attack {
        /// The attacker's handle.
        figure: String,
        /// The target's handle.
        target: String,
        /// The faces to force: the attack roll, then the defense roll.
        #[serde(default)]
        rig: Vec<Vec<Sym>>,
    },
}
