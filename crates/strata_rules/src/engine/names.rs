//! Names and owners of commands and events.

use strata_kernel::Sym;
use strata_model::{Answer, Command, Decision, DecisionKind, Event, FigureId, GameState, PlayerId};

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
pub fn event_kind(event: &Event) -> &str {
    match event {
        Event::FigureMoved { .. } => "figure_moved",
        Event::DiceRolled { .. } => "dice_rolled",
        Event::WoundsApplied { .. } => "wounds_applied",
        Event::FigureDestroyed(_) => "figure_destroyed",
        Event::FigureRemoved(_) => "figure_removed",
        Event::MarkerChanged { .. } => "marker_changed",
        Event::TokenPlaced { .. } => "token_placed",
        Event::TokenRevealed { .. } => "token_revealed",
        Event::TokensDealt { .. } => "tokens_dealt",
        Event::TokensDiscarded(_) => "tokens_discarded",
        Event::PhaseEntered(_) => "phase_entered",
        Event::PhaseExited(_) => "phase_exited",
        Event::ActivationStarted { .. } => "activation_started",
        Event::SlotsChanged { .. } => "slots_changed",
        Event::StepChanged(_) => "step_changed",
        Event::ActivationEnded => "activation_ended",
        Event::RelicClaimed { .. } => "relic_claimed",
        Event::RelicReleased(_) => "relic_released",
        Event::RelicRevealed(_) => "relic_revealed",
        Event::AbilityAttached { .. } => "ability_attached",
        Event::AbilityDetached { .. } => "ability_detached",
        Event::VarSet { .. } => "var_set",
        Event::ControlChanged { .. } => "control_changed",
        Event::RoundStarted(_) => "round_started",
        Event::TurnStarted { .. } => "turn_started",
        Event::Custom { id, .. } => id.as_ref(),
    }
}

/// Whether `name` is `kind`.
pub(crate) fn is_kind(name: &Sym, kind: &str) -> bool {
    let name: &str = name.as_ref();
    name == kind
}

/// The figure an event is about, if its own fields name one. Custom events and relic claims
/// name none here; [`event_subjects`] reads those.
pub fn event_figure(event: &Event) -> Option<FigureId> {
    match event {
        Event::FigureMoved { figure, .. }
        | Event::WoundsApplied { figure, .. }
        | Event::MarkerChanged { figure, .. }
        | Event::ControlChanged { figure, .. }
        | Event::SlotsChanged { figure, .. }
        | Event::FigureDestroyed(figure)
        | Event::FigureRemoved(figure) => Some(*figure),
        _ => None,
    }
}

/// Who `self` and `target` are for a hook that `event` sets off, which has no owner of its own.
///
/// - An event about one figure ([`event_figure`]) gives that figure as both.
/// - A relic claim gives the figure standing on the relic as both.
/// - A custom event whose payload is a map reads it by convention: `self` is its `figure` or else
///   its `attacker`, and `target` its `target` or else its `figure`. So core:combat's `attack`
///   event gives the attacker as `self` and the attacked figure as `target`.
/// - Any other event, such as `unit_removed`, gives neither. A selector that needs the missing
///   figure then selects nothing, and the effect does nothing to it.
pub fn event_subjects(state: &GameState, event: &Event) -> (Option<FigureId>, Option<FigureId>) {
    match event {
        Event::RelicClaimed { relic, .. } => {
            let at = state.relics.get(relic.index()).and_then(|r| r.location);
            let occupant = at.and_then(|at| state.occupant(at));
            (occupant, occupant)
        }
        Event::Custom { payload, .. } => {
            let figure = |key: &str| {
                let found = payload.as_map().ok()?.get(key)?.as_int().ok()?;
                u32::try_from(found).ok().map(FigureId)
            };
            let own = figure("figure").or_else(|| figure("attacker"));
            (own, figure("target").or_else(|| figure("figure")))
        }
        _ => {
            let figure = event_figure(event);
            (figure, figure)
        }
    }
}

/// The player an event happens to: a figure's controller, a unit's owner, or the active player.
pub(crate) fn affected_player(state: &GameState, event: &Event) -> PlayerId {
    let figure = match event {
        Event::FigureMoved { figure, .. }
        | Event::WoundsApplied { figure, .. }
        | Event::MarkerChanged { figure, .. }
        | Event::ControlChanged { figure, .. }
        | Event::FigureDestroyed(figure)
        | Event::FigureRemoved(figure) => state.figures.get(figure.index()),
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
