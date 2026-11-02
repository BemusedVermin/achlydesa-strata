//! The registration surface modules write into (design §6.2).

use std::any::{Any, TypeId};
use std::collections::BTreeMap;

use strata_kernel::{Sym, WindowId};
use strata_model::{
    ActionDef, Command, ModifierDef, PlayerId, ReactionDef, ReplacementDef, TriggerDef,
};

use crate::engine::{Ctx, Halt};

/// A node in the phase tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhaseDef {
    /// The phase's name, such as `round_end`.
    pub id: Sym,
}

/// Where a phase is inserted relative to an existing one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Anchor {
    /// Immediately before the named phase.
    Before(Sym),
    /// Immediately after the named phase.
    After(Sym),
    /// As the last child of the named phase.
    Within(Sym),
}

/// Handles one kind of command.
pub trait CommandHandler: Send + Sync + 'static {
    /// The command kind handled.
    fn kind(&self) -> &str;
    /// Resolves `command` for `player` against the working copy, halting for decisions.
    fn resolve(&self, ctx: &mut dyn Ctx, player: PlayerId, command: &Command) -> Result<(), Halt>;
}

/// A shared capability a module offers, such as `dyn LineOfSight`.
///
/// Every `Send + Sync` type, including trait objects of such traits, is a service.
pub trait Service: Send + Sync + 'static {}
impl<T: ?Sized + Send + Sync + 'static> Service for T {}

/// The functions a module exports to scripts under one namespace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LuaApi {
    /// Names of the exported functions.
    pub functions: Vec<Sym>,
}

/// Collects every registration from the active modules.
#[derive(Default)]
pub struct Registrar {
    pub(crate) phases: Vec<(PhaseDef, Anchor)>,
    pub(crate) commands: Vec<Box<dyn CommandHandler>>,
    pub(crate) actions: Vec<ActionDef>,
    pub(crate) modifiers: Vec<ModifierDef>,
    pub(crate) triggers: Vec<TriggerDef>,
    pub(crate) replacements: Vec<ReplacementDef>,
    pub(crate) reactions: Vec<ReactionDef>,
    pub(crate) windows: Vec<WindowId>,
    pub(crate) services: BTreeMap<TypeId, Box<dyn Any + Send + Sync>>,
    pub(crate) lua_apis: BTreeMap<Sym, LuaApi>,
    conflicts: Vec<String>,
}

impl Registrar {
    /// Inserts a phase at `at`.
    pub fn phase(&mut self, p: PhaseDef, at: Anchor) {
        self.phases.push((p, at));
    }

    /// Adds a command handler.
    pub fn command<H: CommandHandler>(&mut self, h: H) {
        self.commands.push(Box::new(h));
    }

    /// Adds an action.
    pub fn action(&mut self, a: ActionDef) {
        self.actions.push(a);
    }

    /// Adds a query modifier.
    pub fn modifier(&mut self, m: ModifierDef) {
        self.modifiers.push(m);
    }

    /// Adds a trigger.
    pub fn trigger(&mut self, t: TriggerDef) {
        self.triggers.push(t);
    }

    /// Adds a replacement.
    pub fn replacement(&mut self, r: ReplacementDef) {
        self.replacements.push(r);
    }

    /// Adds a reaction, offered whenever its window opens.
    pub fn reaction(&mut self, r: ReactionDef) {
        self.reactions.push(r);
    }

    /// Declares a named window.
    pub fn window(&mut self, w: WindowId) {
        if self.windows.contains(&w) {
            self.conflicts
                .push(format!("window `{w}` is already declared"));
        } else {
            self.windows.push(w);
        }
    }

    /// Provides the service `S`. Only one module may provide each service.
    pub fn service<S: ?Sized + Service>(&mut self, s: Box<S>) {
        if self
            .services
            .insert(TypeId::of::<S>(), Box::new(s))
            .is_some()
        {
            self.conflicts.push(format!(
                "service `{}` is already provided",
                std::any::type_name::<S>()
            ));
        }
    }

    /// Exports `api` to scripts as `ctx.<namespace>`.
    pub fn lua_api(&mut self, namespace: &str, api: LuaApi) {
        if self.lua_apis.insert(Sym::new(namespace), api).is_some() {
            self.conflicts
                .push(format!("Lua namespace `{namespace}` is already exported"));
        }
    }

    /// Removes and returns the oldest recorded registration conflict.
    pub(crate) fn take_conflict(&mut self) -> Option<String> {
        (!self.conflicts.is_empty()).then(|| self.conflicts.remove(0))
    }
}
