//! The sprite catalog: every pack's `assets.lua`, typed (design §13.4, §13.8).
//!
//! Sprite metadata is presentation data. It never enters the rules registry or the content hash.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use glam::UVec2;
use strata_kernel::{PackId, Sym, Value};
use strata_lua::{PackSource, declare};

use super::{Animation, IDLE, PixelScale, SpriteDef};

/// The file each pack may declare its assets in.
pub const ASSETS_FILE: &str = "assets.lua";
/// The folder sprite sheets are found in, relative to the pack root.
pub const ASSETS_DIR: &str = "assets";
/// How many directions every sprite set has, one per hex facing.
pub const DIRECTIONS: i64 = 6;

/// A problem in one pack's `assets.lua`. The pack keeps the default scale and no sprites, so its
/// figures fall back to generated ones.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{file}: {message}")]
pub struct AssetError {
    /// The file, as `pack/assets.lua`.
    pub file: String,
    /// What is wrong.
    pub message: String,
}

/// One declared sprite and where its sheet is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteEntry {
    /// The pack that declared it, whose pixel scale it is drawn at.
    pub pack: PackId,
    /// The sprite sheet on disk.
    pub sheet: PathBuf,
    /// The frame layout and animations.
    pub def: SpriteDef,
}

/// Every pack's pixel scale and sprites.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpriteCatalog {
    scales: BTreeMap<PackId, PixelScale>,
    sprites: BTreeMap<Sym, SpriteEntry>,
}

impl SpriteCatalog {
    /// Reads each pack's `assets.lua`, in load order; a later pack's sprite replaces an earlier
    /// one with the same key. A pack without the file has the default scale and no sprites.
    pub fn load(packs: &[PackSource]) -> (Self, Vec<AssetError>) {
        let mut catalog = Self::default();
        let mut errors = Vec::new();
        for pack in packs {
            let path = pack.root.join(ASSETS_FILE);
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Err(error) = catalog.add_pack(&pack.id, &pack.root, &source) {
                errors.push(error);
            }
        }
        (catalog, errors)
    }

    /// Adds the pack `id` rooted at `root` from the text of its `assets.lua`. Nothing is added
    /// when the file has a problem.
    pub fn add_pack(&mut self, id: &PackId, root: &Path, source: &str) -> Result<(), AssetError> {
        let file = format!("{id}/{ASSETS_FILE}");
        let fail = |message: String| AssetError {
            file: file.clone(),
            message,
        };
        let globals = ["pixels_per_level", "hex_width_px"];
        let out = declare(&file, source, &["sprite"], &globals).map_err(|e| fail(e.to_string()))?;
        let mut scale = PixelScale::default();
        for (name, slot) in globals
            .iter()
            .zip([&mut scale.pixels_per_level, &mut scale.hex_width_px])
        {
            if let Some(value) = out.globals.get(*name) {
                *slot = positive(value)
                    .ok_or_else(|| fail(format!("`{name}` must be a positive integer")))?;
            }
        }
        let mut sprites = BTreeMap::new();
        for call in &out.calls {
            let at = |message: String| fail(format!("line {}: sprite {message}", call.line));
            let entry = sprite(id, root, &call.value).map_err(at)?;
            let key = entry.def.key.clone();
            if sprites.insert(key.clone(), entry).is_some() {
                return Err(at(format!("`{key}` is declared twice")));
            }
        }
        self.scales.insert(id.clone(), scale);
        self.sprites.extend(sprites);
        Ok(())
    }

    /// The pixel scale of pack `id`; the default when it declares none.
    pub fn scale(&self, id: &PackId) -> PixelScale {
        self.scales.get(id).copied().unwrap_or_default()
    }

    /// The sprite declared under `key`, if any pack declares one.
    pub fn sprite(&self, key: &str) -> Option<&SpriteEntry> {
        self.sprites.get(key)
    }

