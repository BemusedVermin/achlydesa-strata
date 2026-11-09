//! `UiPlugin`: the `bevy_egui` panels (design §13.3). The panels draw view-models that are built
//! from the latest [`PlayerView`](strata_session::PlayerView) and the event log, so what they show
//! can be tested without a window.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::dev::Inspected;
use crate::input::DecisionInput;
use crate::sim_link::{EventLog, LatestView, SendAnswer};

pub mod cards;
pub mod log;
pub mod prompt;
pub mod turn;

/// The unit cards, the turn tracker, the game log and the decision prompt.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inspected>()
            .add_systems(EguiPrimaryContextPass, panels);
    }
}

/// Draws every panel from the newest view and log, and sends the prompt's answer. A stat clicked
/// on a unit card opens in the rules inspector.
pub(crate) fn panels(
    mut contexts: EguiContexts,
    view: Res<LatestView>,
    log: Res<EventLog>,
    mut inspected: ResMut<Inspected>,
    mut input: ResMut<DecisionInput>,
    mut answers: MessageWriter<SendAnswer>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    let names = view
        .0
        .as_deref()
        .map(|v| v.names.clone())
        .unwrap_or_default();
    if let Some(view) = view.0.as_deref() {
        egui::Window::new("Turn").show(ctx, |ui| turn::show(ui, &turn::TurnModel::of(view)));
        let mut clicked = None;
        egui::Window::new("Units").show(ctx, |ui| {
            clicked = cards::show(ui, &cards::card_models(view))
        });
        if clicked.is_some() {
            inspected.0 = clicked;
        }
        // Edit a copy, so the input is only marked changed when the prompt really changed it.
        let mut edited = input.clone();
        if edited.widget.is_some() {
            let mut answer = None;
            egui::Window::new("Decision").show(ctx, |ui| {
                if edited.sent {
                    ui.label("Waiting for the rules.");
                } else {
                    answer = prompt::decision(ui, view, &mut edited);
                }
                prompt::why(ui, &edited);
            });
            if let Some(answer) = answer {
                answers.write(SendAnswer(answer));
            }
        }
        input.set_if_neq(edited);
    }
    egui::Window::new("Log").show(ctx, |ui| log::show(ui, &log::lines(&log.0, &names)));
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
