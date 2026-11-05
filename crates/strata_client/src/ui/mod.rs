//! `UiPlugin`: the `bevy_egui` panels (design §13.3). The panels draw view-models that are built
//! from the latest [`PlayerView`](strata_session::PlayerView) and the event log, so what they show
//! can be tested without a window.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};

use crate::dev::Inspected;
use crate::sim_link::{EventLog, LatestView};

pub mod cards;
pub mod log;
pub mod turn;

/// The unit cards, the turn tracker and the game log.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inspected>()
            .add_systems(EguiPrimaryContextPass, panels);
    }
}

/// Draws every panel from the newest view and log. A stat clicked on a unit card opens in the
/// rules inspector.
fn panels(
    mut contexts: EguiContexts,
    view: Res<LatestView>,
    log: Res<EventLog>,
    mut inspected: ResMut<Inspected>,
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
    }
    egui::Window::new("Log").show(ctx, |ui| log::show(ui, &log::lines(&log.0, &names)));
    Ok(())
}

#[cfg(test)]
mod tests;
