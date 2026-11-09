//! The token placement screen's state: which token of the hand goes on which unit.
//!
//! The rules describe the placement as a [`TokenChoice`] rather than as listed commands, and the
//! finished placement is checked with the same [`TokenChoice::check`] the rules use, so the
//! screen only ever submits a placement the rules accept.

use strata_kernel::DefId;
use strata_model::{Command, FigureId, GameState, PlayerId, TokenChoice, TokenPlacement, UnitId};

use super::decision::Highlights;
use super::pick::{Pick, Refusal};

/// The hand placed so far.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TokenPlacer {
    viewer: PlayerId,
    choice: TokenChoice,
    assigned: Vec<Option<UnitId>>,
    selected: Option<usize>,
}

impl TokenPlacer {
    /// A placement for `viewer` over `choice`, with the first token selected.
    pub fn new(viewer: PlayerId, choice: TokenChoice) -> Self {
        let assigned = vec![None; choice.hand.len()];
        let selected = (!choice.hand.is_empty()).then_some(0);
        Self {
            viewer,
            choice,
            assigned,
            selected,
        }
    }

    /// The tokens to place, by kind.
    pub fn hand(&self) -> &[DefId] {
        &self.choice.hand
    }

    /// The units that may take a token.
    pub fn units(&self) -> &[UnitId] {
        &self.choice.units
    }

    /// The unit each token of the hand is on so far.
    pub fn assigned(&self) -> &[Option<UnitId>] {
        &self.assigned
    }

    /// The token, by its position in the hand, that the next unit picked receives.
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    /// Selects the `token`th token of the hand, to place it or move it.
    pub fn select(&mut self, token: usize) {
        if token < self.assigned.len() {
            self.selected = Some(token);
        }
    }

    /// Puts the selected token on `unit`. A token already there goes back to the hand, and the
    /// next token still in hand is selected.
    pub fn assign(&mut self, unit: UnitId) -> Result<(), Refusal> {
        if !self.choice.units.contains(&unit) {
            return Err(Refusal::NoTokenHere);
        }
        let token = self.selected.ok_or(Refusal::PickToken)?;
        for held in self.assigned.iter_mut().filter(|u| **u == Some(unit)) {
            *held = None;
        }
        self.assigned[token] = Some(unit);
        self.selected = self.assigned.iter().position(Option::is_none);
        Ok(())
    }

    /// Takes every token back into the hand.
    pub fn clear(&mut self) {
        *self = Self::new(self.viewer, self.choice.clone());
    }

    /// The placement command, once every token is on a unit as the rules allow.
    pub fn command(&self) -> Option<Command> {
        let put = self.hand().iter().zip(&self.assigned).map(|(token, unit)| {
            unit.map(|unit| TokenPlacement {
                unit,
                token: *token,
            })
        });
        let put: Vec<TokenPlacement> = put.collect::<Option<_>>()?;
        self.choice.check(&put).ok()?;
        Some(Command::PlaceTokens(put))
    }

    /// Applies a board click: a figure puts the selected token on its unit.
    pub fn click(&mut self, state: &GameState, pick: &Pick) -> Result<(), Refusal> {
        let figure = pick.figure.ok_or(Refusal::NoFigure)?;
        let unit = state.figures.get(figure.index()).map(|f| f.unit);
        let owner = unit
            .and_then(|u| state.units.get(u.index()))
            .map(|u| u.owner);
        match unit {
            Some(_) if owner != Some(self.viewer) => Err(Refusal::NotYours),
            Some(unit) => self.assign(unit),
            None => Err(Refusal::NoFigure),
        }
    }

    /// What the board marks: the figures of every unit that may take a token, while one is
    /// selected.
    pub fn highlights(&self, state: &GameState) -> Highlights {
        let mut out = Highlights::default();
        if self.selected.is_some() {
            let figures = state.figures.iter().enumerate();
            let placeable = figures.filter(|(_, f)| self.choice.units.contains(&f.unit));
            out.figures = placeable.map(|(i, _)| FigureId(i as u32)).collect();
        }
        out
    }
}

#[cfg(test)]
mod tests;
