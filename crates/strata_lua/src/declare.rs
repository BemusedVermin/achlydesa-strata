//! Running a presentation file such as a pack's `assets.lua` (design §13.4, §13.8).
//!
//! These files are not part of the data stages: they never reach the content registry or the
//! content hash. They run in their own sandboxed state, with only the constructors the caller
//! names, and come back as plain [`Value`]s for the caller to type.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use mlua::Lua;
use strata_kernel::Value;

use crate::convert::lua_to_value;
use crate::error::HostError;
use crate::host::{Limits, LuaHost};

/// One constructor call, such as `sprite { ... }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    /// The constructor's name.
    pub kind: String,
    /// The table it was called with.
    pub value: Value,
    /// The line of the call.
    pub line: u32,
}

/// What a declaration file left behind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Declarations {
    /// The requested globals the file set, by name.
    pub globals: BTreeMap<String, Value>,
    /// Every constructor call, in the order the file made them.
    pub calls: Vec<Declaration>,
}

/// Runs `source`, named `name` in error messages, with a global constructor for each of `kinds`,
/// then reads back the `globals` it set. Calling anything else is a script error.
pub fn declare(
    name: &str,
    source: &str,
    kinds: &[&str],
    globals: &[&str],
) -> Result<Declarations, HostError> {
    let host = LuaHost::new(Limits::default())?;
    let env = host.new_env()?;
    let calls = Rc::new(RefCell::new(Vec::new()));
    for kind in kinds {
        let (calls, name) = (calls.clone(), (*kind).to_owned());
        let constructor = host.lua().create_function(move |lua, table: mlua::Value| {
            let line = line_of_caller(lua);
            let value = lua_to_value(lua, &table)
                .map_err(|e| mlua::Error::runtime(format!("line {line}: {name}: {e}")))?;
            if !matches!(value, Value::Map(_)) {
                let message = format!("line {line}: {name} takes a table of named fields");
                return Err(mlua::Error::runtime(message));
            }
            let kind = name.clone();
            calls.borrow_mut().push(Declaration { kind, value, line });
            Ok(())
        })?;
        env.raw_set(*kind, constructor)?;
    }
    let chunk = host.load(&env, name, source)?;
    host.call(&chunk, ())?;
    let mut out = Declarations::default();
    for global in globals {
        let value: mlua::Value = env.raw_get(*global)?;
        if !value.is_nil() {
            out.globals
                .insert((*global).to_owned(), lua_to_value(host.lua(), &value)?);
        }
    }
    out.calls = calls.take();
    Ok(out)
}

/// The line of the Lua code that called the running Rust function, or 0.
fn line_of_caller(lua: &Lua) -> u32 {
    lua.inspect_stack(1, |frame| frame.current_line())
        .flatten()
        .unwrap_or(0) as u32
}

#[cfg(test)]
mod tests;
