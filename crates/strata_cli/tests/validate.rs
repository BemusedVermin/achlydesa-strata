use assert_cmd::Command;

fn strata(args: &[&str]) -> assert_cmd::assert::Assert {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/validate");
    Command::cargo_bin("strata")
        .unwrap()
        .current_dir(dir)
        .args(args)
        .assert()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

/// Content hashes cover script bytes, which line-ending conversion can change.
fn redact_hashes(text: &str) -> String {
    let line = |line: &str| {
        let words = line.split(' ').map(|word| {
            let hex = word.len() == 64 && word.bytes().all(|b| b.is_ascii_hexdigit());
            if hex { "<hash>" } else { word }
        });
        words.collect::<Vec<_>>().join(" ")
    };
    text.lines().map(line).collect::<Vec<_>>().join(
        "
",
    )
}

#[test]
fn a_valid_pack_set_prints_its_hash_and_exits_zero() {
    let out = strata(&["pack", "validate", "ok"]).success();
    insta::assert_snapshot!(redact_hashes(&text(&out.get_output().stdout)));
}

#[test]
fn every_error_names_its_file_and_line() {
    let out = strata(&["pack", "validate", "bad"]).code(1);
    let err = text(&out.get_output().stderr);
    insta::assert_snapshot!(err);
    for line in [2, 3, 4] {
        assert!(err.contains(&format!("bad/game/data.lua:{line}")), "{err}");
    }
}

#[test]
fn a_script_error_names_its_file_and_line() {
    let out = strata(&["pack", "validate", "script"]).code(1);
    let err = text(&out.get_output().stderr);
    assert!(err.contains("oops/data.lua:2"), "{err}");
}

#[test]
fn a_missing_path_is_reported_and_exits_one() {
    strata(&["pack", "validate", "nowhere"]).code(1);
}

#[test]
fn usage_errors_exit_two() {
    strata(&["pack", "validate"]).code(2);
    strata(&["bogus"]).code(2);
}

#[test]
fn unimplemented_commands_say_so() {
    let out = strata(&["lint", "x"]).code(1);
    assert!(text(&out.get_output().stderr).contains("not implemented"));
    strata(&["pack", "test", "x"]).code(1);
}
