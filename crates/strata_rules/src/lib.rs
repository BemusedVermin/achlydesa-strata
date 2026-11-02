//! Slots, registrar, phases, actions, evaluators, triggers and the resolver.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

pub mod catalog;
pub mod config;
pub mod descriptor;
pub mod module;
pub mod query;
pub mod registrar;
pub mod resolve;
pub mod runtime;
pub mod slots;

pub use catalog::{DuplicateModule, ModuleCatalog};
pub use config::{ConfigError, ConfigSchema};
pub use descriptor::{DescriptorError, Node, check_ability, compile};
pub use module::{ModuleError, RuleModule};
pub use query::{
    EvalCtx, LogicError, LogicEvaluator, Logics, Query, QueryEngine, QueryError, QueryTrace,
    ScriptCaller, SourcedModifier, StatBounds, StatQuery, TraceStep, VoteQuery,
};
pub use registrar::{Anchor, CommandHandler, LuaApi, PhaseDef, Registrar, Service};
pub use resolve::{ResolveError, RulesetSource, Site, resolve};
pub use runtime::RulesRuntime;

#[cfg(test)]
mod tests;
