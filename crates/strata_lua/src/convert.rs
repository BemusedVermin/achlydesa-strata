//! Conversion between Lua values and the engine's `Value` (design §9.4).

use std::collections::BTreeMap;

use mlua::{DeserializeOptions, Lua, LuaSerdeExt, Table};
use serde::de::DeserializeOwned;
use strata_kernel::{Sym, Value};

use crate::error::ConvertError;
use crate::lock::FROZEN_KEY;

const MAX_DEPTH: usize = 64;

/// Converts a Lua value to a [`Value`], rejecting floats, NaN and functions.
///
/// A table with keys `1..=n` becomes a list (the empty table is an empty list) and a table
/// with only string keys becomes a map.
pub fn lua_to_value(lua: &Lua, value: &mlua::Value) -> Result<Value, ConvertError> {
    convert(lua, value, 0)
}

/// Converts a [`Value`] to a Lua value. `Value::Id` has no Lua form yet and is rejected.
pub fn value_to_lua(lua: &Lua, value: &Value) -> Result<mlua::Value, ConvertError> {
    let alloc = |_| ConvertError::Unsupported("lua allocation failure");
    Ok(match value {
        Value::Nil => mlua::Value::Nil,
        Value::Bool(b) => mlua::Value::Boolean(*b),
        Value::Int(i) => mlua::Value::Integer(*i),
        Value::Str(s) => mlua::Value::String(lua.create_string(s.as_str()).map_err(alloc)?),
        Value::Id(_) => return Err(ConvertError::Unsupported("id")),
        Value::List(items) => {
            let table = lua.create_table().map_err(alloc)?;
            for (i, item) in items.iter().enumerate() {
                table
                    .raw_set(i + 1, value_to_lua(lua, item)?)
                    .map_err(alloc)?;
            }
            mlua::Value::Table(table)
        }
        Value::Map(map) => {
            let table = lua.create_table().map_err(alloc)?;
            for (key, item) in map {
                table
                    .raw_set(key.as_str(), value_to_lua(lua, item)?)
                    .map_err(alloc)?;
            }
            mlua::Value::Table(table)
        }
    })
}

/// Deserialization that visits table keys in sorted order. Lua seeds string hashing per state,
/// so with raw order a table with two bad fields would report either one (design §9.4, §16.5).
pub(crate) fn sorted_keys() -> DeserializeOptions {
    DeserializeOptions::new().sort_keys(true)
}

/// Deserializes `value` into a `T`, visiting table keys in sorted order. Frozen tables are
/// read through their contents first: serde reads tables raw, and would see a frozen table as
/// empty.
pub(crate) fn from_lua<T: DeserializeOwned>(lua: &Lua, value: mlua::Value) -> mlua::Result<T> {
    lua.from_value_with(thaw(lua, value, 0)?, sorted_keys())
}

/// `value` with every frozen table in it, at any depth, replaced by a plain copy of its
/// contents. Tables that hold no frozen table are passed through, and the script's own tables
/// are never changed: a table whose children were replaced is copied too.
fn thaw(lua: &Lua, value: mlua::Value, depth: usize) -> mlua::Result<mlua::Value> {
    let mlua::Value::Table(table) = value else {
        return Ok(value);
    };
    if depth >= MAX_DEPTH {
        return Err(mlua::Error::runtime(ConvertError::TooDeep.to_string()));
    }
    let source = contents(lua, &table);
    let mut changed = source.to_pointer() != table.to_pointer();
    let mut entries = Vec::new();
    for pair in source.pairs::<mlua::Value, mlua::Value>() {
        let (key, item) = pair?;
        let thawed = thaw(lua, item.clone(), depth + 1)?;
        changed |= thawed.to_pointer() != item.to_pointer();
        entries.push((key, thawed));
    }
    if !changed {
        return Ok(mlua::Value::Table(table));
    }
    let copy = lua.create_table_with_capacity(0, entries.len())?;
    for (key, item) in entries {
        copy.raw_set(key, item)?;
    }
    Ok(mlua::Value::Table(copy))
}

