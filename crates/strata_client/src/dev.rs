//! `DevPlugin`: developer panels (design §13.3). So far the rules inspector (design §16.6): click
//! a stat on a unit card to see its base, each modifier with its source and whether it ran
//! natively or in Lua, the clamp and the result. The error overlay and hot reload come later.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use strata_model::UnitId;
use strata_session::{Inspection, PlayerView};

use crate::sim_link::LatestView;

/// The stat the inspector shows: a unit and the stat's label on its card.
#[derive(Resource, Default, Debug, PartialEq, Eq)]
pub struct Inspected(pub Option<(UnitId, String)>);

/// The rules inspector.
pub struct DevPlugin;

impl Plugin for DevPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inspected>()
            .add_systems(EguiPrimaryContextPass, inspector);
    }
}

/// The selected stat's trace in `view`. It is read from the newest view, so it follows the match.
pub fn inspection(view: &PlayerView, selected: &Inspected) -> Option<Inspection> {
    let (unit, label) = selected.0.as_ref()?;
    let card = view.cards.iter().find(|c| c.unit == *unit)?;
    let stat = card.stats.iter().find(|s| &s.label == label)?;
    stat.trace.as_ref().map(Inspection::of)
}

/// Draws the inspector while a stat is selected; closing the window clears the selection.
fn inspector(
    mut contexts: EguiContexts,
    view: Res<LatestView>,
    mut selected: ResMut<Inspected>,
) -> Result {
    if selected.0.is_none() {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?;
    let found = view.0.as_deref().and_then(|v| inspection(v, &selected));
    let mut open = true;
    egui::Window::new("Rules inspector")
        .open(&mut open)
        .show(ctx, |ui| show(ui, found.as_ref()));
    if !open {
        selected.0 = None;
    }
    Ok(())
}

/// Draws one inspection, with the steps that ran in Lua highlighted as the slow path.
pub fn show(ui: &mut egui::Ui, inspection: Option<&Inspection>) {
    let Some(inspection) = inspection else {
        ui.label("No trace: this stat shows its printed value.");
        return;
    };
    let lines = inspection.lines();
    // `lines` starts with the query and base lines, then one line per step.
    let steps = 2..2 + inspection.steps.len();
    for (n, line) in lines.iter().enumerate() {
        let lua = steps.contains(&n) && inspection.steps[n - 2].lua;
        match n {
            0 => ui.heading(line),
            _ if lua => ui.colored_label(egui::Color32::ORANGE, line),
            _ => ui.label(line),
        };
    }
    let slow = inspection.lua_steps();
    if slow > 0 {
        ui.small(format!("{slow} step(s) ran in Lua, the slow path."));
    }
}

#[cfg(test)]
mod tests;
