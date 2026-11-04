//! What one player may see of the state and of the events (design §15.3).

use std::sync::Arc;

use crate::cards::UnitCard;
use crate::names::Names;
use strata_kernel::DefId;
use strata_model::{Event, GameState, PlayerId, UnitId, Visibility};

/// The kind shown in place of a token the viewer may not see.
pub const REDACTED: DefId = DefId(u32::MAX);

/// A per-player projection of the state: opponents' hidden tokens keep their count but lose
/// their kind.
#[derive(Clone, Debug)]
pub struct PlayerView {
    /// Whose view this is.
    pub viewer: PlayerId,
    /// The state as the viewer sees it.
    pub state: GameState,
    /// A card per unit, with stats as the rules query them; empty when no content was supplied.
    pub cards: Vec<UnitCard>,
    /// Names of the dice, markers and phases that events mention; empty when no content was
    /// supplied.
    pub names: Arc<Names>,
}

impl PlayerView {
    /// Projects `state` for `viewer`.
    pub fn of(state: &GameState, viewer: PlayerId) -> Self {
        let mut state = state.clone();
        for unit in state.units.iter_mut().filter(|u| u.owner != viewer) {
            for token in unit.tokens.iter_mut() {
                if token.visibility == Visibility::Hidden {
                    token.kind = REDACTED;
                }
            }
        }
        for (seat, player) in state.players.iter_mut().enumerate() {
            if seat != viewer.index() {
                player.hand.iter_mut().for_each(|kind| *kind = REDACTED);
            }
        }
        Self {
            viewer,
            state,
            cards: Vec::new(),
            names: Arc::default(),
        }
    }
}

/// `events` as `viewer` may see them: an opponent's hidden token placement arrives with its
/// kind blanked out. `state` is the state the events were committed to.
pub fn visible_events(state: &GameState, viewer: PlayerId, events: &[Event]) -> Vec<Event> {
    let owned = |unit: UnitId| {
        let unit = state.units.get(unit.index());
        unit.is_some_and(|u| u.owner == viewer)
    };
    let show = |event: &Event| match event {
        Event::TokenPlaced { unit, token } if token.visibility == Visibility::Hidden => {
            let mut token = token.clone();
            if !owned(*unit) {
                token.kind = REDACTED;
            }
            Event::TokenPlaced { unit: *unit, token }
        }
        other => other.clone(),
    };
    events.iter().map(show).collect()
}
