//! A unit's printed abilities: the `abilities` of its definition, attached to the unit as
//! [`AbilityKey::Unit`] instances (design §10.5).
//!
//! They are attached once, when the unit enters the match state: at match setup, or by whatever
//! adds a unit later, such as reinforcements. They belong to the unit, not to its figures, so
//! deploying from reserve, a figure being destroyed and a figure being restored neither attach
//! nor detach them. Rules that take a printed ability away emit `AbilityDetached` like any other.

use std::collections::BTreeMap;

use crate::ability_key::AbilityKey;
use crate::event::Event;
use crate::ids::UnitId;
use crate::state::{AbilityOwner, GameState};
use crate::unit::UnitDef;

impl GameState {
    /// The events that attach `unit`'s printed abilities, where `def` is the unit's definition:
    /// one `AbilityAttached` per entry of `def.abilities` that is not already attached to the
    /// unit, in entry order. Applying them twice is harmless, because the second call finds every
    /// entry attached. Empty if the state has no such unit.
    ///
    /// `AbilityKey` indexes entries with a `u16`, so at most the first 65,535 are attached; no
    /// real unit comes near that.
    pub fn printed_ability_events(&self, unit: UnitId, def: &UnitDef) -> Vec<Event> {
        let Some(state) = self.units.get(unit.index()) else {
            return Vec::new();
        };
        let owner = AbilityOwner::Unit(unit);
        let count = u16::try_from(def.abilities.len()).unwrap_or(u16::MAX);
        let attached = |key: AbilityKey| {
            let mut all = self.abilities.iter();
            all.any(|a| a.def == key && a.owner == owner)
        };
        let keys = (0..count).map(|index| AbilityKey::Unit {
            unit: state.def,
            index,
        });
        keys.filter(|key| !attached(*key))
            .map(|def| Event::AbilityAttached {
                def,
                owner,
                params: BTreeMap::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
