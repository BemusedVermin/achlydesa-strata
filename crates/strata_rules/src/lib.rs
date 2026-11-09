//! Slots, registrar, phases, actions, evaluators, triggers and the resolver.

#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]
#![warn(missing_docs)]

pub mod army;
pub mod catalog;
pub mod config;
pub mod descriptor;
pub mod engagement;
pub mod engine;
pub mod flow;
pub mod los;
pub mod module;
pub mod movement;
pub mod native;
pub mod query;
pub mod registrar;
pub mod resolve;
pub mod runtime;
pub mod slots;
pub mod victory;

pub use army::{ArmyNames, ArmyRules, ArmyUnit, ArmyViolation, NameKind, RuleOrigin};
pub use catalog::{DuplicateModule, ModuleCatalog};
pub use config::{ConfigError, ConfigSchema};
pub use descriptor::{DescriptorError, NativeEvaluator, Node, World, check_ability, compile};
pub use engagement::{Engagement, EngagementMemo};
pub use engine::{
    CommandRecord, Ctx, Engine, Halt, HookRunner, MAX_DECISIONS, Outcome, RuleViolation, event_kind,
};
pub use flow::{
    ActionRunner, ActivationGate, ActivationRules, Flow, FlowError, OrderingPolicy, PhaseTree,
    ROOT, TokenRules, take_action,
};
pub use los::{FigureVolume, LineOfSight, LosResult, LosWorld};
pub use module::{ModuleError, RuleModule};
pub use movement::{MoveWorld, Movement, Reach, TerrainRules, footprint_cells, passable_cells};
pub use native::{
    DeploySlot, DicePool, MAX_POOL, NativeAction, NativeActions, NativeEnv, NativeLogic, NoEnv,
    Roll, SCRIPT_LOGIC, logic_name, native_name, reroll, roll_pool,
};
pub use query::{
    EvalCtx, LogicError, LogicEvaluator, Logics, Query, QueryEngine, QueryError, QueryTrace,
    ScriptCaller, SourcedModifier, StatBounds, StatQuery, TraceStep, VoteQuery,
};
pub use registrar::{Anchor, ApiHandler, CommandHandler, LuaApi, PhaseDef, Registrar, Service};
pub use resolve::{ResolveError, RulesetSource, Site, resolve};
pub use runtime::RulesRuntime;
pub use victory::{MatchResult, Progress, Victory};

#[cfg(test)]
mod tests;
