use assert_cmd::Command;

fn strata(args: &[&str]) -> assert_cmd::assert::Assert {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/pack_test");
    Command::cargo_bin("strata")
        .unwrap()
        .current_dir(dir)
        .args(args)
        .assert()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

#[test]
fn passing_tests_print_a_summary_and_exit_zero() {
    let out = strata(&["pack", "test", "passing"]).success();
    insta::assert_snapshot!(text(&out.get_output().stdout));
}

#[test]
fn a_failing_test_prints_its_failure_and_exits_nonzero() {
    let out = strata(&["pack", "test", "failing"]).code(1);
    insta::assert_snapshot!(text(&out.get_output().stdout));
}

#[test]
fn the_filter_runs_only_matching_tests() {
    let out = strata(&["pack", "test", "failing", "--filter", "base defense"]).success();
    let stdout = text(&out.get_output().stdout);
    assert!(stdout.contains("1 passed, 0 failed"), "{stdout}");
}

#[test]
fn packs_that_do_not_load_exit_nonzero_with_a_message() {
    let out = strata(&["pack", "test", "nowhere"]).code(1);
    let err = text(&out.get_output().stderr);
    assert!(err.contains("no tests ran"), "{err}");
}

#[test]
fn the_toy_pack_passes_for_both_combat_implementations() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/packs");
    let out = Command::cargo_bin("strata")
        .unwrap()
        .current_dir(dir)
        .args(["pack", "test", "toy"])
        .assert()
        .success();
    let stdout = text(&out.get_output().stdout);
    assert!(stdout.contains("8 passed, 0 failed"), "{stdout}");
    assert!(
        stdout.contains("toy_rust") && stdout.contains("toy_lua"),
        "{stdout}"
    );
}
