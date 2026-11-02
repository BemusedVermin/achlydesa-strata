//! Walking the phase tree and keeping the phase stack in step (design §6.4, §12.4).

use strata_kernel::Sym;
use strata_model::Event;

use super::tree::{PhaseTree, ROOT};
use crate::engine::{Ctx, Halt};

impl PhaseTree {
    /// Enters `phase`, pushing it on the phase stack.
    pub fn enter(&self, ctx: &mut dyn Ctx, phase: &Sym) -> Result<(), Halt> {
        let id = self
            .id(phase)
            .ok_or_else(|| Halt::reject(format!("unknown phase `{phase}`")))?;
        ctx.emit(Event::PhaseEntered(id))
    }

    /// Enters the first top-level phase.
    pub fn start(&self, ctx: &mut dyn Ctx) -> Result<(), Halt> {
        match self.first_child(&Sym::new(ROOT)) {
            Some(first) => self.enter(ctx, first),
            None => Err(Halt::reject("there are no phases")),
        }
    }

    /// Moves to the next phase: into the first child if the current phase has children,
    /// otherwise out of it and into its next sibling, leaving parents that are used up.
    /// Returns without entering anything once the last top-level phase has ended.
    pub fn advance(&self, ctx: &mut dyn Ctx) -> Result<(), Halt> {
        let top = ctx.state().flow.phases.last().map(|f| f.phase);
        let Some(mut done) = top.and_then(|id| self.name(id)).cloned() else {
            return Err(Halt::reject("no phase is active"));
        };
        if let Some(child) = self.first_child(&done) {
            return self.enter(ctx, child);
        }
        loop {
            let id = self
                .id(&done)
                .ok_or_else(|| Halt::reject("unknown phase"))?;
            ctx.emit(Event::PhaseExited(id))?;
            if let Some(next) = self.next_sibling(&done) {
                return self.enter(ctx, next);
            }
            match self.parent(&done) {
                Some(parent) if parent.as_str() != ROOT => done = parent.clone(),
                _ => return Ok(()),
            }
        }
    }
}
