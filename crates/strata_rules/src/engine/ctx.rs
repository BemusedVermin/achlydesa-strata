//! What every rule sees while resolving (design §5.1).

use strata_kernel::{CommandRng, WindowId};
use strata_model::{Answer, Decision, Event, GameState, PlayerId, UnitId};

use super::halt::Halt;

/// The working copy of a command's resolution.
pub trait Ctx {
    /// The working copy of the state.
    fn state(&self) -> &GameState;
    /// The command's random stream.
    fn rng(&mut self) -> &mut CommandRng;
    /// Passes `e` through replacements, applies it and queues its triggers.
    fn emit(&mut self, e: Event) -> Result<(), Halt>;
    /// Pauses at window `id`, offering its eligible reactions to their controllers.
    ///
    /// A window with no eligible reactions asks nothing. Rejects an undeclared window and a
    /// window opened while it is already open.
    fn window(&mut self, id: &WindowId) -> Result<(), Halt>;
    /// Asks `chooser`. Returns the recorded answer, or halts with `NeedsDecision` once the
    /// tape is exhausted.
    fn choose(&mut self, chooser: PlayerId, d: Decision) -> Result<Answer, Halt>;
    /// Whether `player` may activate `unit` now, according to the ruleset's activation gate.
    ///
    /// Contexts without a ruleset, and rulesets without a gate, allow every activation.
    fn may_activate(&self, _player: PlayerId, _unit: UnitId) -> bool {
        true
    }
}
