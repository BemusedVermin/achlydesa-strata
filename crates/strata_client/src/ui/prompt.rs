//! The decision prompt: a thin egui view over [`crate::input`].
//!
//! Board choices are also listed as buttons, so a decision can be answered before the board is
//! drawn and figures in reserve can still be picked.

use bevy_egui::egui;
use strata_model::{Answer, FigureId};
use strata_session::PlayerView;

use crate::input::DecisionInput;
use crate::input::decision::{Widget, reorder};
use crate::input::pick::Pick;

/// A figure as the prompt names it: its unit's name and its ID.
fn figure_label(view: &PlayerView, figure: FigureId) -> String {
    let unit = view.state.figures.get(figure.index()).map(|f| f.unit);
    let card = view.cards.iter().find(|c| Some(c.unit) == unit);
    match card {
        Some(card) => format!("{} · figure {}", card.name, figure.0),
        None => format!("Figure {}", figure.0),
    }
}

/// Draws the pending decision and returns the answer once given.
pub fn decision(ui: &mut egui::Ui, view: &PlayerView, input: &mut DecisionInput) -> Option<Answer> {
    let (Some(widget), Some(pending)) = (input.widget.clone(), view.state.pending.as_ref()) else {
        return None;
    };
    ui.heading(pending.decision.prompt.as_str());
    if pending.chooser != view.viewer {
        ui.label(format!(
            "Player {} decides.",
            u32::from(pending.chooser.0) + 1
        ));
    }
    let mut answer = None;
    match &widget {
        Widget::Figures(figures) => {
            ui.label("Pick a highlighted figure.");
            for figure in figures {
                if ui.button(figure_label(view, *figure)).clicked() {
                    return input.click(&Pick::figure(*figure));
                }
            }
        }
        Widget::Spaces(spaces) => {
            ui.label("Pick a highlighted space.");
            for space in spaces {
                let pick = Pick {
                    space: Some(*space),
                    figure: None,
                };
                let label = format!("Space {},{} level {}", space.q, space.r, space.z);
                if ui.button(label).clicked() {
                    return input.click(&pick);
                }
            }
        }
        Widget::Options(options) => {
            for (i, option) in options.iter().enumerate() {
                if ui.button(option).clicked() {
                    answer = Some(Answer::Option(i as u32));
                }
            }
        }
        Widget::YesNo => {
            ui.horizontal(|ui| {
                if ui.button("Yes").clicked() {
                    answer = Some(Answer::YesNo(true));
                }
                if ui.button("No").clicked() {
                    answer = Some(Answer::YesNo(false));
                }
            });
        }
        Widget::Stepper { min, max } => {
            ui.add(egui::DragValue::new(&mut input.number).range(*min..=*max));
            if ui.button("Confirm").clicked() {
                answer = Some(Answer::Number(input.number));
            }
        }
        Widget::Order(items) => {
            ui.label("Drag the items into order, first at the top.");
            let mut moved = None;
            for (row, item) in input.order.iter().enumerate() {
                let label = items.get(*item as usize).cloned().unwrap_or_default();
                let id = egui::Id::new(("decision_order", row));
                let dragged = ui.dnd_drag_source(id, row, |ui| ui.label(label));
                if let Some(from) = dragged.response.dnd_release_payload::<usize>() {
                    moved = Some((*from, row));
                }
            }
            if let Some((from, to)) = moved {
                reorder(&mut input.order, from, to);
            }
            if ui.button("Confirm").clicked() {
                answer = Some(Answer::Order(input.order.clone()));
            }
        }
    }
    input.settle(answer)
}

/// Draws why the last click did nothing, if it did.
pub fn why(ui: &mut egui::Ui, input: &DecisionInput) {
    if let Some(why) = input.why {
        ui.colored_label(egui::Color32::LIGHT_RED, why.to_string());
    }
}
