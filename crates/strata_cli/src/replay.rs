//! `strata replay`: verify, dump and turn into a test a saved match.

use std::fs;
use std::path::{Path, PathBuf};

use miette::{IntoDiagnostic, Result, WrapErr};
use strata_session::Replay;

use crate::pack_test::load;

/// Reads the replay at `file`.
fn read(file: &Path) -> Result<Replay> {
    let bytes = fs::read(file)
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot read {}", file.display()))?;
    Replay::load(&bytes)
        .into_diagnostic()
        .wrap_err_with(|| format!("cannot load {}", file.display()))
}

/// Replays every command against the packs and checks each state hash.
pub fn verify(file: &Path, packs: &[PathBuf]) -> Result<()> {
    let replay = read(file)?;
    let tests = load(packs).into_diagnostic()?;
    let events = tests.replay_events(&replay).into_diagnostic()?;
    println!(
        "verified {} commands: every state hash matches",
        events.len()
    );
    Ok(())
}

/// Prints the replay as text. With packs, the events of each command are included.
pub fn dump(file: &Path, packs: &[PathBuf]) -> Result<()> {
    let replay = read(file)?;
    let events = if packs.is_empty() {
        Vec::new()
    } else {
        let tests = load(packs).into_diagnostic()?;
        tests.replay_events(&replay).into_diagnostic()?
    };
    print!("{}", strata_session::dump(&replay, &events));
    Ok(())
}

/// Prints a pack test that starts after `at` commands and expects the next command's outcome,
/// or writes it to `out`.
pub fn to_test(file: &Path, packs: &[PathBuf], at: usize, out: Option<&Path>) -> Result<()> {
    let replay = read(file)?;
    let tests = load(packs).into_diagnostic()?;
    let text = tests.to_test(&replay, at).into_diagnostic()?;
    match out {
        Some(path) => fs::write(path, text)
            .into_diagnostic()
            .wrap_err_with(|| format!("cannot write {}", path.display())),
        None => {
            print!("{text}");
            Ok(())
        }
    }
}
