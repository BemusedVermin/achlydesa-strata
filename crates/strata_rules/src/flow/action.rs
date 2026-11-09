//! Taking an action: targeting, slot consumption, then the resolver (design §6.5).

use strata_kernel::Value;
use strata_model::{ActionDef, Command, FigureId, FigureStatus, GameState, PlayerId};

use super::activation::ActivationRules;
use crate::engine::{Ctx, Halt};
use crate::native::{NativeEnv, NoEnv};

/// Evaluates the targeting and resolver logic of actions.
///
/// The native evaluator (T-025) and the Lua host (T-027) provide the real one.
pub trait ActionRunner: Send + Sync {
    /// Every legal `args` value for `figure` taking `def`.
    fn targets(
        &self,
        def: &ActionDef,
        figure: FigureId,
        state: &GameState,
    ) -> Result<Vec<Value>, Halt>;
    /// Resolves `def` for `figure` against `args`, emitting events.
    fn resolve(
        &self,
        def: &ActionDef,
        figure: FigureId,
        args: &Value,
        ctx: &mut dyn Ctx,
    ) -> Result<(), Halt>;
    /// What native actions may ask of the content; by default nothing.
    fn native_env(&self) -> &dyn NativeEnv {
        &NoEnv
    }
}

/// Runs a `Command::Action` for `player`. An action's `DefId` is its index in `actions`.
///
/// It checks who may act, validates the targets, spends the slots, and only then resolves.
pub fn take_action(
    actions: &[ActionDef],
    rules: &ActivationRules,
    runner: &dyn ActionRunner,
    ctx: &mut dyn Ctx,
    player: PlayerId,
    command: &Command,
) -> Result<(), Halt> {
    let Command::Action {
        figure,
        action,
        args,
    } = command
    else {
        return Err(Halt::reject("not an action"));
    };
    let def = actions
        .get(action.0 as usize)
        .ok_or_else(|| Halt::reject("unknown action"))?;
    let state = ctx.state();
    let acting = state
        .flow
        .activation
        .as_ref()
        .zip(state.figures.get(figure.index()))
        .is_some_and(|(act, f)| {
            f.unit == act.unit && f.controller == player && f.status == FigureStatus::OnBoard
        });
    if !acting {
        return Err(Halt::reject("that figure cannot act now"));
    }
    if def.targets.is_some() && !runner.targets(def, *figure, state)?.contains(args) {
        return Err(Halt::reject("illegal target"));
    }
    rules.consume(ctx, *figure, &def.slots)?;
    runner.resolve(def, *figure, args, ctx)
}
