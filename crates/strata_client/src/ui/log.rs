//! The scrollable game log, from the events the viewer may see.

use bevy_egui::egui;
use strata_model::Event;

fn seat(player: strata_model::PlayerId) -> u32 {
    u32::from(player.0) + 1
}

/// One line for `event`, or `None` for events a player does not need to read, such as variable
/// writes and slot bookkeeping.
pub fn describe(event: &Event) -> Option<String> {
    Some(match event {
        Event::RoundStarted(round) => format!("Round {round} begins"),
        Event::TurnStarted { turn, active } => format!("Turn {turn}: player {}", seat(*active)),
        Event::ActivationStarted { unit, .. } => format!("Unit {} activates", unit.0),
        Event::FigureMoved { figure, to, .. } => {
            let hex = to.hex();
            format!("Figure {} moves to {},{}", figure.0, hex.q, hex.r)
        }
        Event::DiceRolled { results, .. } => format!("Dice rolled: {results:?}"),
        Event::WoundsApplied { figure, amount, .. } => {
            format!("Figure {} takes {amount} wound(s)", figure.0)
        }
        Event::FigureDestroyed(figure) => format!("Figure {} is destroyed", figure.0),
        Event::TokensDealt { player, tokens } => {
            format!("Player {} draws {} token(s)", seat(*player), tokens.len())
        }
        Event::TokenPlaced { unit, .. } => format!("A token is placed on unit {}", unit.0),
        Event::TokenRevealed { unit, .. } => format!("A token is revealed on unit {}", unit.0),
        Event::RelicClaimed { by, .. } => format!("Player {} claims a relic", seat(*by)),
        Event::ControlChanged { figure, controller } => {
            format!("Player {} takes figure {}", seat(*controller), figure.0)
        }
        _ => return None,
    })
}

/// The log lines for `events`, oldest first.
pub fn lines(events: &[Event]) -> Vec<String> {
    events.iter().filter_map(describe).collect()
}

/// Draws the log, pinned to the newest line.
pub fn show(ui: &mut egui::Ui, lines: &[String]) {
    let area = egui::ScrollArea::vertical().stick_to_bottom(true);
    area.show(ui, |ui| {
        for line in lines {
            ui.label(line);
        }
    });
}
