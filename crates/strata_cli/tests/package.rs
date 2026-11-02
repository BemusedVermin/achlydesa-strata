use std::fs;
use std::path::Path;

use assert_cmd::Command;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/package");

fn package(set: &str, out: &Path, client: &Path) -> assert_cmd::assert::Assert {
    Command::cargo_bin("strata")
        .unwrap()
        .arg("package")
        .arg("--out")
        .arg(out)
        .arg("--client")
        .arg(client)
        .arg(format!("{FIXTURES}/{set}"))
        .assert()
}

fn fake_client(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("client-bin");
    fs::write(&path, b"client").unwrap();
    path
}

#[test]
fn a_public_pack_set_is_bundled_with_a_manifest() {
    let tmp = tempfile::tempdir().unwrap();
    let client = fake_client(tmp.path());
    let out = tmp.path().join("dist");
    package("public", &out, &client).success();

    let manifest = fs::read_to_string(out.join("manifest.txt")).unwrap();
    let lines: Vec<&str> = manifest.lines().collect();
    assert!(lines[0].starts_with("client strata-client"), "{manifest}");
    assert!(lines[1].starts_with("pack base 1.0.0 "), "{manifest}");
    assert!(lines[2].starts_with("pack game 1.0.0 "), "{manifest}");
    assert_eq!(lines[1].rsplit(' ').next().unwrap().len(), 64);
    assert!(lines[3].starts_with("pack-set "), "{manifest}");

    assert!(out.join("packs/base/pack.lua").is_file());
    assert!(out.join("packs/game/data.lua").is_file());
    assert!(
        !out.join("packs/base/tests").exists(),
        "tests are not shipped"
    );
    let name = format!("strata-client{}", std::env::consts::EXE_SUFFIX);
    assert_eq!(fs::read(out.join(name)).unwrap(), b"client");
}

#[test]
fn a_private_pack_on_the_search_path_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let client = fake_client(tmp.path());
    let out = tmp.path().join("dist");
    let run = package("mixed", &out, &client).code(1);
    let err = String::from_utf8_lossy(&run.get_output().stderr).into_owned();
    assert!(err.contains("refusing to package private pack"), "{err}");
    assert!(err.contains("`secret`"), "{err}");
    assert!(
        !out.exists(),
        "nothing is written when packaging is refused"
    );
}

#[test]
fn a_missing_client_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("dist");
    let run = package("public", &out, &tmp.path().join("absent")).code(1);
    let err = String::from_utf8_lossy(&run.get_output().stderr).into_owned();
    assert!(err.contains("client binary not found"), "{err}");
}
