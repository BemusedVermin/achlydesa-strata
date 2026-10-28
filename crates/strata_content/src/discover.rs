//! Scanning pack roots for `pack.lua` (design §4.1).

use std::path::{Path, PathBuf};

use crate::error::PackError;
use crate::manifest::{LoadedPack, load_manifest, manifest_host};

/// Loads the manifest of every direct subdirectory of each root that contains a `pack.lua`.
///
/// Directories are visited in sorted order, so the result and any error do not depend on the
/// file system's listing order.
pub fn discover_packs(roots: &[PathBuf]) -> Result<Vec<LoadedPack>, PackError> {
    let host = manifest_host()?;
    let mut packs = Vec::new();
    for root in roots {
        for dir in subdirectories(root)? {
            let file = dir.join("pack.lua");
            if file.is_file() {
                packs.push(load_manifest(&host, &dir, &file)?);
            }
        }
    }
    Ok(packs)
}

fn subdirectories(root: &Path) -> Result<Vec<PathBuf>, PackError> {
    let io = |source| PackError::Io {
        path: root.to_owned(),
        source,
    };
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(root).map_err(io)? {
        let path = entry.map_err(io)?.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}
