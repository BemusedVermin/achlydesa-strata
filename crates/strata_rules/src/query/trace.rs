//! The record of how a query was answered, for the rules inspector (T-064).

use strata_kernel::{Sym, Value};
use strata_model::ModifierLayer;

use super::modifier::Applied;

/// One modifier's contribution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceStep {
    /// The modifier's source.
    pub source: Sym,
    /// The layer it applied in.
    pub layer: ModifierLayer,
    /// Its priority within the layer.
    pub priority: i32,
    /// What it contributed: an amount for stats, a vote for booleans.
    pub value: Value,
    /// Whether it was marked `override`.
    pub overrides: bool,
}

/// Inputs, each applied modifier in order, and the result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryTrace {
    /// The query name.
    pub query: Sym,
    /// The subject asked about.
    pub subject: Value,
    /// The extra inputs.
    pub context: Value,
    /// The base value or default vote.
    pub base: Value,
    /// Modifiers that applied, in application order.
    pub steps: Vec<TraceStep>,
    /// The value before clamping, for stats.
    pub unclamped: Option<i64>,
    /// The final answer.
    pub result: Value,
}

impl From<&Applied> for TraceStep {
    fn from(a: &Applied) -> Self {
        Self {
            source: a.source.clone(),
            layer: a.layer,
            priority: a.priority,
            value: a.value.clone(),
            overrides: a.overrides,
        }
    }
}
