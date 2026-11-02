//! How a resolution stops early, and what a finished command reports.

use strata_model::{Answer, ApplyError, Command, Decision, Event, PlayerId};

/// A command broke a rule and was refused.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct RuleViolation(pub String);

/// Why a resolution stopped before finishing. Propagate it with `?`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Halt {
    /// The answer tape ran out; the chooser must answer before the command can continue.
    NeedsDecision {
        /// The question.
        decision: Decision,
        /// Who must answer.
        chooser: PlayerId,
    },
    /// The command is illegal, or a rule failed.
    Rejected(RuleViolation),
}

impl Halt {
    /// A rejection with the given reason.
    pub fn reject(reason: impl Into<String>) -> Self {
        Halt::Rejected(RuleViolation(reason.into()))
    }
}

impl From<ApplyError> for Halt {
    fn from(e: ApplyError) -> Self {
        Halt::reject(e.to_string())
    }
}

/// What submitting a command or answering a decision did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The command finished and its events are in the state.
    Committed(Vec<Event>),
    /// The command is paused until the chooser answers.
    NeedsDecision {
        /// The question.
        decision: Decision,
        /// Who must answer.
        chooser: PlayerId,
    },
    /// The command was refused.
    Rejected(RuleViolation),
}

/// One committed command: the save-file entry (design §5.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandRecord {
    /// Position in the command log.
    pub index: u32,
    /// Who submitted it.
    pub player: PlayerId,
    /// What was submitted.
    pub command: Command,
    /// The answer tape it finished with.
    pub answers: Vec<Answer>,
    /// The state hash after the command.
    pub state_hash: u64,
}
