//! Running the `test {}` records of a pack set against a ruleset (design §16.4).

mod export;
pub(crate) mod host;
mod report;
mod rig;
pub(crate) mod run;
mod state;

use std::collections::BTreeMap;
use std::path::PathBuf;

use strata_content::{ContentRegistry, discover_packs, load, resolve_load_order};
use strata_lua::{Limits, LuaModule, PackSource};
use strata_model::TestDef;
use strata_rules::{ModuleCatalog, RuleModule};

pub use export::ExportError;
pub use report::{Failure, TestReport, summary};
pub use rig::MatchRig;

/// The packs could not be loaded, so no test ran.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct PackTestError(pub String);

/// A loaded pack set whose tests can be run.
pub struct PackTests {
    pub(crate) registry: ContentRegistry,
    pub(crate) packs: Vec<PackSource>,
    pub(crate) catalog: ModuleCatalog,
}

impl PackTests {
    /// Loads the packs under `roots`; Lua rule modules are available to every test's ruleset.
    pub fn load(roots: &[PathBuf]) -> Result<Self, PackTestError> {
        let fail = |e: &dyn std::fmt::Display| PackTestError(e.to_string());
        let found = discover_packs(roots).map_err(|e| fail(&e))?;
        let order = resolve_load_order(&found).map_err(|e| fail(&e))?;
        let by_id: BTreeMap<_, _> = found.iter().map(|p| (&p.manifest.id, p)).collect();
        let packs: Vec<PackSource> = order
            .iter()
            .filter_map(|id| by_id.get(id))
            .map(|p| PackSource {
                id: p.manifest.id.clone(),
                root: p.root.clone(),
                dependencies: p.manifest.dependencies.keys().cloned().collect(),
            })
            .collect();
        let registry = load(roots).map_err(|errors| {
            let lines: Vec<String> = errors.iter().map(ToString::to_string).collect();
            PackTestError(lines.join("\n"))
        })?;
        let mut catalog = ModuleCatalog::default();
        for record in registry.records_of("rule_module") {
            let module = LuaModule::new(record, &packs, Limits::default());
            if let Some(module) = module {
                catalog.register(Box::new(module)).map_err(|e| fail(&e))?;
            }
        }
        Ok(Self {
            registry,
            packs,
            catalog,
        })
    }

    /// Makes a native module, such as one the engine ships, available to rulesets.
    pub fn with_native(mut self, module: Box<dyn RuleModule>) -> Result<Self, PackTestError> {
        let added = self.catalog.register(module);
        added.map_err(|e| PackTestError(e.to_string()))?;
        Ok(self)
    }

    /// Runs every test whose name contains `filter`, or all of them, in ID order.
    pub fn run(&self, filter: Option<&str>) -> Vec<TestReport> {
        let tests = self.registry.iter::<TestDef>();
        let wanted = |def: &TestDef| filter.is_none_or(|f| def.name.0.contains(f));
        let tests = tests.filter(|(_, _, def)| wanted(def));
        let run = |(_, id, def): (_, &str, &TestDef)| {
            let mut events = Vec::new();
            let outcome = self.run_one(id, def, &mut events);
            TestReport {
                name: def.name.0.clone(),
                outcome,
                events,
            }
        };
        tests.map(run).collect()
    }
}