    /// Every declared sprite, by key.
    pub fn sprites(&self) -> impl Iterator<Item = &SpriteEntry> {
        self.sprites.values()
    }
}

fn positive(value: &Value) -> Option<u32> {
    value
        .as_int()
        .ok()
        .and_then(|i| u32::try_from(i).ok())
        .filter(|i| *i > 0)
}

/// The fields of `value`, refusing any not in `allowed`.
fn fields<'a>(value: &'a Value, allowed: &[&str]) -> Result<&'a BTreeMap<Sym, Value>, String> {
    let Value::Map(map) = value else {
        return Err("expects a table of named fields".into());
    };
    match map.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(unknown) => Err(format!("has an unknown field `{unknown}`")),
        None => Ok(map),
    }
}

/// The integers `names` of the table `field`, each at least `min`.
fn ints<const N: usize>(
    map: &BTreeMap<Sym, Value>,
    field: &str,
    names: [&str; N],
    min: i64,
) -> Result<[u32; N], String> {
    let table = map.get(field).ok_or_else(|| format!("needs `{field}`"))?;
    let table = fields(table, &names).map_err(|e| format!("`{field}` {e}"))?;
    let mut out = [0; N];
    for (slot, name) in out.iter_mut().zip(names) {
        let value = table.get(name).and_then(|v| v.as_int().ok());
        *slot = value
            .filter(|v| *v >= min)
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| format!("`{field}.{name}` must be an integer of at least {min}"))?;
    }
    Ok(out)
}

fn text<'a>(map: &'a BTreeMap<Sym, Value>, field: &str) -> Result<&'a str, String> {
    match map.get(field) {
        Some(Value::Str(s)) if !s.as_str().is_empty() => Ok(s.as_str()),
        _ => Err(format!("needs a string `{field}`")),
    }
}

/// A sheet path must stay inside the pack's assets folder.
fn sheet_path(root: &Path, sheet: &str) -> Result<PathBuf, String> {
    let relative = Path::new(sheet);
    if !relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(format!(
            "sheet `{sheet}` must be a relative path inside `{ASSETS_DIR}/`"
        ));
    }
    Ok(root.join(ASSETS_DIR).join(relative))
}

fn sprite(pack: &PackId, root: &Path, value: &Value) -> Result<SpriteEntry, String> {
    let allowed = [
        "key",
        "sheet",
        "frame",
        "anchor",
        "directions",
        "animations",
    ];
    let map = fields(value, &allowed)?;
    let key = Sym::new(text(map, "key")?);
    let sheet = sheet_path(root, text(map, "sheet")?)?;
    let [w, h] = ints(map, "frame", ["w", "h"], 1)?;
    let [x, y] = ints(map, "anchor", ["x", "y"], 0)?;
    if x > w || y > h {
        return Err(format!("anchor ({x}, {y}) lies outside the {w}x{h} frame"));
    }
    let directions = map.get("directions").map_or(Ok(DIRECTIONS), Value::as_int);
    if directions != Ok(DIRECTIONS) {
        return Err(format!(
            "`directions` must be {DIRECTIONS}, one per hex facing"
        ));
    }
    let listed = map.get("animations").ok_or("needs `animations`")?;
    let Value::Map(listed) = listed else {
        return Err("`animations` must map names to `{ frames, fps }`".into());
    };
    let mut animations = BTreeMap::new();
    for name in listed.keys() {
        let [frames, fps] = ints(listed, name.as_str(), ["frames", "fps"], 1)
            .map_err(|e| format!("animation {e}"))?;
        animations.insert(name.clone(), Animation { frames, fps });
    }
    if !animations.contains_key(IDLE) {
        return Err(format!("`{key}` needs an `{IDLE}` animation"));
    }
    let def = SpriteDef {
        key,
        frame: UVec2::new(w, h),
        anchor: UVec2::new(x, y),
        animations,
    };
    Ok(SpriteEntry {
        pack: pack.clone(),
        sheet,
        def,
    })
}

#[cfg(test)]
mod tests;
