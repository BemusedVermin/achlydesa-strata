//! Questions the rules ask mid-resolution, and their answers (design §5.1).

use serde::{Deserialize, Serialize};
use strata_kernel::{Space, StableHash, Sym};

use crate::command::Command;
use crate::ids::{FigureId, PlayerId};

/// What a decision asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub enum DecisionKind {
    /// Pick one of these figures.
    ChooseFigure(Vec<FigureId>),
    /// Pick one of these spaces.
    ChooseSpace(Vec<Space>),
    /// Pick one of these labelled options.
    ChooseOption(Vec<Sym>),
    /// Answer yes or no.
    YesNo,
    /// Pick a whole number from `min` to `max` inclusive.
    Number {
        /// Smallest allowed value.
        min: i32,
        /// Largest allowed value.
        max: i32,
    },
    /// Put these labelled items in order, such as simultaneous triggers.
    Order(Vec<Sym>),
}

/// A question with a prompt, asked of a chooser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct Decision {
    /// Text key shown to the chooser.
    pub prompt: Sym,
    /// What is being asked.
    pub kind: DecisionKind,
}

/// Why a decision cannot be asked: no answer could satisfy it, so the match would wait forever.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Unanswerable {
    /// A figure, space, option or order decision with an empty list. The text names the list.
    #[error("the decision offers no {0}")]
    Empty(&'static str),
    /// A number decision whose range is empty.
    #[error("number min {min} is greater than max {max}")]
    InvertedRange {
        /// The decision's smallest allowed value.
        min: i32,
        /// The decision's largest allowed value.
        max: i32,
    },
}

impl Decision {
    /// Checks that some answer fits this decision. The engine refuses to ask one that fails, and
    /// every caller of `choose`, native or Lua, goes through that check.
    pub fn check(&self) -> Result<(), Unanswerable> {
        let empty = |len: usize, what| match len {
            0 => Err(Unanswerable::Empty(what)),
            _ => Ok(()),
        };
        match &self.kind {
            DecisionKind::ChooseFigure(figures) => empty(figures.len(), "figures"),
            DecisionKind::ChooseSpace(spaces) => empty(spaces.len(), "spaces"),
            DecisionKind::ChooseOption(options) => empty(options.len(), "options"),
            DecisionKind::Order(items) => empty(items.len(), "items to order"),
            DecisionKind::YesNo => Ok(()),
            &DecisionKind::Number { min, max } if min > max => {
                Err(Unanswerable::InvertedRange { min, max })
            }
            DecisionKind::Number { .. } => Ok(()),
        }
    }
}

/// A chooser's reply, matching the kind of the decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub enum Answer {
    /// The chosen figure.
    Figure(FigureId),
    /// The chosen space.
    Space(Space),
    /// Index of the chosen option.
    Option(u32),
    /// The yes or no reply.
    YesNo(bool),
    /// The chosen number.
    Number(i32),
    /// Indices of the items, first to last.
    Order(Vec<u32>),
}

/// An unanswered decision, kept in state so a mid-decision save restores the same question.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, StableHash)]
pub struct PendingDecision {
    /// The question.
    pub decision: Decision,
    /// Who must answer.
    pub chooser: PlayerId,
    /// Who submitted the command.
    pub player: PlayerId,
    /// The command being resolved.
    pub command: Command,
    /// Answers given so far, in order.
    pub answers: Vec<Answer>,
}
