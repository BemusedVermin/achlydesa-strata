//! The text half of the previews: cost labels over the stops of a move, and a window with the
//! hovered stop's flags or the hovered attack's pools, sight and exact wound odds (design §13.6).
//! The lines and rings are drawn by `strata_render`'s `PreviewPlugin`.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};
use strata_kernel::{Frac, Space};
use strata_render::{ClientBoard, IsoCameraMarker};
use strata_scene::{LIFT, WoundOdds, percent, surface_point};
use strata_session::{MoveStop, PlayerView};

use super::prompt::figure_label;
use crate::input::preview::{AttackModel, PreviewModel};
use crate::sim_link::LatestView;

/// The line that says the pools and odds shown ignore reactions and abilities.
pub const BEFORE_REACTIONS: &str = "Before reactions and abilities:";

/// Exact fractions longer than this are shown as percentages alone.
const MAX_SHOWN_DENOMINATOR: i128 = 10_000;

/// A chance as a percentage, with the exact fraction when it is short enough to read.
pub fn chance_text(chance: Frac) -> String {
    let shown = format!("{:.1}%", percent(chance));
    if chance.denom() == 1 || chance.denom() > MAX_SHOWN_DENOMINATOR {
        shown
    } else {
        format!("{shown} ({}/{})", chance.numer(), chance.denom())
    }
}

/// What reaching a stop costs and causes, one line each.
pub fn stop_lines(stop: &MoveStop) -> Vec<String> {
    let mut lines = vec![format!("Move cost {}", stop.cost)];
    if stop.stops_here {
        lines.push("Entering this space ends the move.".into());
    }
    if stop.falls {
        lines.push("The figure falls on the last step.".into());
    }
    if stop.provokes_reprisal {
        lines.push("Leaving engagement on the way gives an enemy a reprisal.".into());
    }
    lines
}

/// One line per wound count, `wounds: chance`, then the chance of at least one wound.
pub fn odds_lines(odds: &WoundOdds) -> Vec<String> {
    let mut lines: Vec<String> = (0..=odds.max())
        .map(|n| {
            format!(
                "{n} wound{}: {}",
                if n == 1 { "" } else { "s" },
                chance_text(odds.chance(n))
            )
        })
        .collect();
    if odds.max() > 0 {
        lines.push(format!(
            "At least 1 wound: {}",
            chance_text(odds.at_least(1))
        ));
    }
    lines
}

/// The hovered attack, line by line: the pools, sight and cover, then the wound odds, which are
/// those before any reaction or ability changes the attack.
pub fn attack_lines(view: &PlayerView, attack: &AttackModel) -> Vec<String> {
    let pool = |p: &crate::input::preview::PoolLine, verb: &str| {
        format!("{} {} {verb} ({} counts)", p.dice, p.die, p.face)
    };
    let mut lines = vec![
        format!(
            "{} attacks {}",
            figure_label(view, attack.attacker),
            figure_label(view, attack.target)
        ),
        pool(&attack.attack, "attack"),
        pool(&attack.defense, "defense"),
    ];
    match attack.clear {
        Some(clear) => lines.push(format!(
            "In sight: {} of the target clear",
            chance_text(clear)
        )),
        None if !attack.ranged => lines.push("Adjacent: no line of sight needed.".into()),
        None => {}
    }
    if attack.cover {
        lines.push("Cover: +1 defense die.".into());
    }
    // The pools and odds are those the attack starts from: reactions in the combat windows and
    // special attacks may change them.
    lines.push(BEFORE_REACTIONS.into());
    match &attack.odds {
        Some(odds) => lines.extend(odds_lines(odds)),
        None => lines.push("No exact odds for these dice.".into()),
    }
    lines
}

/// Draws the cost labels and the preview window.
pub(crate) fn preview_ui(
    mut contexts: EguiContexts,
    model: Res<PreviewModel>,
    view: Res<LatestView>,
    board: Option<Res<ClientBoard>>,
    camera: Query<(&Camera, &GlobalTransform), With<IsoCameraMarker>>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if let (Some(board), Ok((camera, at))) = (board, camera.single()) {
        let orientation = board.0.orientation;
        let screen = |space: Space| {
            let world = surface_point(orientation, space.hex(), space.z) + Vec3::Y * LIFT;
            camera.world_to_viewport(at, world).ok()
        };
        labels(ctx, &model.costs, screen);
    }
    let Some(view) = view.0.as_deref() else {
        return Ok(());
    };
    let lines = match (&model.stop, &model.attack) {
        (Some(stop), _) => stop_lines(stop),
        (None, Some(attack)) => attack_lines(view, attack),
        (None, None) => return Ok(()),
    };
    egui::Window::new("Preview").show(ctx, |ui| {
        for line in lines {
            ui.label(line);
        }
    });
    Ok(())
}

/// Paints each cost where `screen` puts its space, behind every window.
fn labels(ctx: &egui::Context, costs: &[(Space, i64)], screen: impl Fn(Space) -> Option<Vec2>) {
    let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("preview_costs"));
    let painter = ctx.layer_painter(layer);
    for &(space, cost) in costs {
        let Some(at) = screen(space) else { continue };
        painter.text(
            egui::pos2(at.x, at.y),
            egui::Align2::CENTER_CENTER,
            cost.to_string(),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
    }
}

#[cfg(test)]
mod tests;
