//! The service that describes token placement (design §6.1, §12.5).

use strata_model::{GameState, PlayerId, TokenChoice};

/// Describes how a player may place the tokens in hand. A tokens module provides it as a service,
/// because the placements of a whole hand are too many to list as legal commands.
pub trait TokenRules: Send + Sync {
    /// The placement open to `player` now, or `None` when they have no tokens to place.
    fn choice(&self, state: &GameState, player: PlayerId) -> Option<TokenChoice>;
}
