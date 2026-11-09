//! Display names for the definitions and actions that events and commands refer to by ID, built
//! where the content lives so the client can word its log and panels without a registry (design
//! §13.1).

use std::collections::BTreeMap;

use strata_content::ContentRegistry;
use strata_kernel::{DefId, Sym};
use strata_model::{AbilityDef, AbilityKey, DieDef, MarkerDef, RelicDef, UnitDef};

/// Names of dice, markers, phases and actions by `DefId`, and of abilities by the stable name a
/// modifier's source carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Names {
    dice: BTreeMap<DefId, String>,
    markers: BTreeMap<DefId, String>,
    phases: BTreeMap<DefId, String>,
    actions: BTreeMap<DefId, String>,
    abilities: BTreeMap<String, String>,
}

impl Names {
    /// The names in `registry`, plus `phases`, whose position is their `DefId`.
    pub fn of(registry: &ContentRegistry, phases: &[Sym]) -> Self {
        let dice = registry
            .iter::<DieDef>()
            .map(|(id, _, d)| (id, d.name.0.clone()));
        let markers = registry
            .iter::<MarkerDef>()
            .map(|(id, _, m)| (id, m.name.0.clone()));
        let phases = (0u32..).zip(phases);
        Self {
            dice: dice.collect(),
            markers: markers.collect(),
            phases: phases
                .map(|(i, p)| (DefId(i), p.as_str().to_owned()))
                .collect(),
            actions: BTreeMap::new(),
            abilities: ability_names(registry),
        }
    }

    /// These names plus `actions`, whose position is their `DefId`.
    pub fn with_actions<'a>(mut self, actions: impl IntoIterator<Item = &'a Sym>) -> Self {
        let named = (0u32..).zip(actions);
        self.actions = named
            .map(|(i, a)| (DefId(i), a.as_str().to_owned()))
            .collect();
        self
    }

    /// The name of the die `id`.
    pub fn die(&self, id: DefId) -> Option<&str> {
        self.dice.get(&id).map(String::as_str)
    }

    /// The name of the marker `id`, such as a wound.
    pub fn marker(&self, id: DefId) -> Option<&str> {
        self.markers.get(&id).map(String::as_str)
    }

    /// The name of the action `id`, such as `move`.
    pub fn action(&self, id: DefId) -> Option<&str> {
        self.actions.get(&id).map(String::as_str)
    }

    /// The name of the phase `id`.
    pub fn phase(&self, id: DefId) -> Option<&str> {
        self.phases.get(&id).map(String::as_str)
    }

    /// The display name of the ability whose stable name is `source`, such as `Shieldwall` for
    /// `p:soldier/abilities/1`; `None` when `source` names no ability, as for a ruleset.
    pub fn ability(&self, source: &str) -> Option<&str> {
        self.abilities.get(source).map(String::as_str)
    }
}

/// The display name of every ability in `registry`, keyed by its stable name: ability records,
/// units' printed abilities and relics' granted abilities.
fn ability_names(registry: &ContentRegistry) -> BTreeMap<String, String> {
    let index = |len: usize| 0..u16::try_from(len).unwrap_or(u16::MAX);
    let records = registry
        .iter::<AbilityDef>()
        .map(|(id, _, _)| AbilityKey::Def(id));
    let units = registry.iter::<UnitDef>().flat_map(|(unit, _, def)| {
        let entries = index(def.abilities.len());
        entries.map(move |index| AbilityKey::Unit { unit, index })
    });
    let relics = registry.iter::<RelicDef>().flat_map(|(relic, _, def)| {
        let len = def.grants.as_ref().map_or(0, |g| g.abilities.len());
        index(len).map(move |index| AbilityKey::Relic { relic, index })
    });
    let keys = records.chain(units).chain(relics);
    let named = keys.filter_map(|key| registry.ability_with_name(key));
    named
        .map(|(def, name)| (name, def.name.0.clone()))
        .collect()
}
