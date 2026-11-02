//! Runtime states: one sandboxed Lua state per thread, bound to the registry's `ScriptRef`s
//! (design §9.1).

use std::collections::BTreeMap;

use mlua::{Function, Table, Value};
use strata_kernel::{StableHash, StableHasher};
use strata_model::ScriptRef;
use strata_rules::{EvalCtx, LogicError, ScriptCaller};

use crate::convert::{contents, lua_to_value, value_to_lua};
use crate::data::{DataStage, PackSource, Record};
use crate::error::{HostError, RuntimeError};
use crate::host::Limits;

const MAX_DEPTH: usize = 64;

/// A runtime state that calls the functions behind `ScriptRef`s.
///
/// It is not `Send`: each thread builds its own from the same packs and never shares it.
pub struct RuntimeLua {
    stage: DataStage,
    functions: BTreeMap<(String, String), Function>,
}

impl RuntimeLua {
    /// Runs the pack scripts in a fresh state, binds every function in a definition to its
    /// path, and rejects the state unless those paths hash like `scripts`, the registry's refs.
    pub fn new(scripts: &[ScriptRef], packs: &[PackSource]) -> Result<Self, RuntimeError> {
        Self::with_limits(scripts, packs, Limits::default())
    }

    /// Like [`RuntimeLua::new`], with explicit limits for the state.
    pub fn with_limits(
        scripts: &[ScriptRef],
        packs: &[PackSource],
        limits: Limits,
    ) -> Result<Self, RuntimeError> {
        let stage = DataStage::run_with(packs, limits)?;
        let mut functions = BTreeMap::new();
        for record in stage.records() {
            if let Some(table) = stage.table(&record.kind, &record.id) {
                let mut route = Vec::new();
                harvest(
                    stage.host().lua(),
                    &record,
                    &table,
                    &mut route,
                    &mut functions,
                )?;
            }
        }
        let expected = path_hash(scripts.iter().map(|s| (s.pack.as_str(), s.path.as_str())));
        let harvested = path_hash(
            functions
                .keys()
                .map(|(p, path)| (p.as_str(), path.as_str())),
        );
        if expected != harvested {
            return Err(RuntimeError::HashMismatch {
                expected,
                harvested,
            });
        }
        Ok(Self { stage, functions })
    }

    /// Calls the function behind `script` with `args` under the instruction budget.
    /// A script that raises `Halt` comes back as [`HostError::Halt`].
    pub fn call_script(
        &self,
        script: &ScriptRef,
        args: &[strata_kernel::Value],
    ) -> Result<strata_kernel::Value, HostError> {
        let key = (
            script.pack.as_str().to_owned(),
            script.path.as_str().to_owned(),
        );
        let function = self.functions.get(&key).ok_or_else(|| {
            HostError::Script(format!("no function bound to `{}`", script.path.as_str()))
        })?;
        let host = self.stage.host();
        let lua = host.lua();
        let args = args
            .iter()
            .map(|arg| value_to_lua(lua, arg))
            .collect::<Result<Vec<_>, _>>()?;
        let mut out = host.call(function, mlua::MultiValue::from_iter(args))?;
        let first = out.pop_front().unwrap_or(Value::Nil);
        Ok(lua_to_value(lua, &first)?)
    }
}

impl ScriptCaller for RuntimeLua {
    /// Calls the script with `(source, subject, context)`.
    fn call(
        &self,
        script: &ScriptRef,
        cx: &EvalCtx<'_>,
    ) -> Result<strata_kernel::Value, LogicError> {
        let source = strata_kernel::Value::Str(cx.source.clone());
        let args = [source, cx.subject.clone(), cx.context.clone()];
        self.call_script(script, &args)
            .map_err(|error| LogicError(error.to_string()))
    }
}

/// Hashes a set of `(pack, path)` pairs; order and duplicates do not matter.
fn path_hash<'a>(paths: impl Iterator<Item = (&'a str, &'a str)>) -> u64 {
    let sorted: Vec<(&str, &str)> = paths
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut hasher = StableHasher::new();
    sorted.stable_hash(&mut hasher);
    hasher.finish()
}

/// Collects every function under `table`, keyed by pack and `kind:name/route`.
fn harvest(
    lua: &mlua::Lua,
    record: &Record,
    table: &Table,
    route: &mut Vec<String>,
    out: &mut BTreeMap<(String, String), Function>,
) -> Result<(), RuntimeError> {
    if route.len() >= MAX_DEPTH {
        return Err(RuntimeError::TooDeep(record.id.clone()));
    }
    for pair in contents(lua, table).pairs::<Value, Value>() {
        let (key, value) = pair?;
        let segment = match &key {
            Value::Integer(i) => i.to_string(),
            Value::String(s) => s.to_str()?.to_owned(),
            _ => continue,
        };
        route.push(segment);
        match value {
            Value::Function(function) => {
                let name = record.id.split_once(':').map_or(&*record.id, |(_, n)| n);
                let path = format!("{}:{name}/{}", record.kind, route.join("/"));
                out.insert((record.source.pack.as_str().to_owned(), path), function);
            }
            Value::Table(child) => harvest(lua, record, &child, route, out)?,
            _ => {}
        }
        route.pop();
    }
    Ok(())
}
