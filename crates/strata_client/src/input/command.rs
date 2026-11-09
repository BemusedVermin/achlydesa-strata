//! The command the viewer is building: a unit activation or an action from the legal commands,
//! or a token placement from the token choice.

use bevy::prelude::*;
use strata_model::{Command, GameState};
use strata_session::PlayerView;

use super::builder::CommandBuilder;
use super::decision::Highlights;
use super::pick::{Pick, Refusal};
use super::tokens::TokenPlacer;

/// The input state the board and the command panel share.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandInput {
    /// The command being built from the legal commands.
    pub builder: CommandBuilder,
    /// The token placement, while the viewer has tokens to place.
    pub tokens: Option<TokenPlacer>,
    /// Why the last click did nothing.
    pub why: Option<Refusal>,
    /// A command went out and the next view has not arrived; further input waits for it.
    pub sent: bool,
}

impl CommandInput {
    /// Fresh input over what `view` offers its viewer.
    pub fn of(view: &PlayerView) -> Self {
        let tokens = view.tokens.clone();
        Self {
            builder: CommandBuilder::new(view.viewer, view.legal.clone()),
            tokens: tokens.map(|choice| TokenPlacer::new(view.viewer, choice)),
            why: None,
            sent: false,
        }
    }

    /// Applies a board click: it places the selected token while tokens are being placed, and
    /// moves the command builder on otherwise. A refused click leaves its reason in
    /// [`why`](Self::why); a finished command is returned to be sent.
    pub fn click(&mut self, state: &GameState, pick: &Pick) -> Option<Command> {
        if self.sent {
            return None;
        }
        let result = match &mut self.tokens {
            Some(tokens) => tokens.click(state, pick).map(|()| None),
            None => self.builder.click(state, pick),
        };
        self.why = result.as_ref().err().copied();
        self.settle(result.ok().flatten())
    }

    /// Notes that `command`, if any, is going out.
    pub fn settle(&mut self, command: Option<Command>) -> Option<Command> {
        self.sent |= command.is_some();
        command
    }

    /// What the board marks: the units that may take the selected token, or what the builder's
    /// step allows; nothing once a command has gone out.
    pub fn highlights(&self, state: &GameState) -> Highlights {
        match (&self.tokens, self.sent) {
            (_, true) => Highlights::default(),
            (Some(tokens), false) => tokens.highlights(state),
            (None, false) => self.builder.highlights(state),
        }
    }
}

#[cfg(test)]
mod tests;
