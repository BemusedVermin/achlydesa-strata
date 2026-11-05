//! The definition behind an [`AbilityKey`] (design §10.5, §12.1).

use strata_model::{AbilityDef, AbilityKey, RelicDef, UnitDef};

use crate::ContentRegistry;

impl ContentRegistry {
    /// The ability `key` names, or `None` if there is no such record or entry.
    pub fn ability(&self, key: AbilityKey) -> Option<&AbilityDef> {
        match key {
            AbilityKey::Def(id) => self.get::<AbilityDef>(id),
            AbilityKey::Unit { unit, index } => {
                let abilities = &self.get::<UnitDef>(unit)?.abilities;
                abilities.get(usize::from(index))
            }
            AbilityKey::Relic { relic, index } => {
                let abilities = &self.get::<RelicDef>(relic)?.grants.as_ref()?.abilities;
                abilities.get(usize::from(index))
            }
        }
    }

    /// A stable name for the ability `key` names, such as `t:shield` for a record or
    /// `t:soldier/abilities/1` for a unit's first ability. Inline abilities are named by their
    /// route, in the route grammar `ScriptRef` paths use; the prefix differs, since a
    /// `ScriptRef` path starts with the record kind and bare name rather than the full ID.
    pub fn ability_name(&self, key: AbilityKey) -> Option<String> {
        self.ability_with_name(key).map(|(_, name)| name)
    }

    /// The ability `key` names, with its [`ability_name`](Self::ability_name).
    pub fn ability_with_name(&self, key: AbilityKey) -> Option<(&AbilityDef, String)> {
        let ability = self.ability(key)?;
        let route = |record: &str, list: &str, index: u16| {
            format!("{record}/{list}/{}", u32::from(index) + 1)
        };
        let name = match key {
            AbilityKey::Def(id) => self.name::<AbilityDef>(id)?.to_owned(),
            AbilityKey::Unit { unit, index } => {
                route(self.name::<UnitDef>(unit)?, "abilities", index)
            }
            AbilityKey::Relic { relic, index } => {
                route(self.name::<RelicDef>(relic)?, "grants/abilities", index)
            }
        };
        Some((ability, name))
    }
}
