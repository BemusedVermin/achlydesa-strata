//! The setup phase: everything before the first round (design §6.4).
//!
//! Game flow is a tree of phases: `setup`, then the rounds. `setup` holds the steps that ready
//! the board, such as `deploy`, which `core:deployment` registers along with `setup` itself. A
//! match begins with no phase open, so the first setup command opens `setup` and its first step.
//! When a step is done, [`finish_step`] moves on to the next; after the last one `setup` closes
//! and [`SETUP_ENDED`] is emitted, on which `core:rounds` starts round 1.

use strata_kernel::{Sym, Value};
use strata_model::{Event, GameState};
use strata_rules::{Anchor, Ctx, Halt, NativeEnv, PhaseDef, PhaseTree, ROOT, Registrar};

use crate::deploy_order::anyone_waiting;

/// The phase that holds every setup step.
pub const SETUP: &str = "setup";
/// The setup step in which players place their units.
pub const DEPLOY: &str = "deploy";
/// The custom event emitted when the last setup step is done and `setup` has closed.
pub const SETUP_ENDED: &str = "setup_ended";

/// Registers `setup` as a top-level phase with `deploy` as its step. The `deployment` slot
/// registers before the `turns` slot, so `setup` comes before `round` in the tree.
pub(crate) fn register(r: &mut Registrar) {
    let phase = |id: &str| PhaseDef { id: Sym::new(id) };
    r.phase(phase(SETUP), Anchor::Within(Sym::new(ROOT)));
    r.phase(phase(DEPLOY), Anchor::Within(Sym::new(SETUP)));
}

/// The phase on top of the stack.
pub(crate) fn current<'t>(state: &GameState, tree: &'t PhaseTree) -> Option<&'t Sym> {
    state.flow.phases.last().and_then(|f| tree.name(f.phase))
}

/// Whether the ruleset has a setup phase whose work is not done yet, so the first round must
/// wait: some unit still waits to be placed.
pub fn unfinished(state: &GameState, env: &dyn NativeEnv, tree: &PhaseTree) -> bool {
    let has_setup = tree.id(&Sym::new(SETUP)).is_some();
    has_setup && state.flow.round == 0 && anyone_waiting(state, env)
}

/// Opens `setup` and its first step, unless a phase is already open or the game has started.
pub(crate) fn open(ctx: &mut dyn Ctx, tree: &PhaseTree) -> Result<(), Halt> {
    let flow = &ctx.state().flow;
    if flow.round > 0 || !flow.phases.is_empty() {
        return Ok(());
    }
    tree.enter(ctx, &Sym::new(SETUP))?;
    tree.advance(ctx)
}

/// Closes the current setup step and opens the next one, or, after the last, closes `setup` and
/// emits [`SETUP_ENDED`].
pub(crate) fn finish_step(ctx: &mut dyn Ctx, tree: &PhaseTree) -> Result<(), Halt> {
    let top = current(ctx.state(), tree).cloned();
    let mut done = top.ok_or_else(|| Halt::reject("no setup step is open"))?;
    loop {
        let id = tree
            .id(&done)
            .ok_or_else(|| Halt::reject("unknown phase"))?;
        ctx.emit(Event::PhaseExited(id))?;
        if done.as_str() == SETUP {
            return ctx.emit(Event::Custom {
                id: Sym::new(SETUP_ENDED),
                payload: Value::Nil,
            });
        }
        if let Some(next) = tree.next_sibling(&done) {
            return tree.enter(ctx, next);
        }
        done = tree
            .parent(&done)
            .filter(|p| p.as_str() != ROOT)
            .ok_or_else(|| Halt::reject(format!("phase `{done}` is not inside `{SETUP}`")))?
            .clone();
    }
}
