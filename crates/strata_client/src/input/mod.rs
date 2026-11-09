//! Decision and command input (design §13.3, §13.5).
//!
//! What is highlighted, what a click means and why a click is refused are decided by the
//! Bevy-free [`decision`], [`builder`], [`tokens`] and [`pick`] modules. The systems here only
//! feed them the newest view and the board's clicks, and send the answers and commands they
//! produce to the simulation. A pending decision takes clicks first; otherwise they build a
//! command.

use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use strata_model::Answer;
use strata_render::{BoardHighlights, SpaceClicked};
use strata_session::PlayerView;

use crate::sim_link::{LatestView, SendAnswer, SendCommand, SimStatus};

pub mod builder;
mod command;
pub mod decision;
pub mod pick;
pub mod tokens;

pub use command::{CommandInput, Press};

use decision::{Highlights, Widget};
use pick::{Pick, Refusal};

/// The input state the board and the decision prompt share.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct DecisionInput {
    /// The widget for the pending decision, if any.
    pub widget: Option<Widget>,
    /// The stepper's value.
    pub number: i32,
    /// The ordering list, as indices of the items.
    pub order: Vec<u32>,
    /// Why the last click did nothing.
    pub why: Option<Refusal>,
    /// An answer went out and the next view has not arrived; further input waits for it.
    pub sent: bool,
}

impl DecisionInput {
    /// Fresh input for the state `view` shows. Every seat is played from this client until
    /// hot-seat switching lands (T-056), so a pending decision gets a widget whoever chooses.
    pub fn of(view: &PlayerView) -> Self {
        let pending = view.state.pending.as_ref();
        let widget = pending.map(|p| Widget::of(&p.decision));
        let (number, order) = widget.as_ref().map(Widget::start).unwrap_or_default();
        Self {
            widget,
            number,
            order,
            why: None,
            sent: false,
        }
    }

    /// Applies a click to the pending decision. A refused click leaves its reason in
    /// [`why`](Self::why); with no decision pending, a click does nothing.
    pub fn click(&mut self, pick: &Pick) -> Option<Answer> {
        let widget = self.widget.as_ref().filter(|_| !self.sent)?;
        let result = widget.click(pick);
        self.why = result.as_ref().err().copied();
        self.settle(result.ok())
    }

    /// The reason the last click was refused, while it still applies: none once an answer has
    /// gone out.
    pub fn refusal(&self) -> Option<Refusal> {
        self.why.filter(|_| !self.sent)
    }

    /// Notes that `answer`, if any, is going out.
    pub fn settle(&mut self, answer: Option<Answer>) -> Option<Answer> {
        self.sent |= answer.is_some();
        answer
    }

    /// What the board marks: exactly the decision's legal figures or spaces, until it is
    /// answered.
    pub fn highlights(&self) -> Highlights {
        match (&self.widget, self.sent) {
            (Some(widget), false) => widget.highlights(),
            _ => Highlights::default(),
        }
    }
}

/// The figures and spaces that are highlighted: the pending decision's, or else what the command
/// being built allows. The board itself draws [`BoardHighlights`], which holds only spaces; this
/// keeps the figures too, for the panels and tests.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct Highlighted(pub Highlights);

/// Turns board clicks into answers and commands, and keeps the highlights current.
pub struct PlayerInputPlugin;

impl Plugin for PlayerInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DecisionInput>()
            .init_resource::<CommandInput>()
            .init_resource::<Highlighted>()
            .init_resource::<BoardHighlights>()
            .add_message::<SpaceClicked>()
            .add_message::<SendCommand>()
            .add_systems(Update, (refresh, on_click, highlight).chain());
    }
}

/// Starts over whenever a new view arrives, and clears the input once the match has stopped.
fn refresh(
    view: Res<LatestView>,
    status: Res<SimStatus>,
    mut input: ResMut<DecisionInput>,
    mut command: ResMut<CommandInput>,
) {
    if *status != SimStatus::Running {
        input.set_if_neq(DecisionInput::default());
        command.set_if_neq(CommandInput::default());
    } else if let Some(view) = view.0.as_deref().filter(|_| view.is_changed()) {
        *input = DecisionInput::of(view);
        *command = CommandInput::of(view);
    }
}

fn on_click(
    mut clicks: MessageReader<SpaceClicked>,
    egui: Option<Res<EguiWantsInput>>,
    view: Res<LatestView>,
    mut input: ResMut<DecisionInput>,
    mut command: ResMut<CommandInput>,
    mut answers: MessageWriter<SendAnswer>,
    mut commands: MessageWriter<SendCommand>,
) {
    let over_panel = egui.is_some_and(|e| e.is_pointer_over_area());
    for SpaceClicked(space) in clicks.read() {
        let Some(view) = view.0.as_deref().filter(|_| !over_panel) else {
            continue;
        };
        let pick = Pick::at(&view.state, *space);
        if input.widget.is_some() {
            if let Some(answer) = input.click(&pick) {
                answers.write(SendAnswer(answer));
            }
        } else if let Some(built) = command.click(&view.state, &pick) {
            commands.write(SendCommand(built));
        }
    }
}

fn highlight(
    view: Res<LatestView>,
    input: Res<DecisionInput>,
    command: Res<CommandInput>,
    mut shown: ResMut<Highlighted>,
    mut board: ResMut<BoardHighlights>,
) {
    let Some(state) = view.0.as_deref().map(|v| &v.state) else {
        return;
    };
    if view.is_changed() || input.is_changed() || command.is_changed() {
        let highlights = match input.widget {
            Some(_) => input.highlights(),
            None => command.highlights(state),
        };
        board.set_if_neq(BoardHighlights(highlights.board_spaces(state)));
        shown.set_if_neq(Highlighted(highlights));
    }
}

#[cfg(test)]
mod tests;
