//! The round, turn, active player and token status.

use bevy_egui::egui;
use strata_kernel::DefId;
use strata_session::{PlayerView, REDACTED};

/// A token as the viewer may see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenModel {
    /// Its kind is known: the definition ID.
    Known(u32),
    /// An opponent's hidden token.
    Hidden,
}

impl TokenModel {
    fn of(kind: DefId) -> Self {
        if kind == REDACTED {
            Self::Hidden
        } else {
            Self::Known(kind.0)
        }
    }
}

/// The tokens on one unit's card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenRow {
    /// The unit's display name.
    pub unit: String,
    /// Whether the viewer owns the unit.
    pub mine: bool,
    /// Its tokens, in placement order.
    pub tokens: Vec<TokenModel>,
}

/// Where the match is in its turn structure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnModel {
    /// The current round.
    pub round: u32,
    /// The turn within the round.
    pub turn: u32,
    /// Seat number of the active player, counted from 1.
    pub active: u32,
    /// Whether the viewer is the active player.
    pub my_turn: bool,
    /// Token kinds in the viewer's hand.
    pub hand: usize,
    /// Units that carry tokens.
    pub rows: Vec<TokenRow>,
}

impl TurnModel {
    /// The turn tracker for `view`.
    pub fn of(view: &PlayerView) -> Self {
        let state = &view.state;
        let name = |i: usize| {
            let card = view.cards.iter().find(|c| c.unit.index() == i);
            card.map_or_else(|| format!("Unit {i}"), |c| c.name.clone())
        };
        let carrying = state.units.iter().enumerate();
        let carrying = carrying.filter(|(_, u)| !u.tokens.is_empty());
        let rows = carrying.map(|(i, unit)| TokenRow {
            unit: name(i),
            mine: unit.owner == view.viewer,
            tokens: unit.tokens.iter().map(|t| TokenModel::of(t.kind)).collect(),
        });
        let player = state.players.get(view.viewer.index());
        Self {
            round: state.flow.round,
            turn: state.flow.turn,
            active: u32::from(state.flow.active.0) + 1,
            my_turn: state.flow.active == view.viewer,
            hand: player.map_or(0, |p| p.hand.len()),
            rows: rows.collect(),
        }
    }
}

/// Draws the tracker.
pub fn show(ui: &mut egui::Ui, turn: &TurnModel) {
    ui.label(format!("Round {} · Turn {}", turn.round, turn.turn));
    if turn.my_turn {
        ui.label("Active: you");
    } else {
        ui.label(format!("Active: player {}", turn.active));
    }
    ui.label(format!("Tokens in hand: {}", turn.hand));
    for row in &turn.rows {
        let owner = if row.mine { "yours" } else { "opponent" };
        let tokens: Vec<String> = row
            .tokens
            .iter()
            .map(|t| match t {
                TokenModel::Known(kind) => format!("#{kind}"),
                TokenModel::Hidden => "?".to_owned(),
            })
            .collect();
        ui.label(format!("{} ({owner}): {}", row.unit, tokens.join(" ")));
    }
}
