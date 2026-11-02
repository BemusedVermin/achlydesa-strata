use assert_cmd::Command;

fn lint(pack: &str) -> assert_cmd::assert::Assert {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/lint");
    Command::cargo_bin("strata")
        .unwrap()
        .current_dir(dir)
        .args(["lint", pack])
        .assert()
}

fn stdout(assert: &assert_cmd::assert::Assert) -> String {
    let text = String::from_utf8_lossy(&assert.get_output().stdout).replace("\r\n", "\n");
    // Selene may or may not be installed on the machine running the tests.
    text.lines()
        .filter(|line| !line.starts_with("notice: selene"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn every_check_names_file_line_and_fix() {
    let out = lint("bad").code(1);
    let text = stdout(&out);
    insta::assert_snapshot!(text);
    assert!(text.contains("bad/game/rules/division.lua:2:"), "{text}");
    assert!(text.contains("bad/game/units/hooks.lua:8:"), "{text}");
    assert!(text.contains("bad/game/units/drift.lua:2:"), "{text}");
    assert!(text.contains("bad/game/units/syntax.lua"), "{text}");
    assert_eq!(text.matches("  fix: ").count(), 4, "{text}");
}

#[test]
fn clean_scripts_pass() {
    let out = lint("ok").success();
    let text = stdout(&out);
    assert!(text.contains("0 error(s), 0 warning(s)"), "{text}");
}

#[test]
fn warnings_do_not_fail_the_run() {
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("game");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(pack.join("pack.lua"), "pack {}\n").unwrap();
    std::fs::write(pack.join("a.lua"), "aura { amount = 7, text = \"x\" }\n").unwrap();
    let out = Command::cargo_bin("strata")
        .unwrap()
        .arg("lint")
        .arg(dir.path())
        .assert()
        .success();
    let text = stdout(&out);
    assert!(text.contains("warning:"), "{text}");
}

#[test]
fn division_in_a_data_file_is_not_flagged() {
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("game");
    std::fs::create_dir_all(&pack).unwrap();
    std::fs::write(pack.join("pack.lua"), "pack {}\n").unwrap();
    std::fs::write(pack.join("data.lua"), "local x = 4 / 2\n").unwrap();
    Command::cargo_bin("strata")
        .unwrap()
        .arg("lint")
        .arg(dir.path())
        .assert()
        .success();
}
