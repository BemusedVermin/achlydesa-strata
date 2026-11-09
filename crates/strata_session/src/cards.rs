//! Unit cards: what a player reads off a unit, computed on the simulation thread where the
//! rules can be asked (design §13.1, §8.4).

use std::sync::Arc;

use strata_model::{FigureId, GameState, UnitId};
use strata_rules::QueryTrace;

use crate::names::Names;

/// One stat on a card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatLine {
    /// Short label, such as `ATK`.
    pub label: String,
    /// The printed value.
    pub base: i64,
    /// The value after every modifier.
    pub value: i64,
    /// How `value` was reached, for the rules inspector; `None` when the stat could not be
    /// queried and shows its printed value.
    pub trace: Option<QueryTrace>,
}

/// One ability on a card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AbilityLine {
    /// Display name.
    pub name: String,
    /// Card text, generated or overridden.
    pub text: String,
}

/// One figure of a unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FigureLine {
    /// The figure.
    pub figure: FigureId,
    /// Whether it is still in play, on the board or in reserve.
    pub alive: bool,
    /// Wounds on it.
    pub wounds: u32,
}

/// A unit's card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitCard {
    /// The unit this card describes.
    pub unit: UnitId,
    /// Display name.
    pub name: String,
    /// The unit's kind, such as `squad`.
    pub kind: String,
    /// Stats in display order.
    pub stats: Vec<StatLine>,
    /// Abilities with their card text.
    pub abilities: Vec<AbilityLine>,
    /// The unit's figures, in order.
    pub figures: Vec<FigureLine>,
}

/// Something that can write the card of every unit in a state.
pub trait CardSource {
    /// A card for each unit of `state` it knows, in unit order.
    fn cards(&self, state: &GameState) -> Vec<UnitCard>;

    /// Names for the IDs in events; none by default.
    fn names(&self) -> Arc<Names> {
        Arc::default()
    }
}

/// A source with no content, for runs that have no cards to show.
pub struct NoCards;

impl CardSource for NoCards {
    fn cards(&self, _: &GameState) -> Vec<UnitCard> {
        Vec::new()
    }
}
