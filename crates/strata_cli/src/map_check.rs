//! `strata map check`: validate one map file of a pack and print its fairness report.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use miette::{IntoDiagnostic, Result, WrapErr, miette};
use strata_content::{ContentRegistry, discover_packs, load};
use strata_editor::{MapDocument, RegistryDefs, Severity, ValidationOptions};
use strata_model::MapDef;

/// Validates the map defined in `file`, prints every finding and the fairness report, and
/// exits non-zero if any finding is an error.
pub fn run(file: &Path, options: &ValidationOptions) -> Result<ExitCode> {
    let file = file
        .canonicalize()
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot read {}", file.display()))?;
    let pack_root = file
        .ancestors()
        .find(|dir| dir.join("pack.lua").is_file())
        .ok_or_else(|| miette!("{} is not inside a pack", file.display()))?;
    let roots = [pack_root.parent().unwrap_or(pack_root).to_path_buf()];
    let pack = pack_id(&roots, pack_root)?;
    let registry = load(&roots).map_err(|errors| {
        let first = errors.first().map(ToString::to_string).unwrap_or_default();
        miette!("the packs do not load ({first}); run `strata pack validate`")
    })?;
    let relative = file.strip_prefix(pack_root).into_diagnostic()?;
    let source = format!("{pack}/{}", relative.to_string_lossy().replace('\\', "/"));
    let (id, map) = find_map(&registry, &source)?;
    let mut document = MapDocument::new(map, RegistryDefs::new(registry, pack));
    let validation = document.validate(options);
    println!("map {id} ({source})");
    for finding in &validation.findings {
        println!("{finding}");
    }
    if let Some(fairness) = &validation.fairness {
        print!("{fairness}");
    }
    let errors = validation.count(Severity::Error);
    let warnings = validation.count(Severity::Warning);
    println!("{errors} error(s), {warnings} warning(s)");
    Ok(if errors == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// The ID of the pack whose folder is `pack_root`.
fn pack_id(roots: &[PathBuf], pack_root: &Path) -> Result<String> {
    let packs = discover_packs(roots).map_err(|e| miette!("{e}"))?;
    let same =
        |p: &&strata_content::LoadedPack| p.root.canonicalize().ok().as_deref() == Some(pack_root);
    let pack = packs.iter().find(same);
    let pack = pack.ok_or_else(|| miette!("no pack at {}", pack_root.display()))?;
    Ok(pack.manifest.id.to_string())
}

/// The map whose definition comes from the `pack/relative/path.lua` file `source`.
fn find_map(registry: &ContentRegistry, source: &str) -> Result<(String, MapDef)> {
    let mut maps = registry.iter::<MapDef>();
    let found = maps.find(|(_, _, map)| map.source.file == source);
    let (_, id, map) = found.ok_or_else(|| miette!("{source} defines no map"))?;
    Ok((id.to_owned(), map.clone()))
}
