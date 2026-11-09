//! The finished, immutable rules runtime.

use std::any::TypeId;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use strata_kernel::{ModuleId, SlotId, Sym, Value, WindowId};
use strata_model::{ActionDef, ModifierDef, ReactionDef, ReplacementDef, TriggerDef};

use crate::native::{NativeAction, NativeLogic};
use crate::query::VoteQuery;
use crate::registrar::{Anchor, ApiHandler, CommandHandler, LuaApi, PhaseDef, Registrar, Service};

/// Every registration of a resolved ruleset. It is plain data shared by all threads.
pub struct RulesRuntime {
    pub(crate) slots: BTreeMap<SlotId, ModuleId>,
    pub(crate) addons: Vec<ModuleId>,
    pub(crate) configs: Vec<(ModuleId, Value)>,
    pub(crate) reg: Registrar,
}

impl RulesRuntime {
    /// The module filling `slot`, or `None` if the slot is empty.
    pub fn slot(&self, slot: &SlotId) -> Option<&ModuleId> {
        self.slots.get(slot)
    }

    /// Active slot modules in slot order.
    pub fn slot_modules(&self) -> impl Iterator<Item = (&SlotId, &ModuleId)> {
        self.slots.iter()
    }

    /// Each active module with the configuration it was registered with, in registration order.
    pub fn configs(&self) -> &[(ModuleId, Value)] {
        &self.configs
    }

    /// Active add-on modules in registration order.
    pub fn addons(&self) -> &[ModuleId] {
        &self.addons
    }

    /// Registered phases with their anchors, in registration order.
    pub fn phases(&self) -> &[(PhaseDef, Anchor)] {
        &self.reg.phases
    }

    /// Registered command handlers.
    pub fn commands(&self) -> &[Box<dyn CommandHandler>] {
        &self.reg.commands
    }

    /// Registered actions.
    pub fn actions(&self) -> &[ActionDef] {
        &self.reg.actions
    }

    /// Registered query modifiers.
    pub fn modifiers(&self) -> &[ModifierDef] {
        &self.reg.modifiers
    }

    /// Registered triggers.
    pub fn triggers(&self) -> &[TriggerDef] {
        &self.reg.triggers
    }

    /// Registered replacements.
    pub fn replacements(&self) -> &[ReplacementDef] {
        &self.reg.replacements
    }

    /// Registered reactions.
    pub fn reactions(&self) -> &[ReactionDef] {
        &self.reg.reactions
    }

    /// Declared windows.
    pub fn windows(&self) -> &[WindowId] {
        &self.reg.windows
    }

    /// The keys of every pair query: the core ones in [`VoteQuery::PAIR_KEYS`] and any a
    /// module declared with [`Registrar::pair_query`].
    pub fn pair_queries(&self) -> BTreeSet<Sym> {
        let core = VoteQuery::PAIR_KEYS.iter().map(|k| Sym::new(k));
        core.chain(self.reg.pair_queries.iter().cloned()).collect()
    }

    /// The provider of service `S`, if any module registered one.
    pub fn service<S: ?Sized + Service>(&self) -> Option<&S> {
        let boxed = self.reg.services.get(&TypeId::of::<S>())?;
        boxed.downcast_ref::<Box<S>>().map(|b| &**b)
    }

    /// The Rust logic registered as `name`, if any module provided it.
    pub fn native(&self, name: &str) -> Option<&dyn NativeLogic> {
        self.reg.natives.get(name).map(|n| &**n)
    }

    /// The Rust action registered as `name`, if any module provided it.
    pub fn native_action(&self, name: &str) -> Option<&dyn NativeAction> {
        self.reg.native_actions.get(name).map(|n| &**n)
    }

    /// Every exported namespace with its API, in name order.
    pub fn lua_apis(&self) -> impl Iterator<Item = (&Sym, &LuaApi)> {
        self.reg.lua_apis.iter()
    }

    /// The Rust handler behind `ctx.<namespace>`, for a namespace exported with
    /// `Registrar::lua_api_native`.
    pub fn api_handler(&self, namespace: &str) -> Option<Arc<dyn ApiHandler>> {
        self.reg.api_handlers.get(namespace).cloned()
    }

    /// The API exported under `ctx.<namespace>`.
    pub fn lua_api(&self, namespace: &str) -> Option<&LuaApi> {
        self.reg.lua_apis.get(namespace)
    }
}
