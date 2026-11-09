use assert_cmd::Command;

fn strata(args: &[&str]) -> assert_cmd::assert::Assert {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/maps");
    Command::cargo_bin("strata")
        .unwrap()
        .current_dir(dir)
        .args(args)
        .assert()
}

fn stdout(assert: &assert_cmd::assert::Assert) -> String {
    String::from_utf8_lossy(&assert.get_output().stdout).replace("\r\n", "\n")
}

#[test]
fn a_clean_map_prints_no_findings_and_exits_zero() {
    let out = strata(&["map", "check", "t/maps/fair.lua", "--army-size", "2"]).success();
    insta::assert_snapshot!(stdout(&out));
}

#[test]
fn findings_are_printed_with_severity_and_cells_and_errors_exit_one() {
    let mut text = String::new();
    for map in ["overlap", "unsupported", "inventory", "unstandable_zone"] {
        let out = strata(&["map", "check", &format!("t/maps/{map}.lua")]);
        let code = if map == "unstandable_zone" { 0 } else { 1 };
        text.push_str(&stdout(&out.code(code)));
    }
    let out = strata(&["map", "check", "t/maps/small_zone.lua", "--army-size", "3"]).code(1);
    text.push_str(&stdout(&out));
    insta::assert_snapshot!(text);
}

fn stderr(assert: &assert_cmd::assert::Assert) -> String {
    String::from_utf8_lossy(&assert.get_output().stderr).into_owned()
}

/// Runs `map check` on `file`, expects the "cannot check" exit code, and returns stderr
/// unwrapped onto one line. miette wraps long messages (a long temp path on macOS) and marks
/// continuation lines with a `|` gutter.
fn cannot_check(file: &str) -> String {
    let out = strata(&["map", "check", file]).code(2);
    assert_eq!(stdout(&out), "", "{file}");
    stderr(&out)
        .lines()
        .map(|line| line.trim().trim_start_matches('|').trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn a_file_that_defines_no_map_is_an_error() {
    let err = cannot_check("t/data.lua");
    assert!(err.contains("t/data.lua defines no map"), "{err}");
}

#[test]
fn a_map_that_cannot_be_checked_exits_two() {
    let err = cannot_check("nowhere.lua");
    assert!(err.contains("cannot read nowhere.lua"), "{err}");

    let dir = tempfile::tempdir().unwrap();
    let loose = dir.path().join("loose.lua");
    std::fs::write(&loose, "return 1\n").unwrap();
    let err = cannot_check(loose.to_str().unwrap());
    assert!(err.contains("is not inside a pack"), "{err}");
}
