//! The three data stages, the `data` table and pack-scoped `require` (design §4.3).

mod registry;
mod require;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use mlua::Table;
use strata_kernel::PackId;

use crate::error::{DataError, HostError};
use crate::host::{Limits, LuaHost};
use registry::{Registry, Shared, chunk_name};

/// The constructor kinds the data stage registers.
#[cfg(test)]
pub(crate) fn registry_kinds() -> Vec<&'static str> {
    registry::KINDS.to_vec()
}

const PRELUDE: &str = include_str!("data.lua");
/// The chunk name of the prelude that adds `data.each`.
const PRELUDE_CHUNK: &str = "=data_prelude";

/// One pass over every pack's scripts (design §4.3).
#[derive(Clone, Copy)]
enum Stage {
    Data,
    Updates,
    Final,
}

impl Stage {
    const ALL: [Self; 3] = [Self::Data, Self::Updates, Self::Final];

    fn name(self) -> &'static str {
        match self {
            Self::Data => "data",
            Self::Updates => "data_updates",
            Self::Final => "data_final",
        }
    }

    fn file(self) -> &'static str {
        match self {
            Self::Data => "data.lua",
            Self::Updates => "data_updates.lua",
            Self::Final => "data_final.lua",
        }
    }

    /// Only the first stage must exist in every pack.
    fn required(self) -> bool {
        matches!(self, Self::Data)
    }
}

/// A pack as the data stage sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackSource {
    /// The pack's ID.
    pub id: PackId,
    /// The pack's directory.
    pub root: PathBuf,
    /// Declared dependencies, which bare IDs and `require` may reach after the pack itself.
    pub dependencies: Vec<PackId>,
}

/// Where a definition was created.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// The pack whose script made the call.
    pub pack: PackId,
    /// The script, as `pack/relative/path.lua`.
    pub file: String,
    /// The line of the constructor call.
    pub line: u32,
}

/// A definition and where it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// The constructor kind, such as `unit`.
    pub kind: String,
    /// The full `pack:name` ID.
    pub id: String,
    /// The constructor call that created it.
    pub source: Source,
}

/// The loader state after every data stage has run.
pub struct DataStage {
    host: LuaHost,
    data: Table,
    registry: Shared,
}

impl DataStage {
    /// Runs `data.lua`, then `data_updates.lua`, then `data_final.lua` for every pack, with
    /// `packs` already in load order. The first stage is required; the others are optional.
    pub fn run(packs: &[PackSource]) -> Result<Self, DataError> {
        Self::run_with(packs, Limits::default())
    }

    /// Like [`DataStage::run`], with explicit limits for the loader state.
    /// The instruction budget applies to each stage file separately.
    pub fn run_with(packs: &[PackSource], limits: Limits) -> Result<Self, DataError> {
        let host = LuaHost::new(limits)?;
        let registry: Shared = Rc::new(RefCell::new(Registry::new(packs)));
        let data = registry::install(host.lua(), &registry)?;
        require::install(host.lua(), &registry)?;
        let prelude = host.lua().load(PRELUDE).set_name(PRELUDE_CHUNK);
        prelude.call::<()>((data.clone(), host.lua().globals()))?;
        for pack in packs {
            let env = host.new_env()?;
            let key = require::env_key(pack.id.as_str());
            host.lua().set_named_registry_value(&key, env)?;
        }
        for stage in Stage::ALL {
            for pack in packs {
                run_file(&host, pack, stage)?;
            }
        }
        Ok(Self {
            host,
            data,
            registry,
        })
    }

    /// Returns the loader state, which still holds every record table.
    pub fn host(&self) -> &LuaHost {
        &self.host
    }

    /// Every record in `(kind, ID)` order, with its source.
    pub fn records(&self) -> Vec<Record> {
        let registry = self.registry.borrow();
        let entries = registry.sources.iter();
        entries
            .map(|((kind, id), source)| Record {
                kind: kind.clone(),
                id: id.clone(),
                source: source.clone(),
            })
            .collect()
    }

    /// The record table for a full ID, including any edits made by later stages.
    pub fn table(&self, kind: &str, id: &str) -> Option<Table> {
        let records: Table = self.data.raw_get(kind).ok()?;
        records.raw_get(id).ok()
    }
}

fn run_file(host: &LuaHost, pack: &PackSource, stage: Stage) -> Result<(), DataError> {
    let file = stage.file();
    let path = pack.root.join(file);
    if !path.is_file() {
        if !stage.required() {
            return Ok(());
        }
        return Err(DataError::MissingEntry {
            pack: pack.id.clone(),
            path,
        });
    }
    let source = std::fs::read_to_string(&path).map_err(|source| DataError::Io {
        path: path.clone(),
        source,
    })?;
    let env: Table = host
        .lua()
        .named_registry_value(&require::env_key(pack.id.as_str()))?;
    let chunk = host.load(&env, &chunk_name(pack.id.as_str(), file), &source)?;
    host.call(&chunk, ())
        .map(drop)
        .map_err(|error| match error {
            HostError::Script(message) => DataError::Script {
                pack: pack.id.clone(),
                stage: stage.name(),
                message,
            },
            other => DataError::Host(other),
        })
}
