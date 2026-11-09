//! Rule modules written in Lua (design §6.2).

use std::fmt::Display;

use mlua::{Function, Lua, Table, Value as LuaValue};
use strata_kernel::{DefId, ModuleId, PackId, SlotId, Value, WindowId};
use strata_model::{ActionDef, ModifierDef, ReactionDef, ReplacementDef, TriggerDef};
use strata_rules::{ConfigSchema, ModuleError, Registrar, RuleModule};

use crate::convert::{contents, from_lua, value_to_lua};
use crate::data::{DataStage, PackSource};
use crate::error::HostError;
use crate::freeze::{Def, FrozenRecord, freeze_registered};
use crate::host::{Limits, freeze_deep};

const REGISTRAR: &str = include_str!("registrar.lua");
/// The key under which a module's registrations sit, so their `ScriptRef` paths cannot collide
/// with fields an author wrote.
pub(crate) const REGISTERED: &str = "~registered";

/// A `rule_module` record as a [`RuleModule`].
///
/// Its `register` runs the Lua function in a throwaway state, so the module itself holds only
/// plain data and can be shared by every thread. Each registered function is stored as a
/// `ScriptRef`; a runtime state binds it again with [`RuntimeLua::bind_modules`].
///
/// [`RuntimeLua::bind_modules`]: crate::RuntimeLua::bind_modules
pub struct LuaModule {
    id: ModuleId,
    record: String,
    name: String,
    pack: PackId,
    own: DefId,
    slot: Option<SlotId>,
    depends: Vec<ModuleId>,
    schema: ConfigSchema,
    packs: Vec<PackSource>,
    limits: Limits,
}

impl LuaModule {
    /// Wraps a frozen `rule_module` record, or returns `None` for any other kind. `packs` are
    /// the sources the data stage runs to reach the module's function.
    pub fn new(frozen: &FrozenRecord, packs: &[PackSource], limits: Limits) -> Option<Self> {
        let def = strata_model::RuleModuleDef::from_frozen(&frozen.def)?;
        let record = &frozen.record;
        let name = record.id.split_once(':').map_or(&*record.id, |(_, n)| n);
        Some(Self {
            id: ModuleId::new(&record.id),
            record: record.id.clone(),
            name: name.to_owned(),
            pack: record.source.pack.clone(),
            own: def.id,
            slot: def.slot.clone(),
            depends: def.depends.clone(),
            schema: ConfigSchema::new(def.config.clone()),
            packs: packs.to_vec(),
            limits,
        })
    }
}

impl RuleModule for LuaModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn slot(&self) -> Option<SlotId> {
        self.slot.clone()
    }

    fn depends_on(&self) -> &[ModuleId] {
        &self.depends
    }

    fn config_schema(&self) -> &ConfigSchema {
        &self.schema
    }

    fn register(&self, r: &mut Registrar, cfg: &Value) -> Result<(), ModuleError> {
        let invalid = |e: &dyn Display| ModuleError::Invalid(format!("{}: {e}", self.id));
        let stage = DataStage::run_with(&self.packs, self.limits).map_err(|e| invalid(&e))?;
        let root = run_register(&stage, &self.record, cfg).map_err(|e| invalid(&e))?;
        let lua = stage.host().lua();
        let (pack, name) = (&self.pack, &self.name);
        let frozen =
            freeze_registered(lua, pack, name, self.own, &root).map_err(|e| invalid(&e))?;
        let list: Table = frozen.raw_get(REGISTERED).map_err(|e| invalid(&e))?;
        for entry in list.sequence_values::<Table>() {
            let entry = entry.map_err(|e| invalid(&e))?;
            let kind: String = entry.raw_get("kind").map_err(|e| invalid(&e))?;
            let def: LuaValue = entry.raw_get("def").map_err(|e| invalid(&e))?;
            add(r, lua, &kind, def).map_err(|e| invalid(&format!("`r.{kind}`: {e}")))?;
        }
        Ok(())
    }
}

/// Hands one registration to the registrar.
fn add(r: &mut Registrar, lua: &Lua, kind: &str, def: LuaValue) -> mlua::Result<()> {
    match kind {
        "action" => r.action(from_lua::<ActionDef>(lua, def)?),
        "modifier" => r.modifier(from_lua::<ModifierDef>(lua, def)?),
        "trigger" => r.trigger(from_lua::<TriggerDef>(lua, def)?),
        "replacement" => r.replacement(from_lua::<ReplacementDef>(lua, def)?),
        "reaction" => r.reaction(from_lua::<ReactionDef>(lua, def)?),
        _ => r.window(WindowId::new(&from_lua::<String>(lua, def)?)),
    }
    Ok(())
}

/// Calls the `register` function of the module record `id` with a registrar and `cfg`.
///
/// Returns `{ ["~registered"] = list }`, shaped like a record so that functions inside it are
/// named like the functions of any other definition.
pub(crate) fn run_register(stage: &DataStage, id: &str, cfg: &Value) -> Result<Table, HostError> {
    let host = stage.host();
    let lua = host.lua();
    let record = stage
        .table("rule_module", id)
        .ok_or_else(|| HostError::Script(format!("no rule module `{id}`")))?;
    let register: Function = contents(lua, &record).raw_get("register")?;
    let builder = host.load(&host.new_env()?, "registrar", REGISTRAR)?;
    let mut made = host.call(&builder, ())?;
    let (Some(LuaValue::Table(r)), Some(LuaValue::Table(list))) =
        (made.pop_front(), made.pop_front())
    else {
        return Err(HostError::Script("the registrar builder failed".into()));
    };
    let config = value_to_lua(lua, cfg)?;
    if let LuaValue::Table(table) = &config {
        freeze_deep(lua, table)?;
    }
    host.call(&register, (r, config))?;
    let root = lua.create_table()?;
    root.raw_set(REGISTERED, list)?;
    Ok(root)
}
