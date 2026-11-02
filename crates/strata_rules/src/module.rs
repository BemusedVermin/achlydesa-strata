//! The rule module contract (design §6.2).

use strata_kernel::{ModuleId, SlotId, Value};

use crate::config::ConfigSchema;
use crate::registrar::Registrar;

/// A module could not register its hooks.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ModuleError {
    /// The module rejected its configuration or environment.
    #[error("{0}")]
    Invalid(String),
}

/// A unit of rules that fills a slot or adds hooks to a ruleset.
pub trait RuleModule: Send + Sync {
    /// The module's handle, such as `core:combat`.
    fn id(&self) -> ModuleId;
    /// The slot it fills; `None` for an add-on.
    fn slot(&self) -> Option<SlotId>;
    /// Modules that must be active alongside this one.
    fn depends_on(&self) -> &[ModuleId];
    /// The options it accepts.
    fn config_schema(&self) -> &ConfigSchema;
    /// Registers the module's hooks with the validated config.
    fn register(&self, r: &mut Registrar, cfg: &Value) -> Result<(), ModuleError>;
}
