//! The record registry, constructors and bare-ID resolution (design §4.3, §4.4, §10.1).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::rc::Rc;

use mlua::{Function, Lua, Result, Table, Value};
use strata_kernel::PackId;

use super::{PackSource, Source};

/// Every constructor kind (design §10.1).
pub(crate) const KINDS: [&str; 20] = [
    "stat",
    "die",
    "marker",
    "traits",
    "faction",
    "terrain",
    "tile",
    "feature",
    "footprint",
    "unit",
    "ability",
    "relic",
    "rule_module",
    "ruleset",
    "map",
    "inventory",
    "scenario",
    "format",
    "army",
    "test",
];

/// The global holding every record, keyed by kind and full ID.
pub(crate) const DATA_GLOBAL: &str = "data";
/// Separates the pack from the name in a full ID, and from the module in a `require`.
pub(crate) const PACK_SEPARATOR: char = ':';
/// Separates the pack from the file in a chunk name such as `pack/lib/util.lua`.
pub(crate) const CHUNK_SEPARATOR: char = '/';
/// The string fields every constructor requires.
const ID_FIELD: &str = "id";
const NAME_FIELD: &str = "name";

/// The chunk name a pack file is loaded under; `caller` reads the pack back from it.
pub(crate) fn chunk_name(pack: &str, relative: &str) -> String {
    format!("{pack}{CHUNK_SEPARATOR}{relative}")
}

/// The full ID of `name` in `pack`.
fn full_id(pack: &str, name: &str) -> String {
    format!("{pack}{PACK_SEPARATOR}{name}")
}

/// What the loader knows about one pack.
pub(crate) struct PackInfo {
    pub root: PathBuf,
    pub dependencies: Vec<PackId>,
}

/// Shared by the constructors and `require`; holds no Lua values.
#[derive(Default)]
pub(crate) struct Registry {
    pub sources: BTreeMap<(String, String), Source>,
    pub packs: BTreeMap<PackId, PackInfo>,
    pub loading: BTreeSet<String>,
}

pub(crate) type Shared = Rc<RefCell<Registry>>;

impl Registry {
    pub fn new(packs: &[PackSource]) -> Self {
        let packs = packs.iter().map(|p| {
            let info = PackInfo {
                root: p.root.clone(),
                dependencies: p.dependencies.clone(),
            };
            (p.id.clone(), info)
        });
        Self {
            packs: packs.collect(),
            ..Self::default()
        }
    }

    /// The current pack's own definition wins; otherwise exactly one dependency must define it.
    fn resolve(&self, kind: &str, pack: &PackId, bare: &str) -> Result<Option<String>> {
        let defined = |id: &String| self.sources.contains_key(&(kind.to_owned(), id.clone()));
        let own = full_id(pack.as_str(), bare);
        if defined(&own) {
            return Ok(Some(own));
        }
        let deps = self.packs.get(pack).map_or(&[][..], |p| &p.dependencies);
        let mut found: Vec<String> = deps.iter().map(|d| full_id(d.as_str(), bare)).collect();
        found.retain(defined);
        match found.len() {
            0 | 1 => Ok(found.pop()),
            _ => Err(mlua::Error::runtime(format!(
                "ambiguous {kind} `{bare}`: defined by {}; use the full ID",
                found.join(", ")
            ))),
        }
    }
}

/// The pack, chunk name and line of the Lua code that called the running Rust function.
pub(crate) fn caller(lua: &Lua) -> Result<(PackId, String, u32)> {
    let (chunk, line) = lua
        .inspect_stack(1, |frame| {
            let chunk = frame.source().source;
            (
                chunk.map(|c| c.trim_start_matches('=').to_owned()),
                frame.current_line(),
            )
        })
        .and_then(|(chunk, line)| Some((chunk?, line.unwrap_or(0))))
        .ok_or_else(|| mlua::Error::runtime("must be called from a pack script"))?;
    let pack = chunk.split(CHUNK_SEPARATOR).next().unwrap_or_default();
    Ok((PackId::new(pack), chunk, line as u32))
}

/// Creates the `data` table, one table per kind, and a constructor global for each kind.
pub(crate) fn install(lua: &Lua, shared: &Shared) -> Result<Table> {
    let data = lua.create_table()?;
    for kind in KINDS {
        let records = lua.create_table()?;
        let meta = lua.create_table()?;
        let registry = shared.clone();
        meta.set(
            "__index",
            lua.create_function(move |lua, (records, key): (Table, Value)| {
                let Value::String(key) = key else {
                    return Ok(Value::Nil);
                };
                let key = key.to_str()?;
                if key.contains(PACK_SEPARATOR) {
                    return Ok(Value::Nil);
                }
                let (pack, ..) = caller(lua)?;
                match registry.borrow().resolve(kind, &pack, &key)? {
                    Some(id) => records.raw_get(id),
                    None => Ok(Value::Nil),
                }
            })?,
        )?;
        records.set_metatable(Some(meta))?;
        data.set(kind, records.clone())?;
        lua.globals()
            .set(kind, constructor(lua, shared.clone(), kind, records)?)?;
    }
    lua.globals().set(DATA_GLOBAL, data.clone())?;
    Ok(data)
}

fn constructor(lua: &Lua, shared: Shared, kind: &'static str, records: Table) -> Result<Function> {
    lua.create_function(move |lua, record: Table| {
        let (pack, file, line) = caller(lua)?;
        let fail = |what: String| mlua::Error::runtime(format!("{file}:{line}: {kind} {what}"));
        let text = |field: &str| match record.raw_get(field) {
            Ok(Value::String(s)) => Ok(s.to_str()?.to_owned()),
            _ => Err(fail(format!("needs a string `{field}`"))),
        };
        let id = text(ID_FIELD)?;
        if kind != "rule_module" {
            text(NAME_FIELD)?;
        }
        let full = match id.split_once(PACK_SEPARATOR) {
            None if !id.is_empty() => full_id(pack.as_str(), &id),
            Some((owner, name)) if owner == pack.as_str() && !name.is_empty() => id.clone(),
            _ => {
                return Err(fail(format!(
                    "`{id}` must be a bare name or `{pack}{PACK_SEPARATOR}name`"
                )));
            }
        };
        let mut registry = shared.borrow_mut();
        let key = (kind.to_owned(), full.clone());
        if let Some(first) = registry.sources.get(&key) {
            let (earlier, at) = (&first.file, first.line);
            return Err(fail(format!(
                "`{full}` is already defined at {earlier}:{at}"
            )));
        }
        records.raw_set(full, record.clone())?;
        registry.sources.insert(key, Source { pack, file, line });
        Ok(record)
    })
}
