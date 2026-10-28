//! Conversion between Lua values and the engine's `Value` (design §9.4).

use std::collections::BTreeMap;

use mlua::{Lua, Table};
use strata_kernel::{Sym, Value};

use crate::error::ConvertError;
use crate::host::FROZEN_KEY;

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

fn convert_table(lua: &Lua, table: &Table, depth: usize) -> Result<Value, ConvertError> {
    // A frozen table keeps its contents in a shadow table (see prelude.lua).
    let shadow = lua
        .named_registry_value::<Table>(FROZEN_KEY)
        .ok()
        .and_then(|frozen| {
            frozen
                .raw_get::<Option<Table>>(table.clone())
                .ok()
                .flatten()
        });
    let source = shadow.as_ref().unwrap_or(table);
    let mut list = BTreeMap::new();
    let mut map = BTreeMap::new();
    for pair in source.pairs::<mlua::Value, mlua::Value>() {
        let (key, item) = pair.map_err(|_| ConvertError::BadTable)?;
        let item = convert(lua, &item, depth + 1)?;
        match key {
            mlua::Value::Integer(i) => list.insert(i, item),
            mlua::Value::String(s) => map.insert(sym(&s)?, item),
            _ => return Err(ConvertError::BadTable),
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
