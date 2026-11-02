//! Running the data stages for resolved packs (design §4.3, steps 2-4).

use strata_kernel::PackId;
use strata_lua::{DataError, DataStage, PackSource};

use crate::manifest::LoadedPack;

/// Runs the three data stages over `packs` in `order`, as returned by `resolve_load_order`.
///
/// Dependencies are listed in ID order, because manifests are unordered tables.
///
/// # Panics
///
/// Panics if `order` names a pack that is not in `packs`.
pub fn run_data_stages(packs: &[LoadedPack], order: &[PackId]) -> Result<DataStage, DataError> {
    let sources: Vec<PackSource> = order
        .iter()
        .map(|id| {
            let pack = packs.iter().find(|p| p.manifest.id == *id);
            let pack = pack.expect("load order names a discovered pack");
            PackSource {
                id: id.clone(),
                root: pack.root.clone(),
                dependencies: pack.manifest.dependencies.keys().cloned().collect(),
            }
        })
        .collect();
    DataStage::run(&sources)
}
