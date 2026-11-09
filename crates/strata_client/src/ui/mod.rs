//! `UiPlugin`: the `bevy_egui` panels (design §13.3). The panels draw view-models that are built
//! from the latest [`PlayerView`](strata_session::PlayerView) and the event log, so what they show
//! can be tested without a window.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::dev::Inspected;
use crate::input::{CommandInput, DecisionInput};
use crate::sim_link::{EventLog, LatestView, SendAnswer, SendCommand};

pub mod cards;
pub mod command;
pub mod log;
pub mod prompt;
pub mod turn;

/// The unit cards, the turn tracker, the game log, the decision prompt, the command panel and
/// the token placement screen.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inspected>()
            .add_systems(EguiPrimaryContextPass, panels);
    }
}

/// Where the panels send the answers and commands they produce.
#[derive(SystemParam)]
pub(crate) struct Outbox<'w> {
    answers: MessageWriter<'w, SendAnswer>,
    commands: MessageWriter<'w, SendCommand>,
}

/// Draws every panel from the newest view and log, and sends the prompt's answer or the command
/// panel's command. A stat clicked on a unit card opens in the rules inspector.
pub(crate) fn panels(
    mut contexts: EguiContexts,
    view: Res<LatestView>,
    log: Res<EventLog>,
    mut inspected: ResMut<Inspected>,
    mut input: ResMut<DecisionInput>,
    mut command: ResMut<CommandInput>,
    mut outbox: Outbox,
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
                outbox.answers.write(SendAnswer(answer));
            }
        }
        input.set_if_neq(edited);
        if input.widget.is_none()
            && let Some(built) = command_panel(ctx, view, &mut command)
        {
            outbox.commands.write(SendCommand(built));
        }
    }
    egui::Window::new("Log").show(ctx, |ui| log::show(ui, &log::lines(&log.0, &names)));
    Ok(())
}

/// Draws the token placement screen while the viewer has tokens to place, and the command panel
/// otherwise. Like the prompt, it edits a copy so the input only changes when the panel changed it.
fn command_panel(
    ctx: &egui::Context,
    view: &strata_session::PlayerView,
    input: &mut ResMut<CommandInput>,
) -> Option<strata_model::Command> {
    let mut edited = (**input).clone();
    let title = match edited.tokens {
        Some(_) => "Token placement",
        None => "Command",
    };
    let mut built = None;
    egui::Window::new(title).show(ctx, |ui| {
        if edited.sent {
            ui.label("Waiting for the rules.");
        } else if edited.tokens.is_some() {
            built = command::tokens(ui, view, &mut edited);
        } else {
            built = command::command(ui, view, &mut edited);
        }
        if let Some(why) = edited.why {
            ui.colored_label(egui::Color32::LIGHT_RED, why.to_string());
        }
    });
    input.set_if_neq(edited);
    built
}

#[cfg(test)]
pub(crate) mod tests;
