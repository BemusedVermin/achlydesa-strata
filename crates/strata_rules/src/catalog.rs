//! The built-in modules a ruleset can name.

use std::collections::BTreeMap;

use strata_kernel::ModuleId;

use crate::module::RuleModule;

/// A module ID was registered twice.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("module `{0}` is already in the catalog")]
pub struct DuplicateModule(pub ModuleId);

/// Rule modules by ID. Lua modules join later through the Lua host.
#[derive(Default)]
pub struct ModuleCatalog {
    modules: BTreeMap<ModuleId, Box<dyn RuleModule>>,
}

impl ModuleCatalog {
    /// Adds a module under its own ID.
    pub fn register(&mut self, module: Box<dyn RuleModule>) -> Result<(), DuplicateModule> {
        let id = module.id();
        if self.modules.contains_key(&id) {
            return Err(DuplicateModule(id));
        }
        self.modules.insert(id, module);
        Ok(())
    }

    /// Looks up a module by ID.
    pub fn get(&self, id: &ModuleId) -> Option<&dyn RuleModule> {
        self.modules.get(id).map(|m| &**m)
    }
}
