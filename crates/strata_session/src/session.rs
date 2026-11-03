//! The command loop: asks the right controller, feeds the engine, records the match.

use strata_model::{Event, GameState, PlayerId};
use strata_rules::{Engine, Outcome, RuleViolation};

use crate::controller::Controller;
use crate::replay::{Replay, SNAPSHOT_EVERY, Snapshot};
use crate::view::PlayerView;

/// The match cannot go on.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    /// A seat that must act has no controller.
    #[error("player {} has no controller", .0.0)]
    NoController(PlayerId),
    /// A controller offered a command or answer the rules refused.
    #[error("rejected: {0}")]
    Rejected(RuleViolation),
    /// The match could not be started.
    #[error("cannot start the match: {0}")]
    Launch(String),
    /// A controller gave up because the match was shut down.
    #[error("the match was stopped")]
    Stopped,
}

/// A match in progress: the engine plus one controller per seat.
pub struct Session<'a> {
    engine: Engine<'a>,
    controllers: Vec<Box<dyn Controller>>,
    snapshots: Vec<Snapshot>,
}

impl<'a> Session<'a> {
    /// Plays `engine` with `controllers`, indexed by `PlayerId`.
    pub fn new(engine: Engine<'a>, controllers: Vec<Box<dyn Controller>>) -> Self {
        Self {
            engine,
            controllers,
            snapshots: Vec::new(),
        }
    }

    /// The committed state.
    pub fn state(&self) -> &GameState {
        self.engine.state()
    }

    /// Asks whoever must act next, and returns the events that committed (none when the
    /// command paused on a decision).
    ///
    /// A pending decision goes to its chooser; otherwise the active player picks a command.
    pub fn step(&mut self) -> Result<Vec<Event>, SessionError> {
        let state = self.engine.state();
        let outcome = match state.pending.clone() {
            Some(pending) => {
                let view = PlayerView::of(state, pending.chooser);
                let seat = self.seat(pending.chooser)?;
                let answer = seat.answer(&view, &pending.decision);
                if seat.abandoned() {
                    return Err(SessionError::Stopped);
                }
                self.engine.answer(answer)
            }
            None => {
                let active = state.flow.active;
                let view = PlayerView::of(state, active);
                let legal = self.engine.legal_commands(active);
                let seat = self.seat(active)?;
                let command = seat.choose_command(&view, &legal);
                if seat.abandoned() {
                    return Err(SessionError::Stopped);
                }
                self.engine.submit(active, command)
            }
        };
        match outcome {
            Outcome::Committed(events) => {
                self.snapshot();
                Ok(events)
            }
            Outcome::NeedsDecision { .. } => Ok(Vec::new()),
            Outcome::Rejected(violation) => Err(SessionError::Rejected(violation)),
        }
    }

    /// Steps until `commands` commands have committed in all.
    pub fn run(&mut self, commands: u32) -> Result<(), SessionError> {
        while self.engine.state().command_index < commands {
            self.step()?;
        }
        Ok(())
    }

    /// The match so far as a replay, with the snapshots taken along the way.
    pub fn replay(&self) -> Replay {
        let setup = (*self.engine.state().setup).clone();
        Replay::new(setup, self.engine.log(), self.snapshots.clone())
    }

    fn seat(&mut self, player: PlayerId) -> Result<&mut dyn Controller, SessionError> {
        match self.controllers.get_mut(player.index()) {
            Some(seat) => Ok(&mut **seat),
            None => Err(SessionError::NoController(player)),
        }
    }

    fn snapshot(&mut self) {
        let state = self.engine.state();
        if state.command_index % SNAPSHOT_EVERY == 0 {
            self.snapshots.push(Snapshot {
                command_index: state.command_index,
                state: state.clone(),
            });
        }
    }
}
