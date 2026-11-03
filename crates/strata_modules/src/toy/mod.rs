//! The toy ruleset's modules: a shared `attack` command and a swappable `combat` slot (design §17).
//!
//! The pack in `fixtures/packs/toy` supplies the same combat rules in Lua, so the M2 gate can
//! run one set of pack tests against either implementation.

mod combat;
mod driver;

use std::sync::OnceLock;

use strata_rules::ConfigSchema;

pub use combat::Combat;
pub use driver::Driver;

/// The event the driver emits and the combat slot reacts to.
pub(crate) const ATTACK_DECLARED: &str = "attack_declared";

/// The schema of a module that takes no options.
pub(crate) fn no_config() -> &'static ConfigSchema {
    static EMPTY: OnceLock<ConfigSchema> = OnceLock::new();
    EMPTY.get_or_init(ConfigSchema::default)
}
