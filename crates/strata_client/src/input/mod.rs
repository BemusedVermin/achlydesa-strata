//! Decision and command input (design §13.3, §13.5).
//!
//! What is highlighted, what a click means and why a click is refused are decided by the
//! Bevy-free [`builder`], [`decision`] and [`pick`] modules. The systems here only feed them the
//! newest view and the board's clicks, and send what they produce to the simulation.

use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use strata_model::{Answer, Command, GameState};
use strata_render::{BoardHighlights, SpaceClicked};

use crate::sim_link::{LatestView, SendAnswer, SendCommand};

pub mod builder;
pub mod decision;
pub mod pick;

use builder::CommandBuilder;
use decision::{Highlights, Widget};
use pick::{Pick, Refusal};

/// What the viewer's input produced for the simulation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    /// A command to submit.
    Command(Command),
    /// An answer to the pending decision.
    Answer(Answer),
}

/// The input state the board and the panels share.
#[derive(Resource, Clone, Debug, Default)]
pub struct CommandInput {
    /// The command being built.
    pub builder: CommandBuilder,
    /// The widget for the decision the viewer must answer, if any.
    pub widget: Option<Widget>,
    /// The stepper's value.
    pub number: i32,
    /// The ordering list, as indices of the items.
    pub order: Vec<u32>,
    /// Why the last click did nothing.
    pub why: Option<Refusal>,
    /// A reply went out and the next view has not arrived; further input waits for it.
    pub sent: bool,
}

impl CommandInput {
    /// Fresh input for the state `view` shows.
    pub fn of(view: &strata_session::PlayerView) -> Self {
        let pending = view.state.pending.as_ref();
        let widget = pending
            .filter(|p| p.chooser == view.viewer)
            .map(|p| Widget::of(&p.decision));
        let (number, order) = widget.as_ref().map(Widget::start).unwrap_or_default();
        Self {
            builder: CommandBuilder::new(view.viewer, view.legal.clone()),
            widget,
            number,
            order,
            why: None,
            sent: false,
        }
    }

    /// Applies a click: the pending decision takes it if there is one, the command builder
    /// otherwise. A refused click leaves its reason in [`why`](Self::why).
    pub fn click(&mut self, state: &GameState, pick: &Pick) -> Option<Reply> {
        if self.sent {
            return None;
        }
        let result = match &self.widget {
            Some(widget) => widget.click(pick).map(|a| Some(Reply::Answer(a))),
            None => self
                .builder
                .click(state, pick)
                .map(|c| c.map(Reply::Command)),
        };
        self.why = result.as_ref().err().copied();
        self.settle(result.ok().flatten())
    }

    /// Notes that `reply`, if any, is going out.
    pub fn settle(&mut self, reply: Option<Reply>) -> Option<Reply> {
        self.sent |= reply.is_some();
        reply
    }

    /// What the board marks: the decision's legal choices, or the builder's at its step.
    pub fn highlights(&self, state: &GameState) -> Highlights {
        match (&self.widget, self.sent) {
            (_, true) => Highlights::default(),
            (Some(widget), _) => widget.highlights(),
            (None, _) => self.builder.highlights(state),
        }
    }
}

/// What the board highlights now, figures included, for the panels and for tests.
#[derive(Resource, Default, Debug)]
pub struct Highlighted(pub Highlights);

/// Turns board clicks into commands and answers, and keeps the highlights current.
pub struct CommandInputPlugin;

impl Plugin for CommandInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CommandInput>()
            .init_resource::<Highlighted>()
            .init_resource::<BoardHighlights>()
            .add_message::<SpaceClicked>()
            .add_systems(Update, (refresh, on_click, highlight).chain());
    }
}

/// Starts over whenever a new view arrives.
fn refresh(view: Res<LatestView>, mut input: ResMut<CommandInput>) {
    if let Some(view) = view.0.as_deref().filter(|_| view.is_changed()) {
        *input = CommandInput::of(view);
    }
}

/// Sends what a reply asks for.
pub fn send(
    reply: Option<Reply>,
    commands: &mut MessageWriter<SendCommand>,
    answers: &mut MessageWriter<SendAnswer>,
) {
    match reply {
        Some(Reply::Command(command)) => {
            commands.write(SendCommand(command));
        }
        Some(Reply::Answer(answer)) => {
            answers.write(SendAnswer(answer));
        }
        None => {}
    }
}

fn on_click(
    mut clicks: MessageReader<SpaceClicked>,
    egui: Option<Res<EguiWantsInput>>,
    view: Res<LatestView>,
    mut input: ResMut<CommandInput>,
    mut commands: MessageWriter<SendCommand>,
    mut answers: MessageWriter<SendAnswer>,
) {
    let over_panel = egui.is_some_and(|e| e.is_pointer_over_area());
    for SpaceClicked(space) in clicks.read() {
        let Some(view) = view.0.as_deref().filter(|_| !over_panel) else {
            continue;
        };
        let reply = input.click(&view.state, &Pick::at(&view.state, *space));
        send(reply, &mut commands, &mut answers);
    }
}

fn highlight(
    view: Res<LatestView>,
    input: Res<CommandInput>,
    mut shown: ResMut<Highlighted>,
    mut board: ResMut<BoardHighlights>,
) {
    let Some(state) = view.0.as_deref().map(|v| &v.state) else {
        return;
    };
    if view.is_changed() || input.is_changed() {
        let highlights = input.highlights(state);
        board.set_if_neq(BoardHighlights(highlights.board_spaces(state)));
        shown.0 = highlights;
    }
}

#[cfg(test)]
mod tests;
