//! Read-only tables (design §9.3).
//!
//! Freezing a table moves its entries into a fresh *shadow* table and gives the original, now
//! empty, table a metatable that reads from the shadow and rejects writes. The original keeps
//! its identity, so every reference to it sees the frozen view. The registry table at
//! [`FROZEN_KEY`] maps each frozen table to its shadow; [`contents`] reads it.
//!
//! Everything here runs in Rust through the C API, so freezing costs no VM instructions and
//! cannot trip a script's instruction budget, however large the table.
//!
//! A table may carry one kind of metatable when it is frozen: `{ __index = t }` with `t` a
//! table, a lookup fallback such as defaults or a parent. The shadow gets a fresh metatable
//! with that same `__index`, and a deep freeze freezes `t` as well, so a frozen table never
//! hands out anything writable. A function `__index` is refused, because Lua calls it with the
//! table it indexes, which would be the writable shadow; only the engine's own lookups, marked
//! with [`trust_index`], may use one.
//!
//! [`contents`]: crate::convert::contents

use std::collections::BTreeSet;

use mlua::{Function, Lua, Table, Value};

use crate::convert::contents;
use crate::error::HostError;

pub(crate) const FROZEN_KEY: &str = "strata.frozen";
/// The registry key of the shared metamethods a frozen table's metatable uses.
pub(crate) const PARTS_KEY: &str = "strata.frozen_parts";
/// The registry key of the set of engine functions allowed as a frozen table's `__index`.
const TRUSTED_KEY: &str = "strata.trusted_index";
/// The chunk name of the sandbox prelude, whose frames error locations skip.
pub(crate) const SANDBOX_CHUNK: &str = "=sandbox";
/// How far up the stack an error location is looked for.
const MAX_LEVELS: usize = 200;

/// What a frozen table does when a script writes to it.
#[derive(Clone, Copy)]
pub(crate) enum Write {
    /// Raises the frozen-table error.
    Frozen,
    /// Raises the error naming a global; for pack environments.
    Global,
}

impl Write {
    /// The field of the parts table that holds this `__newindex`.
    fn part(self) -> &'static str {
        match self {
            Self::Frozen => "frozen",
            Self::Global => "global",
        }
    }
}

/// Lets the engine function `index` serve as the `__index` of a table that is frozen later.
/// It is called with the shadow table and must only read it.
pub(crate) fn trust_index(lua: &Lua, index: &Function) -> mlua::Result<()> {
    let trusted = match lua.named_registry_value::<Option<Table>>(TRUSTED_KEY)? {
        Some(trusted) => trusted,
        None => {
            let trusted = lua.create_table()?;
            lua.set_named_registry_value(TRUSTED_KEY, &trusted)?;
            trusted
        }
    };
    trusted.raw_set(index, true)
}

/// Makes `table` read-only, keeping its entries. A table that is already frozen is left as it
/// is. A metatable other than `{ __index = <table> }` is refused with [`HostError::Script`].
pub(crate) fn lock(lua: &Lua, table: &Table, write: Write) -> Result<(), HostError> {
    let frozen: Table = lua.named_registry_value(FROZEN_KEY)?;
    if !frozen.raw_get::<Value>(table)?.is_nil() {
        return Ok(());
    }
    let index = match table.metatable() {
        Some(meta) => Some(fallback(lua, &meta)?),
        None => None,
    };
    let parts: Table = lua.named_registry_value(PARTS_KEY)?;
    let entries = table
        .pairs::<Value, Value>()
        .collect::<mlua::Result<Vec<_>>>()?;
    let shadow = lua.create_table_with_capacity(0, entries.len())?;
    for (key, value) in &entries {
        shadow.raw_set(key, value)?;
    }
    if let Some(index) = index {
        let meta = lua.create_table()?;
        meta.raw_set("__index", index)?;
        shadow.set_metatable(Some(meta))?;
    }
    let meta = lua.create_table()?;
    meta.raw_set("__index", &shadow)?;
    meta.raw_set("__newindex", parts.raw_get::<Function>(write.part())?)?;
    meta.raw_set("__len", parts.raw_get::<Function>("len")?)?;
    meta.raw_set("__metatable", false)?;
    table.set_metatable(Some(meta))?;
    for (key, _) in entries {
        table.raw_set(key, Value::Nil)?;
    }
    frozen.raw_set(table, shadow)?;
    Ok(())
}

/// The `__index` a frozen table may keep from its metatable `meta`, or why it may not.
fn fallback(lua: &Lua, meta: &Table) -> Result<Value, HostError> {
    let fields = contents(lua, meta)
        .pairs::<Value, Value>()
        .collect::<mlua::Result<Vec<_>>>()?;
    let index = match fields.as_slice() {
        [(Value::String(key), index)] if key.as_bytes() == b"__index" => index.clone(),
        _ => {
            return Err(HostError::Script(
                "cannot freeze a table with a metatable other than `{ __index = <table> }`"
                    .to_owned(),
            ));
        }
    };
    match &index {
        Value::Table(_) => Ok(index),
        Value::Function(function) if is_trusted(lua, function)? => Ok(index),
        other => Err(HostError::Script(format!(
            "cannot freeze a table with a metatable whose `__index` is a {}: only a table is \
             allowed, because a function would be handed the table's writable contents",
            other.type_name()
        ))),
    }
}

