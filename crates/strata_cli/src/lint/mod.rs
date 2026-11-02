//! `strata lint`: Strata's own Lua checks plus `selene` when it is installed (§16.1).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod checks;
mod selene;

/// How serious a finding is. Errors fail the run; warnings do not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// The script breaks a determinism rule.
    Error,
    /// The script is probably wrong but still deterministic.
    Warning,
}

/// One finding: where it is, what is wrong and how to fix it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// How serious the finding is.
    pub severity: Severity,
    /// 1-based source line.
    pub line: usize,
    /// What is wrong.
    pub message: String,
    /// What to change.
    pub fix: String,
}

/// Folders of a pack whose scripts are logic rather than data.
const LOGIC_DIRS: [&str; 4] = ["rules", "abilities", "scripts", "lib"];

/// Lints the packs under `roots`, prints the findings and returns the exit code.
pub fn run(roots: &[PathBuf]) -> ExitCode {
    let (mut errors, mut warnings) = (0, 0);
    for root in roots {
        let Ok(files) = lua_files(root) else {
            eprintln!("error: cannot read {}", root.display());
            return ExitCode::from(1);
        };
        for (path, logic) in files {
            let shown = path.display().to_string().replace('\\', "/");
            let Ok(source) = fs::read_to_string(&path) else {
                eprintln!("error: {shown}: cannot read file");
                errors += 1;
                continue;
            };
            for d in checks::check(&source, logic) {
                match d.severity {
                    Severity::Error => errors += 1,
                    Severity::Warning => warnings += 1,
                }
                print_diagnostic(&shown, &d);
            }
        }
        errors += selene::run(root);
    }
    println!("{errors} error(s), {warnings} warning(s)");
    if errors == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn print_diagnostic(file: &str, d: &Diagnostic) {
    let label = match d.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    println!("{label}: {file}:{}: {}", d.line, d.message);
    println!("  fix: {}", d.fix);
}

/// Every `.lua` file under `root` in path order, with whether it is a logic file.
fn lua_files(root: &Path) -> std::io::Result<Vec<(PathBuf, bool)>> {
    let mut out = Vec::new();
    let packs = if root.join("pack.lua").is_file() {
        vec![root.to_path_buf()]
    } else {
        sorted_entries(root)?
            .into_iter()
            .filter(|p| p.is_dir())
            .collect()
    };
    for pack in packs {
        let mut found = Vec::new();
        collect(&pack, &mut found)?;
        for path in found {
            let first = path.strip_prefix(&pack).ok().and_then(|p| p.iter().next());
            let logic = first.is_some_and(|c| LOGIC_DIRS.iter().any(|d| c == *d));
            out.push((path, logic));
        }
    }
    Ok(out)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for path in sorted_entries(dir)? {
        if path.is_dir() {
            collect(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "lua") {
            out.push(path);
        }
    }
    Ok(())
}

fn sorted_entries(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    Ok(paths)
}
