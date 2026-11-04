//! The scrollable game log, from the events the viewer may see.

use bevy_egui::egui;
use strata_kernel::DefId;
use strata_model::Event;
use strata_session::Names;

fn seat(player: strata_model::PlayerId) -> u32 {
    u32::from(player.0) + 1
}

fn or_id(name: Option<&str>, id: DefId) -> String {
    name.map_or_else(|| format!("#{}", id.0), str::to_owned)
}

/// One line for `event`, or `None` for events a player does not need to read, such as variable
/// writes and slot bookkeeping. `names` words the dice, markers and phases it mentions.
pub fn describe(event: &Event, names: &Names) -> Option<String> {
    Some(match event {
        Event::PhaseEntered(phase) => {
            format!("Phase: {}", or_id(names.phase(*phase), *phase))
        }
        Event::RoundStarted(round) => format!("Round {round} begins"),
        Event::TurnStarted { turn, active } => format!("Turn {turn}: player {}", seat(*active)),
        Event::ActivationStarted { unit, .. } => format!("Unit {} activates", unit.0),
        Event::FigureMoved { figure, to, .. } => {
            let hex = to.hex();
            format!("Figure {} moves to {},{}", figure.0, hex.q, hex.r)
        }
        Event::DiceRolled { die, results } => {
            format!("{} rolled: {results:?}", or_id(names.die(*die), *die))
        }
        Event::WoundsApplied {
            figure,
            wound,
            amount,
        } => {
            let kind = or_id(names.marker(*wound), *wound);
            format!("Figure {} takes {amount} {kind}", figure.0)
        }
        Event::MarkerChanged {
            figure,
            marker,
            delta,
            ..
        } => {
            let kind = or_id(names.marker(*marker), *marker);
            format!("Figure {} {delta:+} {kind}", figure.0)
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
pub fn lines(events: &[Event], names: &Names) -> Vec<String> {
    events.iter().filter_map(|e| describe(e, names)).collect()
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
