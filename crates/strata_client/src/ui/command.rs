//! The command panel and the token placement screen: thin egui views over
//! [`CommandInput`].
//!
//! Every board choice is also listed as a button, so a command can be built before the board is
//! drawn.

use bevy_egui::egui;
use strata_kernel::DefId;
use strata_model::{Command, UnitId};
use strata_session::PlayerView;

use super::prompt::figure_label;
use crate::input::CommandInput;
use crate::input::builder::{Step, Target};
use crate::input::pick::Pick;
use crate::input::tokens::TokenPlacer;

/// What the panel asks for at `step`.
pub fn step_text(step: Step) -> &'static str {
    match step {
        Step::Idle => "Nothing to do. Waiting for the next turn.",
        Step::Unit => "Pick a unit to activate.",
        Step::Figure => "Pick a figure to act.",
        Step::Action(_) => "Pick an action.",
        Step::Target(..) => "Pick a highlighted target.",
        Step::End => "No figure can act. End the activation.",
    }
}

/// A token as the screen names it. `core:command_tokens` numbers its tokens and uses 0 for the
/// decoy.
pub fn token_label(kind: DefId) -> String {
    match kind.0 {
        0 => "Decoy".to_owned(),
        n => format!("Token {n}"),
    }
}

fn unit_label(view: &PlayerView, unit: UnitId) -> String {
    let card = view.cards.iter().find(|c| c.unit == unit);
    card.map_or_else(|| format!("Unit {}", unit.0), |c| c.name.clone())
}

fn target_label(view: &PlayerView, target: &Target) -> String {
    match target {
        Target::Figure(f) => figure_label(view, *f),
        Target::Space(s) => format!("Space {},{} level {}", s.q, s.r, s.z),
        Target::Other(value) => format!("{value:?}"),
    }
}

/// Draws the command being built and returns a command once one is complete.
pub fn command(ui: &mut egui::Ui, view: &PlayerView, input: &mut CommandInput) -> Option<Command> {
    let step = input.builder.step();
    ui.label(step_text(step));
    let mut done = None;
    match step {
        Step::Idle | Step::End => {}
        Step::Unit => {
            for unit in input.builder.units() {
                if ui.button(unit_label(view, unit)).clicked() {
                    done = input.builder.choose_unit(unit);
                }
            }
        }
        Step::Figure => {
            for figure in input.builder.actors() {
                if ui.button(figure_label(view, figure)).clicked() {
                    return input.click(&view.state, &Pick::figure(figure));
                }
            }
        }
        Step::Action(figure) => {
            ui.label(figure_label(view, figure));
            for action in input.builder.actions() {
                let name = view.names.action(action);
                let name = name.map_or_else(|| format!("Action {}", action.0), str::to_owned);
                if ui.button(name).clicked() {
                    done = input.builder.choose_action(action);
                }
            }
        }
        Step::Target(..) => {
            let options = input.builder.options();
            let labels: Vec<String> = options
                .iter()
                .map(|c| match c {
                    Command::Action { args, .. } => {
                        target_label(view, &Target::of(args, &view.state))
                    }
                    _ => String::new(),
                })
                .collect();
            for (i, label) in labels.into_iter().enumerate() {
                if ui.button(label).clicked() {
                    done = input.builder.choose_option(i);
                }
            }
        }
    }
    ui.horizontal(|ui| {
        let deeper = matches!(step, Step::Action(_) | Step::Target(..));
        if deeper && ui.button("Back").clicked() {
            input.builder.back();
        }
        if input.builder.can_end() && ui.button("End activation").clicked() {
            done = Some(Command::EndActivation);
        }
    });
    input.settle(done)
}

/// Draws the token placement screen and returns the placement once it is confirmed.
pub fn tokens(ui: &mut egui::Ui, view: &PlayerView, input: &mut CommandInput) -> Option<Command> {
    let placer = input.tokens.as_mut()?;
    ui.label(
        "Put each token on a different unit. Pick a token, then a unit or one of its figures.",
    );
    hand(ui, view, placer);
    ui.separator();
    let mut pressed = None;
    for unit in placer.units().to_vec() {
        if ui.button(unit_label(view, unit)).clicked() {
            pressed = Some(placer.assign(unit));
        }
    }
    let mut done = None;
    ui.horizontal(|ui| {
        if ui.button("Clear").clicked() {
            placer.clear();
        }
        let ready = placer.command();
        if ui
            .add_enabled(ready.is_some(), egui::Button::new("Place tokens"))
            .clicked()
        {
            done = ready;
        }
    });
    if let Some(result) = pressed {
        input.why = result.err();
    }
    input.settle(done)
}

/// One row per token of the hand: its label, where it is so far, and a button to select it.
fn hand(ui: &mut egui::Ui, view: &PlayerView, placer: &mut TokenPlacer) {
    let rows: Vec<(DefId, Option<UnitId>)> = placer
        .hand()
        .iter()
        .copied()
        .zip(placer.assigned().iter().copied())
        .collect();
    for (i, (kind, unit)) in rows.into_iter().enumerate() {
        let on = unit.map_or_else(|| "in hand".to_owned(), |u| unit_label(view, u));
        let text = format!("{}: {on}", token_label(kind));
        if ui
            .selectable_label(placer.selected() == Some(i), text)
            .clicked()
        {
            placer.select(i);
        }
    }
}
