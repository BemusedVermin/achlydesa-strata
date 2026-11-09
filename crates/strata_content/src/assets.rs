//! Running each pack's `assets.lua`, the presentation file that maps visual keys to sprites
//! (design §13.1, §13.4, §13.8).
//!
//! The simulation thread runs these files, because Bevy systems never touch Lua; the client gets
//! the plain [`Declarations`] and types them. They never reach the content registry or the
//! content hash, so changing art cannot break a replay.

use std::io::ErrorKind;
use std::path::PathBuf;

use strata_kernel::PackId;
pub use strata_lua::{Declaration, Declarations};
use strata_lua::{PackSource, declare};

/// The file each pack may declare its assets in.
pub const ASSETS_FILE: &str = "assets.lua";
/// The constructors `assets.lua` may call.
pub const ASSET_CONSTRUCTORS: [&str; 1] = ["sprite"];
/// The globals `assets.lua` may set.
pub const ASSET_GLOBALS: [&str; 2] = ["pixels_per_level", "hex_width_px"];

/// One pack's `assets.lua`, run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackAssets {
    /// The pack.
    pub pack: PackId,
    /// The pack's directory, which sheet paths are relative to.
    pub root: PathBuf,
    /// What the file declared, or why it could not be read or run.
    pub declared: Result<Declarations, String>,
}

/// Runs the `assets.lua` of each of `packs` that has one, in load order. A file that exists but
/// cannot be read is reported like a script error.
pub fn read_pack_assets(packs: &[PackSource]) -> Vec<PackAssets> {
    let mut out = Vec::new();
    for pack in packs {
        let path = pack.root.join(ASSETS_FILE);
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => Ok(source),
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(e) => Err(format!("cannot read {}: {e}", path.display())),
        };
        let name = format!("{}/{ASSETS_FILE}", pack.id);
        let declared = source.and_then(|source| {
            declare(&name, &source, &ASSET_CONSTRUCTORS, &ASSET_GLOBALS).map_err(|e| e.to_string())
        });
        out.push(PackAssets {
            pack: pack.id.clone(),
            root: pack.root.clone(),
            declared,
        });
    }
    out
}

#[cfg(test)]
mod tests;
