//! The victory service interface (design §11.2, §15.2).
//!
//! A module in the `victory` slot provides [`Victory`]. The engine asks it to settle once each
//! command has resolved, handing it the command's events so it can tell which rounds ended;
//! refuses commands once it reports a result; and offers no legal commands then.

use strata_kernel::{Frac, Sym};
use strata_model::{Event, GameState, PlayerId};

use crate::engine::{Ctx, Halt};
use crate::native::NativeEnv;
use crate::query::LogicError;

/// How a match ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchResult {
    /// The condition that ended it, such as `eliminate_all`, or `script` for a Lua function.
    pub condition: Sym,
    /// The condition's position in the scenario's `victory` list.
    pub index: usize,
    /// Every player of the winning team, in player order; empty for a draw.
    pub winners: Vec<PlayerId>,
}

/// How close a player is to meeting one victory condition, for the AI's evaluator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    /// The condition, such as `hold_zone`.
    pub condition: Sym,
    /// The condition's position in the scenario's `victory` list.
    pub index: usize,
    /// From 0 (nowhere near) to 1 (met).
    pub value: Frac,
}

/// How matches end, as the module in the `victory` slot defines it.
pub trait Victory: Send + Sync {
    /// Checks the victory conditions once a command has resolved, with `events` the events it
    /// emitted, and emits the end of the match when one is met. Does nothing once the match is
    /// over.
    fn settle(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, events: &[Event]) -> Result<(), Halt>;
    /// How the match ended, or `None` while it goes on.
    fn result(&self, state: &GameState) -> Option<MatchResult>;
    /// An estimate per condition that can supply one of how close `player` is to meeting it.
    fn progress(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        player: PlayerId,
    ) -> Result<Vec<Progress>, LogicError>;
}
