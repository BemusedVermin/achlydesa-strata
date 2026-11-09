//! Pack-scoped `require` (design §9.3).

use mlua::{Lua, Result, Table, Value};

use super::registry::{CHUNK_SEPARATOR, PACK_SEPARATOR, Shared, caller, chunk_name};
use crate::lock::{freeze_deep, into_lua};

const MODULES_KEY: &str = "strata.modules";
const ENV_PREFIX: &str = "strata.env.";
const REQUIRE_GLOBAL: &str = "require";
/// Separates the parts of a module name, which map to directories.
const MODULE_SEPARATOR: char = '.';
const MODULE_EXTENSION: &str = "lua";

pub(crate) fn env_key(pack: &str) -> String {
    format!("{ENV_PREFIX}{pack}")
}

fn valid_word(word: &str) -> bool {
    let ok = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_';
    !word.is_empty() && word.bytes().all(ok)
}

/// Installs `require`: `"lib.util"` names a file in the calling pack, `"pack:lib.util"` one in a
/// declared dependency. Results are cached per module and frozen, including nested tables.
pub(crate) fn install(lua: &Lua, shared: &Shared) -> Result<()> {
    lua.set_named_registry_value(MODULES_KEY, lua.create_table()?)?;
    let registry = shared.clone();
    let require = lua.create_function(move |lua, name: String| {
        let (from, ..) = caller(lua)?;
        let fail = |why: String| mlua::Error::runtime(format!("require `{name}`: {why}"));
        let (owner, module) = name
            .split_once(PACK_SEPARATOR)
            .unwrap_or((from.as_str(), &name));
        if !module.split(MODULE_SEPARATOR).all(valid_word) {
            return Err(fail(
                "module names are dot-separated lowercase words".into(),
            ));
        }
        let root = {
            let registry = registry.borrow();
            let Some((id, info)) = registry.packs.iter().find(|(id, _)| id.as_str() == owner)
            else {
                return Err(fail(format!("unknown pack `{owner}`")));
            };
            let declared = registry.packs[&from].dependencies.contains(id);
            if *id != from && !declared {
                return Err(fail(format!(
                    "`{from}` does not declare `{owner}` as a dependency"
                )));
            }
            info.root.clone()
        };
        let key = format!("{owner}{PACK_SEPARATOR}{module}");
        let cache: Table = lua.named_registry_value(MODULES_KEY)?;
        let cached: Value = cache.raw_get(key.as_str())?;
        if !cached.is_nil() {
            return Ok(cached);
        }
        if registry.borrow().sealed {
            return Err(fail(
                "modules load only while the packs load; require it at the top of a file".into(),
            ));
        }
        if !registry.borrow_mut().loading.insert(key.clone()) {
            return Err(fail("modules require each other in a cycle".into()));
        }
        let directories = module.replace(MODULE_SEPARATOR, &CHUNK_SEPARATOR.to_string());
        let relative = format!("{directories}.{MODULE_EXTENSION}");
        let loaded = std::fs::read_to_string(root.join(&relative))
            .map_err(|e| fail(format!("{owner}/{relative}: {e}")))
            .and_then(|source| {
                let env: Table = lua.named_registry_value(&env_key(owner))?;
                lua.load(source)
                    .set_name(format!("={}", chunk_name(owner, &relative)))
                    .set_environment(env)
                    .call::<Value>(())
            });
        registry.borrow_mut().loading.remove(&key);
        let value = match loaded? {
            Value::Nil => Value::Boolean(true),
            Value::Table(table) => {
                freeze_deep(lua, &table).map_err(into_lua)?;
                Value::Table(table)
            }
            other => other,
        };
        cache.raw_set(key.as_str(), value.clone())?;
        Ok(value)
    })?;
    lua.globals().set(REQUIRE_GLOBAL, require)
}
