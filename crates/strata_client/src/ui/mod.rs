//! `UiPlugin`: the `bevy_egui` panels (design §13.3). The panels draw view-models that are built
//! from the latest [`PlayerView`](strata_session::PlayerView) and the event log, so what they show
//! can be tested without a window.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::sim_link::{EventLog, LatestView};

pub mod cards;
pub mod log;
pub mod turn;

/// The unit cards, the turn tracker and the game log.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(EguiPrimaryContextPass, panels);
    }
}

/// Draws every panel from the newest view and log.
fn panels(mut contexts: EguiContexts, view: Res<LatestView>, log: Res<EventLog>) -> Result {
    let ctx = contexts.ctx_mut()?;
    let names = view
        .0
        .as_deref()
        .map(|v| v.names.clone())
        .unwrap_or_default();
    if let Some(view) = view.0.as_deref() {
        egui::Window::new("Turn").show(ctx, |ui| turn::show(ui, &turn::TurnModel::of(view)));
        egui::Window::new("Units").show(ctx, |ui| cards::show(ui, &cards::card_models(view)));
    }
    egui::Window::new("Log").show(ctx, |ui| log::show(ui, &log::lines(&log.0, &names)));
    Ok(())
}

#[cfg(test)]
mod tests;
