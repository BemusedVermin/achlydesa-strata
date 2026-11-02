//! Loading packs from disk into a [`ContentRegistry`].

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use strata_lua::{LoadError, freeze_resolved};

use crate::intern::Interner;
use crate::registry::ContentRegistry;
use crate::{discover_packs, resolve_load_order, run_data_stages};

/// Discovers the packs under `roots`, runs the data stages, then freezes and resolves every
/// record into a [`ContentRegistry`].
///
/// A failure in discovery, resolution or the data stages stops the load, since nothing after it
/// can run. Once the data stages succeed, every record is checked and all errors are returned.
pub fn load(roots: &[PathBuf]) -> Result<ContentRegistry, Vec<LoadError>> {
    let packs = discover_packs(roots).map_err(|e| vec![unplaced(e)])?;
    let order = resolve_load_order(&packs).map_err(|e| vec![unplaced(e)])?;
    let stage = run_data_stages(&packs, &order).map_err(|e| vec![unplaced(e)])?;
    let dependencies: BTreeMap<_, BTreeSet<_>> = packs
        .iter()
        .map(|p| {
            let deps = p.manifest.dependencies.keys().cloned().collect();
            (p.manifest.id.clone(), deps)
        })
        .collect();
    let interner = Interner::new(&stage.records(), dependencies);
    let records = freeze_resolved(&stage, &interner)?;
    Ok(ContentRegistry::new(order, records))
}

/// An error whose message already names the file it is about.
fn unplaced(error: impl std::fmt::Display) -> LoadError {
    LoadError {
        file: String::new(),
        line: 0,
        field_path: String::new(),
        message: error.to_string(),
    }
}