fn is_trusted(lua: &Lua, function: &Function) -> mlua::Result<bool> {
    let trusted: Option<Table> = lua.named_registry_value(TRUSTED_KEY)?;
    Ok(match trusted {
        Some(trusted) => trusted.raw_get::<Option<bool>>(function)?.is_some(),
        None => false,
    })
}

/// A table reachable from a frozen one: through a key, a value, or the `__index` fallback.
pub(crate) struct Child {
    /// How it is reached, for error messages: the key, or `__index`.
    pub label: String,
    /// The table.
    pub table: Table,
}

/// Orders the entries of a table so a walk, and the first error it reports, does not depend on
/// Lua's string hashing. Keys that are neither integers nor strings keep their raw order after
/// the rest; they are rare, and only matter when two tables fail to freeze.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Order {
    Int(i64),
    Str(Vec<u8>),
    Other(usize),
}

/// The tables directly reachable from the frozen table `table`, in a fixed order.
pub(crate) fn children(lua: &Lua, table: &Table) -> mlua::Result<Vec<Child>> {
    let shadow = contents(lua, table);
    let mut found = Vec::new();
    for (n, pair) in shadow.pairs::<Value, Value>().enumerate() {
        let (key, value) = pair?;
        let (order, label) = match &key {
            Value::Integer(i) => (Order::Int(*i), i.to_string()),
            Value::String(s) => (Order::Str(s.as_bytes().to_vec()), s.to_string_lossy()),
            other => (Order::Other(n), format!("<{}>", other.type_name())),
        };
        for item in [key, value] {
            if let Value::Table(table) = item {
                found.push((order.clone(), label.clone(), table));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out: Vec<Child> = found
        .into_iter()
        .map(|(_, label, table)| Child { label, table })
        .collect();
    if let Some(meta) = shadow.metatable() {
        if let Value::Table(index) = meta.raw_get::<Value>("__index")? {
            out.push(Child {
                label: "__index".to_owned(),
                table: index,
            });
        }
    }
    Ok(out)
}

/// Freezes tables and everything reachable from them, visiting each table once.
///
/// The walk keeps its own visited set, so a table frozen earlier (by `freeze`, or as part of
/// another tree) is still walked, and its children are frozen too.
pub(crate) struct DeepFreeze<'a> {
    lua: &'a Lua,
    visited: BTreeSet<usize>,
}

impl<'a> DeepFreeze<'a> {
    pub(crate) fn new(lua: &'a Lua) -> Self {
        Self {
            lua,
            visited: BTreeSet::new(),
        }
    }

    /// Marks `table` as walked without freezing it, so the walk does not enter it.
    pub(crate) fn skip(&mut self, table: &Table) {
        self.visited.insert(table.to_pointer() as usize);
    }

    /// Freezes `root` and every table reachable from it that this walk has not visited yet.
    pub(crate) fn freeze(&mut self, root: &Table) -> Result<(), HostError> {
        let mut pending = vec![root.clone()];
        while let Some(table) = pending.pop() {
            if !self.visited.insert(table.to_pointer() as usize) {
                continue;
            }
            lock(self.lua, &table, Write::Frozen)?;
            let children = children(self.lua, &table)?;
            pending.extend(children.into_iter().rev().map(|child| child.table));
        }
        Ok(())
    }
}

/// Freezes `table` and every table reachable from it.
pub(crate) fn freeze_deep(lua: &Lua, table: &Table) -> Result<(), HostError> {
    DeepFreeze::new(lua).freeze(table)
}

/// Makes the pack environment `env` read-only, with every table reachable from it. Writing a
/// global then raises an error naming it. Globals are walked in sorted order, and a failure
/// names the global it was reached from, so the same pack always fails the same way.
pub(crate) fn freeze_env(lua: &Lua, env: &Table) -> Result<(), HostError> {
    lock(lua, env, Write::Global).map_err(|error| in_place("the globals table", error))?;
    let mut walk = DeepFreeze::new(lua);
    walk.skip(env);
    for child in children(lua, env)? {
        walk.freeze(&child.table)
            .map_err(|error| in_place(&format!("global `{}`", child.label), error))?;
    }
    Ok(())
}

/// `error` as an error a Rust callback can raise into Lua.
pub(crate) fn into_lua(error: HostError) -> mlua::Error {
    match error {
        HostError::Lua(error) => error,
        HostError::Script(message) => mlua::Error::runtime(message),
        other => mlua::Error::runtime(other.to_string()),
    }
}

/// Prefixes a refusal with where the refused table was reached from.
fn in_place(place: &str, error: HostError) -> HostError {
    match error {
        HostError::Script(message) => HostError::Script(format!("{place}: {message}")),
        other => other,
    }
}

/// `file:line: ` of the innermost script frame on the stack, skipping C functions (such as
/// `table.insert`) and the sandbox prelude, or an empty string when there is none. Errors
/// raised by the sandbox start with it, so they name the script line that caused them.
pub(crate) fn location(lua: &Lua) -> String {
    for level in 1..MAX_LEVELS {
        let found = lua.inspect_stack(level, |frame| {
            let source = frame.source();
            let script = source.what != "C" && source.source.as_deref() != Some(SANDBOX_CHUNK);
            let line = frame.current_line()?;
            script.then(|| format!("{}:{line}: ", source.short_src.as_deref().unwrap_or("?")))
        });
        match found {
            None => break,
            Some(Some(text)) => return text,
            Some(None) => {}
        }
    }
    String::new()
}
