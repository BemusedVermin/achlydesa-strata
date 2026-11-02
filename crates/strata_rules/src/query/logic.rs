//! Evaluation of [`Logic`]: native descriptors in Rust, scripts through a caller.

use strata_kernel::{Sym, Value, ValueError};
use strata_model::{Descriptor, Logic, ScriptRef};

/// The inputs a condition, selector or value is evaluated against.
pub struct EvalCtx<'a> {
    /// The ability, terrain or relic the modifier came from.
    pub source: &'a Sym,
    /// The figure, unit or other thing being asked about.
    pub subject: &'a Value,
    /// Extra query inputs, such as the attacker.
    pub context: &'a Value,
}

/// A native or script evaluation failed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct LogicError(pub String);

/// Evaluates `Logic::Native` descriptors (implemented in T-025).
pub trait LogicEvaluator: Send + Sync {
    /// Evaluates `descriptor` to a value.
    fn eval(&self, descriptor: &Descriptor, cx: &EvalCtx<'_>) -> Result<Value, LogicError>;
}

/// Calls `Logic::Script` functions (implemented in T-027).
///
/// Not `Send` or `Sync`: a Lua state belongs to one thread.
pub trait ScriptCaller {
    /// Calls the script function and returns its result.
    fn call(&self, script: &ScriptRef, cx: &EvalCtx<'_>) -> Result<Value, LogicError>;
}

/// A query could not be resolved.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum QueryError {
    /// A descriptor or script failed.
    #[error("logic failed: {0}")]
    Logic(#[from] LogicError),
    /// Logic returned the wrong type of value.
    #[error("modifier from `{source_id}` returned the wrong type: {error}")]
    Type {
        /// The modifier's source.
        source_id: Sym,
        /// What was wrong.
        error: ValueError,
    },
    /// No bounds are known for the stat.
    #[error("unknown stat `{0}`")]
    UnknownStat(Sym),
}

/// Dispatches [`Logic`] to the evaluator or the script caller.
#[derive(Clone, Copy)]
pub struct Logics<'a> {
    /// Handles native descriptors.
    pub evaluator: &'a dyn LogicEvaluator,
    /// Handles script functions.
    pub scripts: &'a dyn ScriptCaller,
}

impl Logics<'_> {
    pub(crate) fn eval(&self, logic: &Logic, cx: &EvalCtx<'_>) -> Result<Value, LogicError> {
        match logic {
            Logic::Native(d) => self.evaluator.eval(d, cx),
            Logic::Script(s) => self.scripts.call(s, cx),
        }
    }
}
