//! `strata pack test`: run the `test {}` records of a pack set.

use std::path::PathBuf;
use std::process::ExitCode;

use strata_session::pack_test::{PackTests, summary};

/// Runs the tests under `roots` and returns the process exit code: 1 if any test failed or the
/// packs did not load.
pub fn run(roots: &[PathBuf], filter: Option<&str>) -> ExitCode {
    let tests = match PackTests::load(roots) {
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
