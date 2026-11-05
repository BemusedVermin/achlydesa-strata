//! Decision input (design §13.3, §13.5).
//!
//! What is highlighted, what a click means and why a click is refused are decided by the
//! Bevy-free [`decision`] and [`pick`] modules. The systems here only feed them the newest view
//! and the board's clicks, and send the answers they produce to the simulation.

use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use strata_model::Answer;
use strata_render::{BoardHighlights, SpaceClicked};
use strata_session::PlayerView;

use crate::sim_link::{LatestView, SendAnswer};

pub mod decision;
pub mod pick;

use decision::{Highlights, Widget};
use pick::{Pick, Refusal};

/// The input state the board and the decision prompt share.
#[derive(Resource, Clone, Debug, Default)]
pub struct DecisionInput {
    /// The widget for the decision the viewer must answer, if any.
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
    /// Fresh input for the state `view` shows. Only a decision the viewer must answer gets a
    /// widget.
    pub fn of(view: &PlayerView) -> Self {
        let pending = view.state.pending.as_ref();
        let widget = pending
            .filter(|p| p.chooser == view.viewer)
            .map(|p| Widget::of(&p.decision));
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
    /// [`why`](Self::why).
    pub fn click(&mut self, pick: &Pick) -> Option<Answer> {
        if self.sent {
            return None;
        }
        let result = match &self.widget {
            Some(widget) => widget.click(pick),
            None => Err(Refusal::NothingToDo),
        };
        self.why = result.as_ref().err().copied();
        self.settle(result.ok())
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

/// What the board highlights now, figures included, for the panels and for tests.
#[derive(Resource, Default, Debug)]
pub struct Highlighted(pub Highlights);

/// Turns board clicks into answers and keeps the highlights current.
pub struct DecisionInputPlugin;

impl Plugin for DecisionInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DecisionInput>()
            .init_resource::<Highlighted>()
            .init_resource::<BoardHighlights>()
            .add_message::<SpaceClicked>()
            .add_systems(Update, (refresh, on_click, highlight).chain());
    }
}

/// Starts over whenever a new view arrives.
fn refresh(view: Res<LatestView>, mut input: ResMut<DecisionInput>) {
    if let Some(view) = view.0.as_deref().filter(|_| view.is_changed()) {
        *input = DecisionInput::of(view);
    }
}

fn on_click(
    mut clicks: MessageReader<SpaceClicked>,
    egui: Option<Res<EguiWantsInput>>,
    view: Res<LatestView>,
    mut input: ResMut<DecisionInput>,
    mut answers: MessageWriter<SendAnswer>,
) {
    let over_panel = egui.is_some_and(|e| e.is_pointer_over_area());
    for SpaceClicked(space) in clicks.read() {
        let Some(view) = view.0.as_deref().filter(|_| !over_panel) else {
            continue;
        };
        if let Some(answer) = input.click(&Pick::at(&view.state, *space)) {
            answers.write(SendAnswer(answer));
        }
    }
}

fn highlight(
    view: Res<LatestView>,
    input: Res<DecisionInput>,
    mut shown: ResMut<Highlighted>,
    mut board: ResMut<BoardHighlights>,
) {
    let Some(state) = view.0.as_deref().map(|v| &v.state) else {
        return;
    };
    if view.is_changed() || input.is_changed() {
        let highlights = input.highlights();
        board.set_if_neq(BoardHighlights(highlights.board_spaces(state)));
        shown.0 = highlights;
    }
}

#[cfg(test)]
mod tests;
