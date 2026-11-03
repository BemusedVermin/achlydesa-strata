//! `strata pack test`: run the `test {}` records of a pack set.

use std::path::PathBuf;
use std::process::ExitCode;

use strata_modules::native_modules;
use strata_session::pack_test::{PackTestError, PackTests, summary};

/// Loads the packs under `roots` with every native module the engine ships.
fn load(roots: &[PathBuf]) -> Result<PackTests, PackTestError> {
    let tests = PackTests::load(roots)?;
    native_modules()
        .into_iter()
        .try_fold(tests, PackTests::with_native)
}

/// Runs the tests under `roots` and returns the process exit code: 1 if any test failed or the
/// packs did not load.
pub fn run(roots: &[PathBuf], filter: Option<&str>) -> ExitCode {
    let tests = match load(roots) {
        Ok(tests) => tests,
        Err(error) => {
            eprintln!("{error}\nerror: the packs did not load, so no tests ran");
            return ExitCode::from(1);
        }
    };
    let reports = tests.run(filter);
    print!("{}", summary(&reports));
    if reports.iter().any(|r| r.outcome.is_err()) {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}
