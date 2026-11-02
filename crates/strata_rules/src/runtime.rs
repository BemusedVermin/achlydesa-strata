//! The finished, immutable rules runtime.

use std::any::TypeId;
use std::collections::BTreeMap;

use strata_kernel::{ModuleId, SlotId, WindowId};
use strata_model::{ActionDef, ModifierDef, ReactionDef, ReplacementDef, TriggerDef};

use crate::registrar::{Anchor, CommandHandler, LuaApi, PhaseDef, Registrar, Service};

/// Every registration of a resolved ruleset. It is plain data shared by all threads.
pub struct RulesRuntime {
    pub(crate) slots: BTreeMap<SlotId, ModuleId>,
    pub(crate) addons: Vec<ModuleId>,
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

    /// The provider of service `S`, if any module registered one.
    pub fn service<S: ?Sized + Service>(&self) -> Option<&S> {
        let boxed = self.reg.services.get(&TypeId::of::<S>())?;
        boxed.downcast_ref::<Box<S>>().map(|b| &**b)
    }

    /// The API exported under `ctx.<namespace>`.
    pub fn lua_api(&self, namespace: &str) -> Option<&LuaApi> {
        self.reg.lua_apis.get(namespace)
    }
}