fn convert(lua: &Lua, value: &mlua::Value, depth: usize) -> Result<Value, ConvertError> {
    Ok(match value {
        mlua::Value::Nil => Value::Nil,
        mlua::Value::Boolean(b) => Value::Bool(*b),
        mlua::Value::Integer(i) => Value::Int(*i),
        mlua::Value::Number(_) => return Err(ConvertError::Float),
        mlua::Value::Function(_) => return Err(ConvertError::Function),
        mlua::Value::String(s) => Value::Str(sym(s)?),
        mlua::Value::Table(_) if depth >= MAX_DEPTH => return Err(ConvertError::TooDeep),
        mlua::Value::Table(table) => convert_table(lua, table, depth)?,
        other => return Err(ConvertError::Unsupported(other.type_name())),
    })
}

fn sym(s: &mlua::String) -> Result<Sym, ConvertError> {
    Ok(Sym::new(&s.to_str().map_err(|_| ConvertError::NotUtf8)?))
}

/// The table that really holds `table`'s entries: a frozen table keeps them in a shadow table
/// (see prelude.lua).
pub(crate) fn contents(lua: &Lua, table: &Table) -> Table {
    let shadow = lua
        .named_registry_value::<Table>(FROZEN_KEY)
        .ok()
        .and_then(|frozen| {
            frozen
                .raw_get::<Option<Table>>(table.clone())
                .ok()
                .flatten()
        });
    shadow.unwrap_or_else(|| table.clone())
}

/// A table key, ordered integers before strings so every walk visits fields in the same order.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Key {
    /// An integer key.
    Int(i64),
    /// A string key.
    Str(String),
}

/// Why a table's entries could not be listed.
pub(crate) enum EntryError {
    /// Lua failed while iterating.
    Lua(mlua::Error),
    /// A key was neither an integer nor a string; holds its type name.
    BadKey(&'static str),
    /// A string key was not valid UTF-8.
    NotUtf8,
}

/// The entries of `table` (or of its frozen contents) sorted by [`Key`]. Lua seeds string
/// hashing per state, so raw `pairs()` order would make the first error found vary between
/// runs (design §9.4, §16.5).
pub(crate) fn sorted_entries(
    lua: &Lua,
    table: &Table,
) -> Result<Vec<(Key, mlua::Value)>, EntryError> {
    let mut entries = Vec::new();
    for pair in contents(lua, table).pairs::<mlua::Value, mlua::Value>() {
        let (key, value) = pair.map_err(EntryError::Lua)?;
        let key = match &key {
            mlua::Value::Integer(i) => Key::Int(*i),
            mlua::Value::String(s) => {
                Key::Str(s.to_str().map_err(|_| EntryError::NotUtf8)?.to_owned())
            }
            other => return Err(EntryError::BadKey(other.type_name())),
        };
        entries.push((key, value));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(entries)
}

fn convert_table(lua: &Lua, table: &Table, depth: usize) -> Result<Value, ConvertError> {
    let entries = sorted_entries(lua, table).map_err(|error| match error {
        EntryError::NotUtf8 => ConvertError::NotUtf8,
        EntryError::Lua(_) | EntryError::BadKey(_) => ConvertError::BadTable,
    })?;
    let mut list = BTreeMap::new();
    let mut map = BTreeMap::new();
    for (key, item) in entries {
        let item = convert(lua, &item, depth + 1)?;
        match key {
            Key::Int(i) => list.insert(i, item),
            Key::Str(s) => map.insert(Sym::new(&s), item),
        };
    }
    match (list.is_empty(), map.is_empty()) {
        (_, true) if list.keys().copied().eq(1..=list.len() as i64) => {
            Ok(Value::List(list.into_values().collect()))
        }
        (true, false) => Ok(Value::Map(map)),
        _ => Err(ConvertError::BadTable),
    }
}
