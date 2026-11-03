//! The bridge from registered replacements and triggers to their logic.

use strata_model::{Event, GameState, PlayerId, ReactionDef, ReplacementDef, TriggerDef};

use super::ctx::Ctx;
use super::halt::Halt;
use crate::native::{NativeEnv, NoEnv};

/// Evaluates the filters and effects of replacements and triggers.
///
/// The native evaluator (T-025) and the Lua host (T-027) provide the real one.
pub trait HookRunner: Send + Sync {
    /// Whether `def` applies to `event`.
    fn replaces(
        &self,
        def: &ReplacementDef,
        event: &Event,
        state: &GameState,
    ) -> Result<bool, Halt>;
    /// The events that take `event`'s place; empty cancels it.
    fn replace(
        &self,
        def: &ReplacementDef,
        event: &Event,
        ctx: &mut dyn Ctx,
    ) -> Result<Vec<Event>, Halt>;
    /// Whether `def` reacts to `event`.
    fn fires(&self, def: &TriggerDef, event: &Event, state: &GameState) -> Result<bool, Halt>;
    /// Runs the trigger's effect.
    fn fire(&self, def: &TriggerDef, event: &Event, ctx: &mut dyn Ctx) -> Result<(), Halt>;
    /// The player who may use `def` right now, or `None` if its `when` does not hold.
    fn reactor(&self, def: &ReactionDef, state: &GameState) -> Result<Option<PlayerId>, Halt>;
    /// Runs the reaction's effect.
    fn react(&self, def: &ReactionDef, ctx: &mut dyn Ctx) -> Result<(), Halt>;
    /// What native logic may ask of the content; by default nothing.
    fn env(&self) -> &dyn NativeEnv {
        &NoEnv
    }
}
