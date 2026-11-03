//! Verifying, dumping and turning replays into tests, on the toy pack.

use std::fs;
use std::path::PathBuf;

use strata_model::Command;
use strata_modules::native_modules;

use crate::pack_test::{ExportError, PackTests};
use crate::tests::{attack, pack_tests, play};
use crate::*;

#[test]
fn a_recorded_match_verifies_and_yields_every_commands_events() {
    let tests = pack_tests();
    let (replay, _) = play(&tests, 25);
    let events = tests.replay_events(&replay).unwrap();
    assert_eq!(events.len(), 25);
    assert!(events.iter().all(|batch| !batch.is_empty()));
}

#[test]
fn a_tampered_hash_names_the_exact_command() {
    let tests = pack_tests();
    let (mut replay, _) = play(&tests, 25);
    replay.commands[7].state_hash ^= 1;
    let got = tests.replay_events(&replay).unwrap_err();
    assert!(matches!(
        got,
        ExportError::Replay(ReplayError::HashMismatch { index: 7, .. })
    ));
    assert!(got.to_string().contains("command 7"));
}

#[test]
fn a_tampered_command_is_caught_at_that_command() {
    let tests = pack_tests();
    let (mut replay, _) = play(&tests, 25);
    replay.commands[5].command = attack(1, 2);
    let got = tests.replay_events(&replay).unwrap_err();
    assert!(matches!(
        got,
        ExportError::Replay(
            ReplayError::HashMismatch { index: 5, .. } | ReplayError::Diverged { index: 5 }
        )
    ));
}

#[test]
fn a_replay_for_other_packs_is_refused() {
    let tests = pack_tests();
    let (mut replay, _) = play(&tests, 3);
    replay.setup.packs[0].hash ^= 1;
    let got = tests.replay_events(&replay).unwrap_err();
    assert_eq!(got, ExportError::Replay(ReplayError::Packs));
}

#[test]
fn dump_prints_setup_commands_and_events() {
    let tests = pack_tests();
    let (replay, _) = play(&tests, 3);
    let events = tests.replay_events(&replay).unwrap();
    insta::assert_snapshot!(dump(&replay, &events));
}

#[test]
fn dump_without_packs_leaves_out_the_events() {
    let tests = pack_tests();
    let (replay, _) = play(&tests, 3);
    let text = dump(&replay, &[]);
    assert!(text.contains("#2 player 0"));
    assert!(!text.contains("event "));
}

/// The toy pack with `test` appended to its data, in a folder of its own.
fn pack_with(test: &str) -> (tempfile::TempDir, PackTests) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/packs/toy/toy");
    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("toy");
    fs::create_dir(&pack).unwrap();
    fs::copy(root.join("pack.lua"), pack.join("pack.lua")).unwrap();
    let data = fs::read_to_string(root.join("data.lua")).unwrap();
    fs::write(pack.join("data.lua"), format!("{data}\n{test}")).unwrap();
    let loaded = PackTests::load(&[dir.path().to_path_buf()]).unwrap();
    let modules = native_modules().into_iter();
    (dir, modules.fold(loaded, |t, m| t.with_native(m).unwrap()))
}

#[test]
fn to_test_writes_a_test_that_passes() {
    let tests = pack_tests();
    let (replay, _) = play(&tests, 5);
    let text = tests.to_test(&replay, 0).unwrap();
    insta::assert_snapshot!(text);

    let (_dir, with_test) = pack_with(&text);
    let reports = with_test.run(Some("replay command 0"));
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].outcome.as_ref().err(), None);
}

#[test]
fn a_changed_outcome_fails_the_written_test() {
    let tests = pack_tests();
    let (replay, _) = play(&tests, 5);
    let text = tests.to_test(&replay, 0).unwrap();
    let changed = text.replace(
        "expect.figure('p1.toy:test_archer.a#1', 'reserve')",
        "expect.figure('p1.toy:test_archer.a#1', 'destroyed')",
    );
    assert_ne!(changed, text);

    let (_dir, with_test) = pack_with(&changed);
    let reports = with_test.run(Some("replay command 0"));
    assert!(reports[0].outcome.is_err());
}

#[test]
fn to_test_refuses_what_it_cannot_express() {
    let tests = pack_tests();
    let (mut replay, _) = play(&tests, 3);
    let past = tests.to_test(&replay, 3).unwrap_err();
    assert!(matches!(past, ExportError::Unsupported(_)));

    replay.commands[1].command = Command::EndActivation;
    let got = tests.to_test(&replay, 1).unwrap_err();
    assert!(matches!(got, ExportError::Replay(_)));
}
