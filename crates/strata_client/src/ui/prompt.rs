//! Decision prompts and the command panel: thin egui views over [`crate::input`].
//!
//! Board choices are also listed as buttons, so a decision can be answered before the board is
//! drawn and figures in reserve can still be picked.

use bevy_egui::egui;
use strata_model::{Answer, FigureId};
use strata_session::PlayerView;

use crate::input::builder::{Step, Target};
use crate::input::decision::{Widget, reorder};
use crate::input::pick::Pick;
use crate::input::{CommandInput, Reply};

/// A figure as the panels name it: its unit's name and its ID.
fn figure_label(view: &PlayerView, figure: FigureId) -> String {
    let unit = view.state.figures.get(figure.index()).map(|f| f.unit);
    let card = view.cards.iter().find(|c| Some(c.unit) == unit);
    match card {
        Some(card) => format!("{} · figure {}", card.name, figure.0),
        None => format!("Figure {}", figure.0),
    }
}

fn target_label(view: &PlayerView, target: &Target) -> String {
    match target {
        Target::Figure(f) => figure_label(view, *f),
        Target::Space(s) => format!("Space {},{} level {}", s.q, s.r, s.z),
        Target::Other(value) => format!("{value:?}"),
    }
}

/// Draws the decision the viewer must answer and returns the answer once given.
pub fn decision(ui: &mut egui::Ui, view: &PlayerView, input: &mut CommandInput) -> Option<Reply> {
    let (Some(widget), Some(pending)) = (input.widget.clone(), view.state.pending.as_ref()) else {
        return None;
    };
    ui.heading(pending.decision.prompt.as_str());
    let mut answer = None;
    match &widget {
        Widget::Figures(figures) => {
            ui.label("Pick a highlighted figure.");
            for figure in figures {
                if ui.button(figure_label(view, *figure)).clicked() {
                    return input.click(&view.state, &Pick::figure(*figure));
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
                if ui
                    .button(target_label(view, &Target::Space(*space)))
                    .clicked()
                {
                    return input.click(&view.state, &pick);
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
    input.settle(answer.map(Reply::Answer))
}

/// Draws the command being built and returns a command once one is complete.
pub fn command(ui: &mut egui::Ui, view: &PlayerView, input: &mut CommandInput) -> Option<Reply> {
    let builder = &mut input.builder;
    let mut reply = None;
    match builder.step() {
        Step::Idle => {
            ui.label("Waiting for the other players.");
        }
        Step::Unit => {
            ui.label("Pick a unit to activate.");
            for unit in builder.units() {
                let card = view.cards.iter().find(|c| c.unit == unit);
                let name = card.map_or_else(|| format!("Unit {}", unit.0), |c| c.name.clone());
                if ui.button(name).clicked() {
                    reply = Some(strata_model::Command::ActivateUnit(unit));
                }
            }
        }
        Step::Figure => {
            ui.label("Pick a figure to act.");
            for figure in builder.actors() {
                if ui.button(figure_label(view, figure)).clicked() {
                    return input.click(&view.state, &Pick::figure(figure));
                }
            }
        }
        Step::Action(figure) => {
            ui.label(format!("{}: pick an action.", figure_label(view, figure)));
            for action in builder.actions() {
                let name = view.names.action(action);
                let name = name.map_or_else(|| format!("Action #{}", action.0), str::to_owned);
                if ui.button(name).clicked() {
                    reply = builder.choose_action(action);
                }
            }
        }
        Step::Target(..) => {
            ui.label("Pick a highlighted target.");
            let options = builder.options();
            let targets: Vec<String> = options
                .iter()
                .map(|c| match c {
                    strata_model::Command::Action { args, .. } => {
                        target_label(view, &Target::of(args, &view.state))
                    }
                    _ => String::new(),
                })
                .collect();
            for (i, label) in targets.into_iter().enumerate() {
                if ui.button(label).clicked() {
                    reply = builder.choose_option(i);
                }
            }
        }
    }
    ui.horizontal(|ui| {
        if matches!(builder.step(), Step::Action(_) | Step::Target(..))
            && ui.button("Back").clicked()
        {
            builder.back();
        }
        if builder.can_end() && ui.button("End activation").clicked() {
            reply = Some(strata_model::Command::EndActivation);
        }
    });
    input.settle(reply.map(Reply::Command))
}

/// Draws why the last click did nothing, if it did.
pub fn why(ui: &mut egui::Ui, input: &CommandInput) {
    if let Some(why) = input.why {
        ui.colored_label(egui::Color32::LIGHT_RED, why.to_string());
    }
}
