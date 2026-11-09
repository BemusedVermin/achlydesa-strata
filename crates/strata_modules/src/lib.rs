//! The default Rust rule module for every slot.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

pub mod activation;
pub mod combat;
pub mod deploy_order;
pub mod deployment;
pub mod engagement;
pub mod los;
pub mod movement;
pub mod relics;
pub mod rounds;
pub mod setup;
mod shared;
pub mod tokens;
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
        Box::new(tokens::CommandTokens),
        Box::new(los::Los),
        Box::new(movement::MovementModule),
        Box::new(activation::Activation),
        Box::new(relics::Relics),
        Box::new(engagement::EngagementModule),
        Box::new(deployment::Deployment),
        Box::new(combat::CombatModule),
    ]
}
