//! `strata package`: bundle the client and the public packs into a distributable folder.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use miette::{IntoDiagnostic, Result, WrapErr, miette};
use strata_content::{Visibility, discover_packs, load};

/// The file name of the client binary on this platform.
fn client_name() -> String {
    format!("strata-client{}", std::env::consts::EXE_SUFFIX)
}

/// Where the client binary is looked for when `--client` is not given: next to this executable.
fn default_client() -> Result<PathBuf> {
    let exe = std::env::current_exe().into_diagnostic()?;
    let dir = exe
        .parent()
        .ok_or_else(|| miette!("cannot locate the strata executable"))?;
    Ok(dir.join(client_name()))
}

/// Writes `out/<client>`, `out/packs/<id>/...` and `out/manifest.txt`.
///
/// Fails before writing anything if a pack on the search path is private or the pack set does
/// not load.
pub fn run(roots: &[PathBuf], out: &Path, client: Option<PathBuf>) -> Result<()> {
    let packs = discover_packs(roots).map_err(|e| miette!("{e}"))?;
    let private: Vec<String> = packs
        .iter()
        .filter(|p| p.manifest.visibility == Visibility::Private)
        .map(|p| format!("`{}` ({})", p.manifest.id, p.root.display()))
        .collect();
    if !private.is_empty() {
        return Err(miette!(
            help = "move private packs out of the search path, or leave them out of the paths you pass",
            "refusing to package private pack(s): {}",
            private.join(", ")
        ));
    }
    let registry = load(roots).map_err(|errors| {
        let first = errors.first().map(ToString::to_string).unwrap_or_default();
        miette!(
            "pack set does not load ({} error(s)); first: {first}",
            errors.len()
        )
    })?;

    let client = match client {
        Some(path) => path,
        None => default_client()?,
    };
    if !client.is_file() {
        return Err(miette!(
            help = "build it with `cargo build --release -p strata_client`, or pass --client",
            "client binary not found at {}",
            client.display()
        ));
    }
    copy_file(&client, &out.join(client_name()))?;

    let mut manifest = format!("client {}\n", client_name());
    for identity in registry.pack_identities() {
        let pack = packs.iter().find(|p| p.manifest.id == identity.id);
        let pack = pack.ok_or_else(|| miette!("pack `{}` vanished", identity.id))?;
        copy_pack(&pack.root, &out.join("packs").join(identity.id.to_string()))?;
        let _ = writeln!(
            manifest,
            "pack {} {} {}",
            identity.id, identity.version, identity.content_hash
        );
    }
    let _ = writeln!(manifest, "pack-set {}", registry.set_hash());
    let path = out.join("manifest.txt");
    fs::write(&path, manifest)
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot write {}", path.display()))
}

/// Copies a pack directory, leaving out its top-level `tests` folder.
fn copy_pack(from: &Path, to: &Path) -> Result<()> {
    copy_tree(from, to, true)
}

fn copy_tree(from: &Path, to: &Path, top: bool) -> Result<()> {
    fs::create_dir_all(to)
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot create {}", to.display()))?;
    let read = fs::read_dir(from).into_diagnostic();
    let mut entries = read
        .wrap_err_with(|| format!("cannot read {}", from.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .into_diagnostic()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        let path = entry.path();
        if path.is_dir() {
            if !(top && name == "tests") {
                copy_tree(&path, &to.join(&name), false)?;
            }
        } else {
            copy_file(&path, &to.join(&name))?;
        }
    }
    Ok(())
}

fn copy_file(from: &Path, to: &Path) -> Result<()> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)
            .into_diagnostic()
            .wrap_err_with(|| format!("cannot create {}", dir.display()))?;
    }
    fs::copy(from, to)
        .map(drop)
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot copy {} to {}", from.display(), to.display()))
}
