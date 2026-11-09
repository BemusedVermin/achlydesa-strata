//! Actions written in Rust, run alongside the script actions of a ruleset (design §6.5).

use strata_kernel::Value;
use strata_model::{ActionDef, FigureId, GameState};

use super::{NativeEnv, native_name};
use crate::engine::{Ctx, Halt};
use crate::flow::ActionRunner;
use crate::runtime::RulesRuntime;

/// An action written in Rust.
///
/// A module registers it with [`Registrar::native_action`](crate::Registrar::native_action),
/// which records an [`ActionDef`] whose targeting and resolver are both the descriptor
/// `{ native = "<name>" }`. The ordinary attack is one.
pub trait NativeAction: Send + Sync + 'static {
    /// Every legal `args` value for `figure` taking the action.
    fn targets(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        figure: FigureId,
    ) -> Result<Vec<Value>, Halt>;
    /// Resolves the action for `figure` against `args`, emitting events through `ctx`.
    fn resolve(
        &self,
        ctx: &mut dyn Ctx,
        env: &dyn NativeEnv,
        figure: FigureId,
        args: &Value,
    ) -> Result<(), Halt>;
}

/// Runs native actions itself and hands every other action to the scripts' runner.
pub struct NativeActions<'a> {
    rt: &'a RulesRuntime,
    scripts: Option<&'a dyn ActionRunner>,
    env: &'a dyn NativeEnv,
}

impl<'a> NativeActions<'a> {
    /// Runs the native actions of `rt` with `env`, and the rest with `scripts`, if any.
    pub fn new(
        rt: &'a RulesRuntime,
        scripts: Option<&'a dyn ActionRunner>,
        env: &'a dyn NativeEnv,
    ) -> Self {
        Self { rt, scripts, env }
    }

    /// The native action behind `def`, if its resolver names one.
    fn native(&self, def: &ActionDef) -> Result<Option<&'a dyn NativeAction>, Halt> {
        let Some(name) = native_name(&def.resolve) else {
            return Ok(None);
        };
        let found = self.rt.native_action(name);
        let found = found.ok_or_else(|| Halt::reject(format!("no native action `{name}`")))?;
        Ok(Some(found))
    }

    /// The runner for script actions.
    fn scripts(&self) -> Result<&'a dyn ActionRunner, Halt> {
        self.scripts
            .ok_or_else(|| Halt::reject("no action runner is available"))
    }
}

impl ActionRunner for NativeActions<'_> {
    fn targets(
        &self,
        def: &ActionDef,
        figure: FigureId,
        state: &GameState,
    ) -> Result<Vec<Value>, Halt> {
        match self.native(def)? {
            Some(native) => native.targets(state, self.env, figure),
            None => self.scripts()?.targets(def, figure, state),
        }
    }

    fn resolve(
        &self,
        def: &ActionDef,
        figure: FigureId,
        args: &Value,
        ctx: &mut dyn Ctx,
    ) -> Result<(), Halt> {
        match self.native(def)? {
            Some(native) => native.resolve(ctx, self.env, figure, args),
            None => self.scripts()?.resolve(def, figure, args, ctx),
        }
    }

    fn native_env(&self) -> &dyn NativeEnv {
        self.env
    }
}
