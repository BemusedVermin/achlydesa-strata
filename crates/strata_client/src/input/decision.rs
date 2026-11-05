//! Decision to widget: each kind of decision maps to one UI pattern (design §13.5), so a
//! script's `ctx.choose` gets correct UI with no UI code.

use std::collections::BTreeSet;

use strata_kernel::Space;
use strata_model::{Answer, Decision, DecisionKind, FigureId, GameState};

use super::pick::{Pick, Refusal};

/// How a decision is shown and answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Widget {
    /// Highlight these figures on the board; clicking one answers.
    Figures(BTreeSet<FigureId>),
    /// Highlight these spaces on the board; clicking one answers.
    Spaces(BTreeSet<Space>),
    /// A dialog with one button per option.
    Options(Vec<String>),
    /// A yes and a no button.
    YesNo,
    /// A number stepper bounded by `min` and `max`.
    Stepper {
        /// Smallest allowed value.
        min: i32,
        /// Largest allowed value.
        max: i32,
    },
    /// A list the chooser drags into order.
    Order(Vec<String>),
}

/// What the board marks: figures, and spaces that are not tied to a figure.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Highlights {
    /// Highlighted figures.
    pub figures: BTreeSet<FigureId>,
    /// Highlighted spaces.
    pub spaces: BTreeSet<Space>,
}

impl Highlights {
    /// Every space to outline: the highlighted spaces and where the highlighted figures stand.
    pub fn board_spaces(&self, state: &GameState) -> Vec<Space> {
        let standing = self.figures.iter().filter_map(|f| {
            let figure = state.figures.get(f.index())?;
            figure.location
        });
        let all: BTreeSet<Space> = self.spaces.iter().copied().chain(standing).collect();
        all.into_iter().collect()
    }
}

impl Widget {
    /// The widget for `decision`.
    pub fn of(decision: &Decision) -> Self {
        let labels =
            |items: &[strata_kernel::Sym]| items.iter().map(|s| s.as_str().to_owned()).collect();
        match &decision.kind {
            DecisionKind::ChooseFigure(figures) => Self::Figures(figures.iter().copied().collect()),
            DecisionKind::ChooseSpace(spaces) => Self::Spaces(spaces.iter().copied().collect()),
            DecisionKind::ChooseOption(options) => Self::Options(labels(options)),
            DecisionKind::YesNo => Self::YesNo,
            DecisionKind::Number { min, max } => Self::Stepper {
                min: *min,
                max: *max,
            },
            DecisionKind::Order(items) => Self::Order(labels(items)),
        }
    }

    /// What the board highlights: exactly the legal figures or spaces, and nothing for dialogs.
    pub fn highlights(&self) -> Highlights {
        match self {
            Self::Figures(figures) => Highlights {
                figures: figures.clone(),
                ..Highlights::default()
            },
            Self::Spaces(spaces) => Highlights {
                spaces: spaces.clone(),
                ..Highlights::default()
            },
            _ => Highlights::default(),
        }
    }

    /// The answer a click gives, or why it gives none.
    pub fn click(&self, pick: &Pick) -> Result<Answer, Refusal> {
        match self {
            Self::Figures(figures) => {
                let figure = pick.figure.ok_or(Refusal::NoFigure)?;
                if figures.contains(&figure) {
                    Ok(Answer::Figure(figure))
                } else {
                    Err(Refusal::NotAChoice)
                }
            }
            Self::Spaces(spaces) => match pick.space {
                Some(space) if spaces.contains(&space) => Ok(Answer::Space(space)),
                _ => Err(Refusal::NotAChoice),
            },
            _ => Err(Refusal::UseDialog),
        }
    }

    /// The starting state of the dialog parts: the stepper's value and the list's order.
    pub fn start(&self) -> (i32, Vec<u32>) {
        match self {
            Self::Stepper { min, .. } => (*min, Vec::new()),
            Self::Order(items) => (0, (0..items.len() as u32).collect()),
            _ => (0, Vec::new()),
        }
    }
}

/// Moves the item at `from` to position `to`, shifting the items between.
pub fn reorder(order: &mut Vec<u32>, from: usize, to: usize) {
    if from < order.len() && to < order.len() {
        let item = order.remove(from);
        order.insert(to, item);
    }
}
