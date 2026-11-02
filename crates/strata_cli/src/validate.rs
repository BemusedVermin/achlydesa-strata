//! `strata pack validate`: load a pack set and report every error with its source line.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use miette::{LabeledSpan, NamedSource, Report, miette};
use strata_content::{discover_packs, load};
use strata_lua::LoadError;

/// Loads the packs under `roots`, prints the result and returns the process exit code.
pub fn run(roots: &[PathBuf]) -> ExitCode {
    match load(roots) {
        Ok(registry) => {
            for pack in registry.pack_identities() {
                println!("{} {} {}", pack.id, pack.version, pack.content_hash);
            }
            let count = registry.pack_identities().len();
            println!("ok: {count} pack(s), pack-set hash {}", registry.set_hash());
            ExitCode::SUCCESS
        }
        Err(errors) => {
            let roots_by_pack = pack_roots(roots);
            for error in &errors {
                eprintln!("{:?}", report(error, &roots_by_pack));
            }
            eprintln!("error: {} load error(s)", errors.len());
            ExitCode::from(1)
        }
    }
}

/// Where each discoverable pack lives on disk, by pack ID.
fn pack_roots(roots: &[PathBuf]) -> BTreeMap<String, PathBuf> {
    let packs = discover_packs(roots).unwrap_or_default();
    packs
        .into_iter()
        .map(|p| (p.manifest.id.to_string(), p.root))
        .collect()
}

/// A diagnostic for `error`, with a source snippet when its file can be read.
fn report(error: &LoadError, roots: &BTreeMap<String, PathBuf>) -> Report {
    let Some((path, text)) = read_source(error, roots) else {
        return miette!("{error}");
    };
    let Some(span) = line_span(&text, error.line) else {
        return miette!("{error}");
    };
    let label = if error.field_path.is_empty() {
        "here".to_owned()
    } else {
        format!("at `{}`", error.field_path)
    };
    let file = path.display().to_string().replace('\\', "/");
    miette!(
        labels = vec![LabeledSpan::at(span, label)],
        "{}",
        error.message
    )
    .with_source_code(NamedSource::new(file, text))
}

/// The on-disk path and contents of the script a `pack/relative/path.lua` name refers to.
fn read_source(error: &LoadError, roots: &BTreeMap<String, PathBuf>) -> Option<(PathBuf, String)> {
    let (pack, relative) = error.file.split_once('/')?;
    let path = roots.get(pack)?.join(relative);
    let text = std::fs::read_to_string(&path).ok()?;
    Some((path, text))
}

/// The byte range of the 1-based `line`, without its line ending.
fn line_span(text: &str, line: u32) -> Option<(usize, usize)> {
    let index = usize::try_from(line).ok()?.checked_sub(1)?;
    let mut start = 0;
    for (i, content) in text.split_inclusive('\n').enumerate() {
        if i == index {
            let len = content.trim_end_matches(['\r', '\n']).len();
            return Some((start, len.max(1)));
        }
        start += content.len();
    }
    None
}
