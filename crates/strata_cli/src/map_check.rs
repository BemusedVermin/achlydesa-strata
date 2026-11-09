//! `strata map check`: validate one map file of a pack and print its fairness report.

use std::path::Path;
use std::process::ExitCode;

use miette::{IntoDiagnostic, Result};
use strata_editor::{OpenedMap, Severity, ValidationOptions};

/// The exit code when the map has errors.
const HAS_ERRORS: u8 = 1;

/// The exit code when the map cannot be checked at all, for any error `run` returns.
pub const CANNOT_CHECK: u8 = 2;

/// Validates the map defined in `file`, prints every finding and the fairness report, and
/// returns [`HAS_ERRORS`] if any finding is an error.
///
/// An `Err` means the map could not be checked: the file cannot be read, is not inside a pack,
/// the packs do not load, or the file defines no map. The caller exits with [`CANNOT_CHECK`].
pub fn run(file: &Path, options: &ValidationOptions) -> Result<ExitCode> {
    let opened = OpenedMap::open(file).into_diagnostic()?;
    println!("map {} ({})", opened.id, opened.source);
    let mut document = opened.into_document();
    let validation = document.validate(options);
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
        ExitCode::from(HAS_ERRORS)
    })
}
