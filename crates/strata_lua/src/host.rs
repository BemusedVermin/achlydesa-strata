//! The sandboxed Lua 5.4 host (design §9.3).

use std::cell::Cell;
use std::rc::Rc;

use mlua::{
    Error, Function, HookTriggers, IntoLuaMulti, Lua, LuaOptions, MultiValue, StdLib, Table,
    UserData, Value, VmState,
};

use crate::error::HostError;
use crate::lock::{self, FROZEN_KEY, PARTS_KEY, SANDBOX_CHUNK, Write};

const PRELUDE: &str = include_str!("prelude.lua");
/// The standard libraries, frozen once the sandbox is built.
const LIBRARIES: [&str; 4] = ["string", "table", "math", "utf8"];
/// Instructions between hook invocations; budgets are rounded up to a multiple of this.
const STEP: u32 = 100;

/// Per-state resource limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The most VM instructions a single [`LuaHost::call`] may run.
    pub instructions: u32,
    /// The most bytes the whole Lua state may allocate.
    pub memory: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            instructions: 1_000_000,
            memory: 64 * 1024 * 1024,
        }
    }
}

/// The sentinel scripts raise to abandon a call, such as when a decision is unanswered.
/// `pcall` and `xpcall` rethrow it, so scripts cannot swallow it.
#[derive(Clone, Copy, Debug)]
pub struct Halt;

impl UserData for Halt {}

/// A Lua state with only deterministic, side-effect-free libraries.
pub struct LuaHost {
    lua: Lua,
    raw_pcall: Function,
    used: Rc<Cell<u32>>,
    tripped: Rc<Cell<bool>>,
}

impl LuaHost {
    /// Creates a sandboxed state limited by `limits`.
    pub fn new(limits: Limits) -> Result<Self, HostError> {
        let lua = Lua::new_with(
            StdLib::STRING | StdLib::TABLE | StdLib::MATH | StdLib::UTF8,
            LuaOptions::default(),
        )?;
        let used = Rc::new(Cell::new(0u32));
        let tripped = Rc::new(Cell::new(false));

        let frozen = lua.create_table()?;
        let weak = lua.create_table()?;
        weak.set("__mode", "k")?;
        frozen.set_metatable(Some(weak))?;
        lua.set_named_registry_value(FROZEN_KEY, frozen.clone())?;

        let globals = lua.globals();
        let raw_pcall: Function = globals.get("pcall")?;
        let raw_xpcall: Function = globals.get("xpcall")?;
        let flag = tripped.clone();
        let is_fatal = lua.create_function(move |_, value: Value| {
            let halted = matches!(&value, Value::UserData(ud) if ud.is::<Halt>());
            Ok(halted || flag.get())
        })?;
        globals.set("Halt", lua.create_userdata(Halt)?)?;
        let lock = lua.create_function(|lua, value: Value| {
            let Value::Table(table) = value else {
                return Ok(Some("freeze expects a table".to_owned()));
            };
            match lock::lock(lua, &table, Write::Frozen) {
                Ok(()) => Ok(None),
                Err(HostError::Script(problem)) => Ok(Some(problem)),
                Err(other) => Err(lock::into_lua(other)),
            }
        })?;
        let location = lua.create_function(|lua, ()| Ok(lock::location(lua)))?;
        let parts: Table = lua.load(PRELUDE).set_name(SANDBOX_CHUNK).call((
            globals.clone(),
            raw_pcall.clone(),
            raw_xpcall,
            is_fatal,
            frozen,
            lock,
            location,
        ))?;
        lua.set_named_registry_value(PARTS_KEY, parts)?;
        for name in LIBRARIES {
            lock::lock(&lua, &globals.get::<Table>(name)?, Write::Frozen)?;
        }

        let budget = limits.instructions.div_ceil(STEP).max(1);
        let (count, flag) = (used.clone(), tripped.clone());
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(STEP),
            move |_, _| {
                count.set(count.get().saturating_add(1));
                if count.get() > budget {
                    flag.set(true);
                    return Err(Error::runtime("instruction budget exceeded"));
                }
                Ok(VmState::Continue)
            },
        )?;
        lua.set_memory_limit(limits.memory)?;
        Ok(Self {
            lua,
            raw_pcall,
            used,
            tripped,
        })
    }

    /// Returns the underlying state for registering APIs.
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Creates a per-pack environment holding a copy of the sandbox globals.
    pub fn new_env(&self) -> Result<Table, HostError> {
        let env = self.lua.create_table()?;
        for pair in self.lua.globals().pairs::<Value, Value>() {
            let (key, value) = pair?;
            env.raw_set(key, value)?;
        }
        env.raw_set("_G", env.clone())?;
        Ok(env)
    }

    /// Makes `table` read-only: later writes raise. Its own tables are left as they are.
    pub fn freeze(&self, table: &Table) -> Result<(), HostError> {
        lock::lock(&self.lua, table, Write::Frozen)
    }

    /// Makes the pack environment `env` read-only, with every table reachable from it through
    /// keys, values and `__index` fallbacks (design §9.3). Writing a global then raises an
    /// error naming the global, and writing into a reachable table raises the frozen-table
    /// error. Functions are left alone, so tables only their upvalues hold stay writable.
    ///
    /// Freezing runs in Rust and costs no VM instructions, so it never trips a budget.
    pub fn freeze_env(&self, env: &Table) -> Result<(), HostError> {
        lock::freeze_env(&self.lua, env)
    }

    /// Freezes `table` and every table reachable from it, including tables that were
    /// frozen before and the tables those hold.
    pub(crate) fn freeze_tree(&self, table: &Table) -> Result<(), HostError> {
        lock::freeze_deep(&self.lua, table)
    }

    /// Compiles source text as a function running in `env`.
    pub fn load(&self, env: &Table, name: &str, source: &str) -> Result<Function, HostError> {
        Ok(self
            .lua
            .load(source)
            .set_name(format!("={name}"))
            .set_mode(mlua::ChunkMode::Text)
            .set_environment(env.clone())
            .into_function()?)
    }

    /// Calls `function` with a fresh instruction budget.
    pub fn call(
        &self,
        function: &Function,
        args: impl IntoLuaMulti,
    ) -> Result<MultiValue, HostError> {
        self.used.set(0);
        self.tripped.set(false);
        let mut args = args.into_lua_multi(&self.lua)?;
        args.push_front(Value::Function(function.clone()));
        let mut out: MultiValue = self.raw_pcall.call(args)?;
        if matches!(out.pop_front(), Some(Value::Boolean(true))) {
            return Ok(out);
        }
        Err(match out.pop_front() {
            _ if self.tripped.get() => HostError::Budget,
            Some(Value::UserData(ud)) if ud.is::<Halt>() => HostError::Halt,
            Some(Value::String(text)) => HostError::Script(text.to_string_lossy()),
            Some(Value::Error(error)) => HostError::Script(plain(&error)),
            other => HostError::Script(format!(
                "error value of type {}",
                other.as_ref().map_or("nil", Value::type_name)
            )),
        })
    }
}

/// The message of an error raised by a Rust callback, without mlua's wrapper text.
fn plain(error: &Error) -> String {
    match error {
        Error::RuntimeError(message) => message.clone(),
        Error::CallbackError { cause, .. } => plain(cause),
        other => other.to_string(),
    }
}
