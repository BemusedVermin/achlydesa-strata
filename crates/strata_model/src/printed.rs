//! A unit's printed abilities: the `abilities` of its definition, attached to the unit as
//! [`AbilityKey::Unit`] instances (design §10.5).
//!
//! They are attached once, when the unit enters the match state: at match setup, or by whatever
//! adds a unit later, such as reinforcements. They belong to the unit, not to its figures, so
//! deploying from reserve, a figure being destroyed and a figure being restored neither attach
//! nor detach them. Rules that take a printed ability away emit `AbilityDetached` like any other.

use std::collections::BTreeMap;

use strata_kernel::DefId;

use crate::ability_key::AbilityKey;
use crate::event::Event;
use crate::ids::UnitId;
use crate::state::{AbilityOwner, GameState};
use crate::unit::UnitDef;

impl GameState {
    /// The events that attach `unit`'s printed abilities: one `AbilityAttached` per entry of its
    /// definition's `abilities` that is not already attached to the unit, in entry order.
    /// `unit_def` looks the definition up by the `DefId` the state records for the unit, so the
    /// keys and the entries counted always come from the same definition. Applying the events
    /// twice is harmless, because the second call finds every entry attached. Empty if the state
    /// has no such unit or `unit_def` does not know its definition.
    ///
    /// Every entry is owned by the unit, whatever its scope: content load accepts only `unit`
    /// and `figure` scopes on printed abilities (design §10.5).
    ///
    /// `AbilityKey` indexes entries with a `u16`, so at most the first 65,535 are attached; no
    /// real unit comes near that.
    pub fn printed_ability_events<'d>(
        &self,
        unit: UnitId,
        unit_def: impl FnOnce(DefId) -> Option<&'d UnitDef>,
    ) -> Vec<Event> {
        let Some(state) = self.units.get(unit.index()) else {
            return Vec::new();
        };
        let Some(def) = unit_def(state.def) else {
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
