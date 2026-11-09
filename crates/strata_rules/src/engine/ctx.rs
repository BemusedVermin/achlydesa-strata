//! What every rule sees while resolving (design §5.1).

use strata_kernel::{CommandRng, Space, WindowId};
use strata_model::{Answer, Command, Decision, Event, FigureId, GameState, PlayerId, UnitId};

use super::halt::Halt;
use crate::flow::ActivationRules;

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

    /// Resolves the reprisals `figure` provokes on a move, through the ruleset's `engagement`
    /// slot. `walk` holds the spaces its body covers where it stands and then after each step
    /// (see [`Engagement::leave`](crate::Engagement::leave)). A move calls this before it emits
    /// `FigureMoved`, and skips the move if the figure is out of play afterwards.
    ///
    /// Contexts without a ruleset, and rulesets without an engagement module, provoke nothing.
    fn leave_engagement(&mut self, _figure: FigureId, _walk: &[Vec<Space>]) -> Result<(), Halt> {
        Ok(())
    }

    /// Takes the `Command::Action` in `command` for `player`: checks who may act and the targets,
    /// spends the slots under `rules`, then resolves it.
    ///
    /// Contexts without a ruleset or an action runner reject every action.
    fn run_action(
        &mut self,
        _rules: &ActivationRules,
        _player: PlayerId,
        _command: &Command,
    ) -> Result<(), Halt> {
        Err(Halt::reject("this context cannot take actions"))
    }
}
