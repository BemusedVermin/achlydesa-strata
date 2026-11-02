//! Queries, modifiers and memoization (design §6.6).

mod engine;
mod logic;
mod modifier;
mod stat;
mod trace;
mod vote;

pub use engine::{QueryEngine, StatBounds};
pub use logic::{EvalCtx, LogicError, LogicEvaluator, Logics, QueryError, ScriptCaller};
pub use modifier::SourcedModifier;
pub use stat::StatQuery;
pub use trace::{QueryTrace, TraceStep};
pub use vote::VoteQuery;

use strata_kernel::{Sym, Value};

/// Identifies a question for the memo cache: query name, base or default, subject, context.
pub type QueryKey = (Sym, Value, Value, Value);

/// A question with a typed answer.
pub trait Query {
    /// The answer's type.
    type Output: Clone;

    /// The cache key; equal keys must give equal answers.
    fn key(&self) -> QueryKey;
    /// Resolves the query and records how.
    fn resolve(&self, engine: &QueryEngine<'_>) -> Result<(Self::Output, QueryTrace), QueryError>;
    /// Stores an answer in the cache.
    fn encode(output: &Self::Output) -> Value;
    /// Reads an answer back from the cache.
    fn decode(value: &Value) -> Option<Self::Output>;
}

#[cfg(test)]
mod tests;
