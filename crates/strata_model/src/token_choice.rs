//! What a player may do with the tokens in hand (design §6.1, §12.5).
//!
//! Every way to place a hand is too many to list as commands, so the tokens module describes the
//! choice instead: the hand, and the units that may each take one token. The module's command
//! handler and the client check placements with the same [`TokenChoice::check`].

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use strata_kernel::DefId;

use crate::command::TokenPlacement;
use crate::ids::UnitId;

/// A player's token placement as the rules allow it: put every token of `hand` on a different
/// unit of `units`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenChoice {
    /// The tokens to place, by kind, in the order they were dealt.
    pub hand: Vec<DefId>,
    /// The units that may take a token, in ID order.
    pub units: Vec<UnitId>,
}

/// Why a token placement does not fit its [`TokenChoice`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BadPlacement {
    /// The placement does not put every token in hand, or the hand is empty.
    #[error("place every token in your hand, once")]
    WholeHand,
    /// A token goes on a unit that may not take one, or on a unit that already has one.
    #[error("each token goes on a different live unit of yours")]
    Unit,
    /// A token placed is not in the hand.
    #[error("that token is not in your hand")]
    Token,
}

impl TokenChoice {
    /// Checks that `put` places the whole hand, once each, on different units of the choice.
    pub fn check(&self, put: &[TokenPlacement]) -> Result<(), BadPlacement> {
        if self.hand.is_empty() || put.len() != self.hand.len() {
            return Err(BadPlacement::WholeHand);
        }
        let mut hand = self.hand.clone();
        let mut used = BTreeSet::new();
        for p in put {
            if !self.units.contains(&p.unit) || !used.insert(p.unit) {
                return Err(BadPlacement::Unit);
            }
            let at = hand.iter().position(|k| *k == p.token);
            hand.remove(at.ok_or(BadPlacement::Token)?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
