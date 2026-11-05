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

#[test]
fn a_file_that_defines_no_map_is_an_error() {
    strata(&["map", "check", "t/data.lua"]).code(1);
    strata(&["map", "check", "nowhere.lua"]).code(1);
}
