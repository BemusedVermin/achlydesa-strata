//! The `pack.lua` manifest (design §4.2).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use semver::{Version, VersionReq};
use strata_kernel::{PackId, Value};
use strata_lua::{Limits, LuaHost, lua_to_value};

use crate::error::PackError;

const FIELDS: [&str; 6] = [
    "id",
    "version",
    "name",
    "engine",
    "dependencies",
    "visibility",
];

/// Whether a pack may be bundled by the release packager.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Visibility {
    /// The pack may be distributed.
    #[default]
    Public,
    /// The pack is for personal use and is never bundled.
    Private,
}

/// A parsed `pack.lua`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackManifest {
    /// The pack's ID: lowercase letters, digits and underscores.
    pub id: PackId,
    /// The pack's version.
    pub version: Version,
    /// A display name.
    pub name: String,
    /// The engine versions the pack supports.
    pub engine: VersionReq,
    /// Required packs and the versions accepted, in ID order.
    pub dependencies: BTreeMap<PackId, VersionReq>,
    /// Whether the pack may be bundled (defaults to public).
    pub visibility: Visibility,
}

/// A manifest together with where it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedPack {
    /// The parsed manifest.
    pub manifest: PackManifest,
    /// The pack's directory.
    pub root: PathBuf,
    /// The `pack.lua` the manifest came from.
    pub file: PathBuf,
}

/// Creates the host used to evaluate manifests.
pub(crate) fn manifest_host() -> Result<LuaHost, PackError> {
    LuaHost::new(Limits::default()).map_err(|e| PackError::Script {
        file: PathBuf::new(),
        message: e.to_string(),
    })
}

/// Runs `file` in an environment with no globals and parses the table it returns.
pub fn load_manifest(host: &LuaHost, root: &Path, file: &Path) -> Result<LoadedPack, PackError> {
    let source = std::fs::read_to_string(file).map_err(|source| PackError::Io {
        path: file.to_owned(),
        source,
    })?;
    let script = |message: String| PackError::Script {
        file: file.to_owned(),
        message,
    };
    let env = host
        .lua()
        .create_table()
        .map_err(|e| script(e.to_string()))?;
    let chunk = host
        .load(&env, &file.display().to_string(), &source)
        .map_err(|e| script(e.to_string()))?;
    let results = host.call(&chunk, ()).map_err(|e| script(e.to_string()))?;
    if results.len() != 1 {
        return Err(script("pack.lua must return exactly one table".into()));
    }
    let returned = results.into_iter().next().unwrap_or_default();
    let value = lua_to_value(host.lua(), &returned).map_err(|e| script(e.to_string()))?;
    let manifest = parse(&value).map_err(|message| PackError::Manifest {
        file: file.to_owned(),
        message,
    })?;
    Ok(LoadedPack {
        manifest,
        root: root.to_owned(),
        file: file.to_owned(),
    })
}

fn parse(value: &Value) -> Result<PackManifest, String> {
    let map = value
        .as_map()
        .map_err(|_| "expected a table of named fields".to_owned())?;
    if let Some(key) = map.keys().find(|k| !FIELDS.contains(&k.as_str())) {
        return Err(format!("unknown field `{key}`"));
    }
    let text = |key: &str| -> Result<&str, String> {
        let field = value
            .get(key)
            .map_err(|_| format!("missing field `{key}`"))?;
        field
            .as_str()
            .map_err(|_| format!("`{key}` must be a string"))
    };
    let id = text("id")?;
    let valid = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_';
    if id.is_empty() || !id.bytes().all(valid) {
        return Err(format!(
            "`id` `{id}` must be lowercase letters, digits and underscores"
        ));
    }
    let version = Version::parse(text("version")?).map_err(|e| format!("`version`: {e}"))?;
    let engine = VersionReq::parse(text("engine")?).map_err(|e| format!("`engine`: {e}"))?;
    let mut dependencies = BTreeMap::new();
    // An empty Lua table converts to an empty list, so `dependencies = {}` is a list.
    let declared = value.get("dependencies").ok();
    if let Some(deps) = declared.filter(|v| !matches!(v, Value::List(l) if l.is_empty())) {
        let deps = deps
            .as_map()
            .map_err(|_| "`dependencies` must map pack IDs to ranges".to_owned())?;
        for (name, req) in deps {
            let req = req
                .as_str()
                .map_err(|_| format!("dependency `{name}` must be a version range string"))?;
            let req = VersionReq::parse(req).map_err(|e| format!("dependency `{name}`: {e}"))?;
            dependencies.insert(PackId::new(name.as_str()), req);
        }
    }
    let visibility = match value.get("visibility").map(Value::as_str) {
        Err(_) => Visibility::default(),
        Ok(Ok("public")) => Visibility::Public,
        Ok(Ok("private")) => Visibility::Private,
        Ok(_) => return Err("`visibility` must be \"public\" or \"private\"".into()),
    };
    Ok(PackManifest {
        id: PackId::new(id),
        version,
        name: text("name")?.to_owned(),
        engine,
        dependencies,
        visibility,
    })
}
