//! The sprite catalog: every pack's `assets.lua`, typed (design §13.4, §13.8).
//!
//! Sprite metadata is presentation data. It never enters the rules registry or the content hash.
//! The simulation thread runs the files ([`strata_content::read_pack_assets`]); this module only
//! types what they declared, so the client never touches Lua (design §13.1).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use glam::UVec2;
use strata_content::{ASSETS_FILE, Declarations, PackAssets};
use strata_kernel::{PackId, Sym, Value};

use super::{Animation, IDLE, PixelScale, SpriteDef};

/// The folder sprite sheets are found in, relative to the pack root.
pub const ASSETS_DIR: &str = "assets";
/// How many directions every sprite set has, one per hex facing.
pub const DIRECTIONS: i64 = 6;
/// The most sprite pixels a pack may put in one level of height.
pub const MAX_PIXELS_PER_LEVEL: u32 = 64;
/// The most sprite pixels a pack may put across one hex.
pub const MAX_HEX_WIDTH_PX: u32 = 512;
/// The widest and tallest a sprite frame may be, in pixels.
pub const MAX_FRAME_PX: u32 = 1024;
/// The most frames one animation may have.
pub const MAX_FRAMES: u32 = 64;
/// The most frames per second one animation may play at.
pub const MAX_FPS: u32 = 120;
/// The widest and tallest a sprite sheet may need to be, in pixels: a common texture limit.
pub const MAX_SHEET_PX: u32 = 16_384;

/// A problem in one pack's `assets.lua`. The pack keeps the default scale and no sprites, and its
/// figures fall back to generated ones even where another pack declares the same key.
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
    failed: BTreeSet<PackId>,
}

impl SpriteCatalog {
    /// Types each pack's declarations, in load order; a later pack's sprite replaces an earlier
    /// one with the same key. A pack without `assets.lua` has the default scale and no sprites.
    /// A pack whose file failed adds nothing and is marked [failed](Self::failed).
    pub fn from_packs(packs: &[PackAssets]) -> (Self, Vec<AssetError>) {
        let mut catalog = Self::default();
        let mut errors = Vec::new();
        for pack in packs {
            let added = match &pack.declared {
                Ok(declared) => catalog.add_pack(&pack.pack, &pack.root, declared),
                Err(message) => Err(AssetError {
                    file: format!("{}/{ASSETS_FILE}", pack.pack),
                    message: message.clone(),
                }),
            };
            if let Err(error) = added {
                catalog.failed.insert(pack.pack.clone());
                errors.push(error);
            }
        }
        (catalog, errors)
    }

    /// Adds the pack `id` rooted at `root` from what its `assets.lua` declared. Nothing is added
    /// when a declaration has a problem.
    pub fn add_pack(
        &mut self,
        id: &PackId,
        root: &Path,
        declared: &Declarations,
    ) -> Result<(), AssetError> {
        let file = format!("{id}/{ASSETS_FILE}");
        let fail = |message: String| AssetError {
            file: file.clone(),
            message,
        };
        let mut scale = PixelScale::default();
        let globals = [
            ("pixels_per_level", MAX_PIXELS_PER_LEVEL),
            ("hex_width_px", MAX_HEX_WIDTH_PX),
        ];
        for ((name, max), slot) in globals
            .iter()
            .zip([&mut scale.pixels_per_level, &mut scale.hex_width_px])
        {
            if let Some(value) = declared.globals.get(*name) {
                *slot = positive(value, *max)
                    .ok_or_else(|| fail(format!("`{name}` must be an integer from 1 to {max}")))?;
            }
        }
        let mut sprites = BTreeMap::new();
        for call in &declared.calls {
            let at = |message: String| fail(format!("line {}: sprite {message}", call.line));
            if call.kind != "sprite" {
                let line = call.line;
                return Err(fail(format!(
                    "line {line}: `{}` is not an asset",
                    call.kind
                )));
            }
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

    /// Whether pack `id`'s `assets.lua` failed, so its figures should use generated sprites.
    pub fn failed(&self, id: &PackId) -> bool {
        self.failed.contains(id)
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

fn positive(value: &Value, max: u32) -> Option<u32> {
    value
        .as_int()
        .ok()
        .and_then(|i| u32::try_from(i).ok())
        .filter(|i| (1..=max).contains(i))
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

/// The integers `names` of the table `field`, each from `min` to `max`.
fn ints<const N: usize>(
    map: &BTreeMap<Sym, Value>,
    field: &str,
    names: [(&str, u32); N],
    min: u32,
) -> Result<[u32; N], String> {
    let table = map.get(field).ok_or_else(|| format!("needs `{field}`"))?;
    let allowed = names.map(|(name, _)| name);
    let table = fields(table, &allowed).map_err(|e| format!("`{field}` {e}"))?;
    let mut out = [0; N];
    for (slot, (name, max)) in out.iter_mut().zip(names) {
        let value = table.get(name).and_then(|v| v.as_int().ok());
        *slot = value
            .and_then(|v| u32::try_from(v).ok())
            .filter(|v| (min..=max).contains(v))
            .ok_or_else(|| format!("`{field}.{name}` must be an integer from {min} to {max}"))?;
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
    let frame = [("w", MAX_FRAME_PX), ("h", MAX_FRAME_PX)];
    let [w, h] = ints(map, "frame", frame, 1)?;
    let [x, y] = ints(map, "anchor", [("x", w), ("y", h)], 0)?;
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
        let timing = [("frames", MAX_FRAMES), ("fps", MAX_FPS)];
        let [frames, fps] =
            ints(listed, name.as_str(), timing, 1).map_err(|e| format!("animation {e}"))?;
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
    match def.sheet_size() {
        Some(size) if size.max_element() <= MAX_SHEET_PX => {}
        _ => {
            return Err(format!(
                "needs a sheet wider or taller than {MAX_SHEET_PX} pixels"
            ));
        }
    }
    Ok(SpriteEntry {
        pack: pack.clone(),
        sheet,
        def,
    })
}

#[cfg(test)]
mod tests;
