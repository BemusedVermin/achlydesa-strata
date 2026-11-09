//! The command the viewer is building: a unit activation or an action from the legal commands,
//! or a token placement from the token choice.

use bevy::prelude::*;
use strata_kernel::DefId;
use strata_model::{Command, GameState, UnitId};
use strata_session::PlayerView;

use super::builder::CommandBuilder;
use super::decision::Highlights;
use super::pick::{Pick, Refusal};
use super::tokens::TokenPlacer;

/// A button pressed on the command panel or the token placement screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    /// Activate this unit.
    Unit(UnitId),
    /// Take this action with the selected figure.
    Action(DefId),
    /// Take the `n`th listed target.
    Option(usize),
    /// Step back from the action or the figure.
    Back,
    /// End the activation.
    End,
    /// Select the `n`th token of the hand.
    Token(usize),
    /// Put the selected token on this unit.
    Assign(UnitId),
    /// Take every token back into the hand.
    Clear,
    /// Send the finished token placement.
    Place,
}

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

    /// Applies a panel button. Every press replaces the reason the last click was refused, so an
    /// old refusal does not linger; a finished command is returned to be sent.
    pub fn press(&mut self, press: Press) -> Option<Command> {
        if self.sent {
            return None;
        }
        self.why = None;
        let builder = &mut self.builder;
        let done = match (press, &mut self.tokens) {
            (Press::Unit(unit), _) => builder.choose_unit(unit),
            (Press::Action(action), _) => builder.choose_action(action),
            (Press::Option(index), _) => builder.choose_option(index),
            (Press::Back, _) => {
                builder.back();
                None
            }
            (Press::End, _) => builder.can_end().then_some(Command::EndActivation),
            (Press::Token(token), Some(placer)) => {
                placer.select(token);
                None
            }
            (Press::Assign(unit), Some(placer)) => {
                self.why = placer.assign(unit).err();
                None
            }
            (Press::Clear, Some(placer)) => {
                placer.clear();
                None
            }
            (Press::Place, Some(placer)) => placer.command(),
            (Press::Token(_) | Press::Assign(_) | Press::Clear | Press::Place, None) => None,
        };
        self.settle(done)
    }

    /// The reason the last click was refused, while it still applies: none once a command has
    /// gone out.
    pub fn refusal(&self) -> Option<Refusal> {
        self.why.filter(|_| !self.sent)
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
