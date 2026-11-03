//! The default Rust rule module for every slot.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

pub mod rounds;
pub mod toy;
pub mod units;

use strata_rules::RuleModule;

/// Every native module the engine ships, ready to join a module catalog.
pub fn native_modules() -> Vec<Box<dyn RuleModule>> {
    vec![
        Box::new(toy::Driver),
        Box::new(toy::Combat),
        Box::new(units::Units),
        Box::new(rounds::Rounds),
    ]
}
