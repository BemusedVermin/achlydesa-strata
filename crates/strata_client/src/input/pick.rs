//! What a click points at, and why a click can be refused.

use std::fmt;

use strata_kernel::Space;
use strata_model::{FigureId, FigureStatus, GameState};

/// What a click landed on: a board space, the figure standing there, or both.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pick {
    /// The space clicked, if the click was on the board.
    pub space: Option<Space>,
    /// The figure clicked or standing on the space.
    pub figure: Option<FigureId>,
}

impl Pick {
    /// A click on `space`, with the figure anchored there if there is one.
    pub fn at(state: &GameState, space: Space) -> Self {
        let standing = |f: &&strata_model::FigureState| {
            f.status == FigureStatus::OnBoard && f.location == Some(space)
        };
        let figure = state.figures.iter().position(|f| standing(&f));
        Self {
            space: Some(space),
            figure: figure.map(|i| FigureId(i as u32)),
        }
    }

    /// A click on `figure` itself, for example in a list.
    pub fn figure(figure: FigureId) -> Self {
        Self {
            space: None,
            figure: Some(figure),
        }
    }
}

/// Why a click did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The viewer has no decision to answer.
    NothingToDo,
    /// No figure stands where the click landed.
    NoFigure,
    /// The decision does not offer what was clicked.
    NotAChoice,
    /// The decision is answered in its dialog, not on the board.
    UseDialog,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NothingToDo => "There is no decision for you to answer.",
            Self::NoFigure => "There is no figure there.",
            Self::NotAChoice => "That is not one of the choices.",
            Self::UseDialog => "Answer this in the dialog.",
        })
    }
}
