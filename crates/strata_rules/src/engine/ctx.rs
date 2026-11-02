//! What every rule sees while resolving (design §5.1).

use strata_kernel::CommandRng;
use strata_model::{Answer, Decision, Event, GameState, PlayerId};

use super::halt::Halt;

/// The working copy of a command's resolution.
pub trait Ctx {
    /// The working copy of the state.
    fn state(&self) -> &GameState;
    /// The command's random stream.
    fn rng(&mut self) -> &mut CommandRng;
    /// Passes `e` through replacements, applies it and queues its triggers.
    fn emit(&mut self, e: Event) -> Result<(), Halt>;
    /// Asks `chooser`. Returns the recorded answer, or halts with `NeedsDecision` once the
    /// tape is exhausted.
    fn choose(&mut self, chooser: PlayerId, d: Decision) -> Result<Answer, Halt>;
}
