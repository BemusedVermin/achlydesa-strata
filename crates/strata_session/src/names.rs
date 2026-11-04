//! Display names for the definitions that events refer to by ID, built where the content lives
//! so the client can word its log without a registry (design §13.1).

use std::collections::BTreeMap;

use strata_content::ContentRegistry;
use strata_kernel::{DefId, Sym};
use strata_model::{DieDef, MarkerDef};

/// Names of dice, markers and phases by `DefId`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Names {
    dice: BTreeMap<DefId, String>,
    markers: BTreeMap<DefId, String>,
    phases: BTreeMap<DefId, String>,
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
        }
    }

    /// The name of the die `id`.
    pub fn die(&self, id: DefId) -> Option<&str> {
        self.dice.get(&id).map(String::as_str)
    }

    /// The name of the marker `id`, such as a wound.
    pub fn marker(&self, id: DefId) -> Option<&str> {
        self.markers.get(&id).map(String::as_str)
    }

    /// The name of the phase `id`.
    pub fn phase(&self, id: DefId) -> Option<&str> {
        self.phases.get(&id).map(String::as_str)
    }
}
