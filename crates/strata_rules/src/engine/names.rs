//! Names and owners of commands and events.

use strata_kernel::Sym;
use strata_model::{Answer, Command, Decision, DecisionKind, Event, GameState, PlayerId};

/// The name a handler registers for `command`.
pub(crate) fn command_kind(command: &Command) -> &str {
    match command {
        Command::PlaceTokens(_) => "place_tokens",
        Command::ActivateUnit(_) => "activate_unit",
        Command::Action { .. } => "action",
        Command::EndActivation => "end_activation",
        Command::Custom { id, .. } => id.as_ref(),
    }
}

/// The name that `ReplacementDef::event` and `TriggerDef::event` refer to.
pub(crate) fn event_kind(event: &Event) -> &str {
    match event {
        Event::FigureMoved { .. } => "figure_moved",
        Event::DiceRolled { .. } => "dice_rolled",
        Event::WoundsApplied { .. } => "wounds_applied",
        Event::FigureDestroyed(_) => "figure_destroyed",
        Event::MarkerChanged { .. } => "marker_changed",
        Event::TokenPlaced { .. } => "token_placed",
        Event::TokenRevealed { .. } => "token_revealed",
        Event::PhaseEntered(_) => "phase_entered",
        Event::PhaseExited(_) => "phase_exited",
        Event::RelicClaimed { .. } => "relic_claimed",
        Event::VarSet { .. } => "var_set",
        Event::ControlChanged { .. } => "control_changed",
        Event::Custom { id, .. } => id.as_ref(),
    }
}

/// Whether `name` is `kind`.
pub(crate) fn is_kind(name: &Sym, kind: &str) -> bool {
    let name: &str = name.as_ref();
    name == kind
}

/// The player an event happens to: a figure's controller, a unit's owner, or the active player.
pub(crate) fn affected_player(state: &GameState, event: &Event) -> PlayerId {
    let figure = match event {
        Event::FigureMoved { figure, .. }
        | Event::WoundsApplied { figure, .. }
        | Event::MarkerChanged { figure, .. }
        | Event::ControlChanged { figure, .. }
        | Event::FigureDestroyed(figure) => state.figures.get(figure.index()),
        _ => None,
    };
    let unit = match event {
        Event::TokenPlaced { unit, .. } | Event::TokenRevealed { unit, .. } => {
            state.units.get(unit.index())
        }
        _ => None,
    };
    match (event, figure, unit) {
        (Event::RelicClaimed { by, .. }, _, _) => *by,
        (_, Some(f), _) => f.controller,
        (_, _, Some(u)) => u.owner,
        _ => state.flow.active,
    }
}

/// Whether `answer` is a valid reply to `decision`.
pub(crate) fn answer_fits(decision: &Decision, answer: &Answer) -> bool {
    match (&decision.kind, answer) {
        (DecisionKind::ChooseFigure(options), Answer::Figure(f)) => options.contains(f),
        (DecisionKind::ChooseSpace(options), Answer::Space(s)) => options.contains(s),
        (DecisionKind::ChooseOption(options), Answer::Option(i)) => (*i as usize) < options.len(),
        (DecisionKind::YesNo, Answer::YesNo(_)) => true,
        (DecisionKind::Number { min, max }, Answer::Number(n)) => (min..=max).contains(&n),
        (DecisionKind::Order(items), Answer::Order(order)) => {
            let mut seen = vec![false; items.len()];
            order.len() == items.len()
                && order.iter().all(|&i| {
                    seen.get_mut(i as usize)
                        .is_some_and(|s| !std::mem::replace(s, true))
                })
        }
        _ => false,
    }
}
