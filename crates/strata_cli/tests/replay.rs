use assert_cmd::Command;

fn strata(args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("strata").unwrap().args(args).assert()
}

fn stderr(out: &assert_cmd::assert::Assert) -> String {
    String::from_utf8_lossy(&out.get_output().stderr).into_owned()
}

#[test]
fn the_replay_commands_are_listed() {
    let out = strata(&["replay", "--help"]).success();
    let text = String::from_utf8_lossy(&out.get_output().stdout).into_owned();
    for name in ["verify", "dump", "to-test"] {
        assert!(text.contains(name), "{name} missing from:\n{text}");
    }
}

#[test]
fn a_missing_replay_is_reported() {
    let out = strata(&["replay", "dump", "no_such_file.replay"]).code(1);
    assert!(stderr(&out).contains("cannot read no_such_file.replay"));
}

#[test]
fn a_file_that_is_not_a_replay_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("junk.replay");
    std::fs::write(&file, [1, 2, 3]).unwrap();
    let path = file.to_str().unwrap();
    let out = strata(&["replay", "dump", path]).code(1);
    assert!(stderr(&out).contains("cannot load"));
}

#[test]
fn verify_and_to_test_need_packs() {
    strata(&["replay", "verify", "x.replay"]).code(2);
    strata(&["replay", "to-test", "x.replay", "--at", "0"]).code(2);
}
