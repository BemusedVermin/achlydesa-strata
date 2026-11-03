//! What a test run reports.

use std::fmt;

/// Why a test failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// The one-based step that failed, or `None` when the test could not be set up.
    pub step: Option<usize>,
    /// What went wrong, with the expected and actual values when a check failed.
    pub message: String,
}

/// The result of one test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestReport {
    /// The test's name.
    pub name: String,
    /// `Ok` when every step passed.
    pub outcome: Result<(), Failure>,
}

impl fmt::Display for TestReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.outcome {
            Ok(()) => write!(f, "PASS {}", self.name),
            Err(Failure { step, message }) => {
                let at = step.map_or("setup".to_owned(), |s| format!("step {s}"));
                write!(f, "FAIL {} ({at}): {message}", self.name)
            }
        }
    }
}

/// One line per test and a total, ready to print.
pub fn summary(reports: &[TestReport]) -> String {
    let mut out = String::new();
    for report in reports {
        out.push_str(&format!("{report}\n"));
    }
    let failed = reports.iter().filter(|r| r.outcome.is_err()).count();
    let passed = reports.len() - failed;
    out.push_str(&format!("{passed} passed, {failed} failed\n"));
    out
}
