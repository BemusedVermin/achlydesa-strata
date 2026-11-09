//! The resolver: commands become events through an answer tape (design §5.1 to §5.3, §6.7).

mod ctx;
mod exec;
mod halt;
mod hooks;
mod names;

pub use ctx::Ctx;
pub use exec::MAX_DECISIONS;
pub use halt::{CommandRecord, Halt, Outcome, RuleViolation};
pub use hooks::HookRunner;
pub use names::{event_figure, event_kind, event_subjects};

use strata_model::{Answer, Command, GameState, PendingDecision, PlayerId, TokenChoice};

use exec::Exec;
use names::answer_fits;

use crate::flow::{Flow, TokenRules};
use crate::runtime::RulesRuntime;

/// Drives a game: owns the committed state and the command log.
pub struct Engine<'a> {
    rt: &'a RulesRuntime,
    hooks: &'a dyn HookRunner,
    state: GameState,
    log: Vec<CommandRecord>,
    flow: Option<Flow<'a>>,
}

impl<'a> Engine<'a> {
    /// Starts from `state`, an empty log and the registrations in `rt`.
    pub fn new(rt: &'a RulesRuntime, hooks: &'a dyn HookRunner, state: GameState) -> Self {
        Self {
            rt,
            hooks,
            state,
            log: Vec::new(),
            flow: None,
        }
    }

    /// Supplies the turn structure that [`Engine::legal_commands`] is derived from.
    pub fn with_flow(mut self, flow: Flow<'a>) -> Self {
        self.flow = Some(flow);
        self
    }

    /// The committed state.
    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// The committed commands, oldest first.
    pub fn log(&self) -> &[CommandRecord] {
        &self.log
    }

    /// Submits `command` for `player`. Rejected while a decision is pending.
    pub fn submit(&mut self, player: PlayerId, command: Command) -> Outcome {
        if self.state.pending.is_some() {
            return rejected("a decision is pending");
        }
        self.run(player, command, Vec::new())
    }

    /// Answers the pending decision and re-runs its command with the longer tape.
    ///
    /// An answer that does not fit the question is rejected and the question stays pending.
    pub fn answer(&mut self, answer: Answer) -> Outcome {
        let Some(pending) = self.state.pending.clone() else {
            return rejected("no decision is pending");
        };
        if !answer_fits(&pending.decision, &answer) {
            return rejected("answer does not fit the decision");
        }
        let mut tape = pending.answers;
        tape.push(answer);
        self.run(pending.player, pending.command, tape)
    }

    /// Commands `player` may legally submit; empty until [`Engine::with_flow`] is called.
    pub fn legal_commands(&self, player: PlayerId) -> Vec<Command> {
        match &self.flow {
            Some(flow) => flow.legal_commands(self.rt, self.hooks.env(), &self.state, player),
            None => Vec::new(),
        }
    }

    /// How `player` may place the tokens in hand, from the tokens module's [`TokenRules`]: `None`
    /// while a decision is pending, out of turn, without such a module, or with nothing to place.
    /// Like [`legal_commands`](Self::legal_commands), only the active player is offered a choice,
    /// because the session submits every command as the active player.
    pub fn token_choice(&self, player: PlayerId) -> Option<TokenChoice> {
        let rules = self.rt.service::<dyn TokenRules>()?;
        if self.state.pending.is_some() || self.state.flow.active != player {
            return None;
        }
        rules.choice(&self.state, player)
    }

    fn run(&mut self, player: PlayerId, command: Command, tape: Vec<Answer>) -> Outcome {
        match Exec::run(self.rt, self.hooks, &self.state, player, &command, &tape) {
            Ok((state, events)) => {
                let index = self.state.command_index;
                self.state = state;
                self.state.command_index = index + 1;
                self.log.push(CommandRecord {
                    index,
                    player,
                    command,
                    answers: tape,
                    state_hash: self.state.hash(),
                });
                Outcome::Committed(events)
            }
            Err(Halt::NeedsDecision { decision, chooser }) => {
                self.state.pending = Some(PendingDecision {
                    decision: decision.clone(),
                    chooser,
                    player,
                    command,
                    answers: tape,
                });
                Outcome::NeedsDecision { decision, chooser }
            }
            Err(Halt::Rejected(violation)) => {
                self.state.pending = None;
                Outcome::Rejected(violation)
            }
        }
    }
}

fn rejected(reason: &str) -> Outcome {
    Outcome::Rejected(RuleViolation(reason.into()))
}

#[cfg(test)]
mod tests;
