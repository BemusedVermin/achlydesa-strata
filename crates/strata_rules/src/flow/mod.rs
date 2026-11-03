//! Game flow: the phase tree, activations, slots and actions (design §6.4, §6.5, §12.4).

mod action;
mod activation;
mod legal;
mod phase;
mod tree;

pub use action::{ActionRunner, take_action};
pub use activation::{ActivationRules, OrderingPolicy};
pub use legal::{ActivationGate, Flow};
pub use tree::{FlowError, PhaseTree, ROOT};

#[cfg(test)]
mod tests;
