//! Dependency checks and load order (design §4.3, step 1).

use std::collections::{BTreeMap, BTreeSet};

use semver::Version;
use strata_kernel::PackId;

use crate::ENGINE_VERSION;
use crate::error::PackError;
use crate::manifest::LoadedPack;

type Packs<'a> = BTreeMap<&'a PackId, &'a LoadedPack>;

/// Checks versions and dependencies and returns pack IDs in load order: dependencies first,
/// ties broken by ID. The result depends only on the manifests, not on the order of `packs`.
pub fn resolve_load_order(packs: &[LoadedPack]) -> Result<Vec<PackId>, PackError> {
    let mut sorted: Vec<&LoadedPack> = packs.iter().collect();
    sorted.sort_by(|a, b| (&a.manifest.id, &a.file).cmp(&(&b.manifest.id, &b.file)));
    if let Some(pair) = sorted
        .windows(2)
        .find(|w| w[0].manifest.id == w[1].manifest.id)
    {
        return Err(PackError::Duplicate {
            id: pair[0].manifest.id.clone(),
            first: pair[0].file.clone(),
            second: pair[1].file.clone(),
        });
    }
    let engine = Version::parse(ENGINE_VERSION).expect("ENGINE_VERSION is valid semver");
    let by_id: Packs = sorted.iter().map(|p| (&p.manifest.id, *p)).collect();
    for pack in &sorted {
        let m = &pack.manifest;
        if !m.engine.matches(&engine) {
            return Err(PackError::EngineMismatch {
                pack: m.id.clone(),
                file: pack.file.clone(),
                required: m.engine.to_string(),
                engine: ENGINE_VERSION.to_owned(),
            });
        }
        for (dependency, required) in &m.dependencies {
            let Some(found) = by_id.get(dependency) else {
                return Err(PackError::MissingDependency {
                    pack: m.id.clone(),
                    file: pack.file.clone(),
                    dependency: dependency.clone(),
                });
            };
            if !required.matches(&found.manifest.version) {
                return Err(PackError::VersionMismatch {
                    pack: m.id.clone(),
                    file: pack.file.clone(),
                    dependency: dependency.clone(),
                    required: required.to_string(),
                    found: found.manifest.version.to_string(),
                });
            }
        }
    }
    order(&by_id)
}

fn order(by_id: &Packs) -> Result<Vec<PackId>, PackError> {
    let mut remaining: BTreeSet<&PackId> = by_id.keys().copied().collect();
    let mut loaded = Vec::new();
    while !remaining.is_empty() {
        let ready = remaining.iter().copied().find(|id| {
            by_id[id]
                .manifest
                .dependencies
                .keys()
                .all(|d| !remaining.contains(d))
        });
        let Some(id) = ready else {
            return Err(cycle_error(by_id, &remaining));
        };
        remaining.remove(id);
        loaded.push(id.clone());
    }
    Ok(loaded)
}

/// Every remaining pack waits on another remaining pack, so walking dependencies from the
/// smallest ID must revisit one; that loop is the cycle.
fn cycle_error(by_id: &Packs, remaining: &BTreeSet<&PackId>) -> PackError {
    let mut path: Vec<&PackId> = Vec::new();
    let mut current = *remaining.iter().next().expect("remaining is not empty");
    while !path.contains(&current) {
        path.push(current);
        current = by_id[current]
            .manifest
            .dependencies
            .keys()
            .find(|d| remaining.contains(d))
            .expect("a stuck pack has a remaining dependency");
    }
    let start = path.iter().position(|id| *id == current).unwrap_or(0);
    let mut cycle: Vec<String> = path[start..].iter().map(|id| id.to_string()).collect();
    cycle.push(current.to_string());
    PackError::Cycle {
        file: by_id[current].file.clone(),
        cycle,
    }
}
