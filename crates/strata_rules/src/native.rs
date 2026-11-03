//! Rust logic that a registered trigger runs in place of a Lua script.

use strata_model::{Descriptor, Event, Logic};

use crate::engine::{Ctx, Halt};

/// A trigger effect written in Rust.
///
/// A module registers it by name with [`Registrar::native`](crate::Registrar::native). A trigger
/// whose effect is the descriptor `{ native = "<name>" }` runs it, so a Rust module and a Lua
/// module can fill the same seam.
pub trait NativeLogic: Send + Sync + 'static {
    /// Runs for the event that set the trigger off, emitting events through `ctx`.
    fn run(&self, ctx: &mut dyn Ctx, event: &Event) -> Result<(), Halt>;
}

/// The name in a `{ native = "<name>" }` descriptor, or `None` for any other logic.
pub fn native_name(logic: &Logic) -> Option<&str> {
    let Logic::Native(Descriptor(value)) = logic else {
        return None;
    };
    let name = value.get("native").ok()?;
    name.as_str().ok()
}

#[cfg(test)]
mod tests;
