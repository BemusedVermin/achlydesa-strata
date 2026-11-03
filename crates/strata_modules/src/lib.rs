//! The default Rust rule module for every slot.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

use strata_rules::RuleModule;

/// Every native module the engine ships, ready to join a module catalog.
pub fn native_modules() -> Vec<Box<dyn RuleModule>> {
    Vec::new()
}
