//! Display names for the definitions and actions that events refer to by ID, built where the content lives
//! so the client can word its log without a registry (design §13.1).

use std::collections::BTreeMap;

use strata_content::ContentRegistry;
use strata_kernel::{DefId, Sym};
use strata_model::{DieDef, MarkerDef};

/// Names of dice, markers, phases and actions by `DefId`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Names {
    dice: BTreeMap<DefId, String>,
    markers: BTreeMap<DefId, String>,
    phases: BTreeMap<DefId, String>,
    actions: BTreeMap<DefId, String>,
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
}
