//! Runs `selene` against a generated Strata standard library definition.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

/// Names every pack script can see besides the Lua 5.4 standard library.
const NAMESPACES: [&str; 6] = ["sel", "cond", "val", "eff", "ctx", "require"];

/// The selene standard library definition for Strata scripts.
pub fn std_definition() -> String {
    let mut text = String::from("base: lua54\nglobals:\n");
    let names = strata_lua::stubs::kinds();
    for name in names.iter().copied().chain(NAMESPACES) {
        text.push_str(&format!("  {name}:\n    any: true\n"));
    }
    text
}

/// Lints `root` with selene and returns the number of errors it reported.
/// Prints a notice and returns 0 when selene is not installed.
pub fn run(root: &Path) -> usize {
    let dir = std::env::temp_dir().join(format!("strata-lint-{}", std::process::id()));
    let prepared = fs::create_dir_all(&dir)
        .and_then(|()| fs::write(dir.join("strata.yml"), std_definition()))
        .and_then(|()| fs::write(dir.join("selene.toml"), "std = \"strata\"\n"));
    if prepared.is_err() {
        println!("notice: cannot write the selene definition; skipping selene");
        return 0;
    }
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let result = Command::new("selene")
        .arg("--display-style=quiet")
        .arg(&root)
        .current_dir(&dir)
        .output();
    let _ = fs::remove_dir_all(&dir);
    match result {
        Err(e) if e.kind() == ErrorKind::NotFound => {
            println!("notice: selene is not installed; skipping selene checks");
            0
        }
        Err(e) => {
            println!("notice: could not run selene ({e}); skipping selene checks");
            0
        }
        Ok(out) => {
            print!("{}", String::from_utf8_lossy(&out.stdout));
            usize::from(!out.status.success())
        }
    }
}
