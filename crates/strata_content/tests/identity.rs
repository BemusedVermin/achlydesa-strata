use std::path::{Path, PathBuf};

use strata_content::{ContentRegistry, load};
use strata_kernel::PackId;

fn write_pack(root: &Path, id: &str, version: &str, dependencies: &str, data: &str) {
    let dir = root.join(id);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = format!(
        "return {{ id = '{id}', version = '{version}', name = '{id}', engine = '>=0.1, <0.2', \
         dependencies = {{ {dependencies} }} }}"
    );
    std::fs::write(dir.join("pack.lua"), manifest).unwrap();
    std::fs::write(dir.join("data.lua"), data).unwrap();
}

const BASE: &str = r#"
terrain { id = 'grass', name = 'Grass', standable = true }
faction { id = 'brass', name = 'Brass', color = '#B5893A' }
"#;

const GAME: &str = r#"
terrain { id = 'rock', name = 'Rock', standable = false }
"#;

fn fixture() -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().unwrap();
    write_pack(dir.path(), "base", "1.0.0", "", BASE);
    write_pack(dir.path(), "game", "1.0.0", "base = '^1'", GAME);
    let roots = vec![dir.path().to_owned()];
    (dir, roots)
}

fn loaded(roots: &[PathBuf]) -> ContentRegistry {
    load(roots).unwrap()
}

fn hash_of(registry: &ContentRegistry, pack: &str) -> String {
    let id = PackId::new(pack);
    let identity = registry.pack_identities().iter().find(|i| i.id == id);
    identity.unwrap().content_hash.to_string()
}

#[test]
fn identities_list_each_pack_in_load_order() {
    let (_dir, roots) = fixture();
    let registry = loaded(&roots);
    let identities = registry.pack_identities();
    let ids: Vec<_> = identities.iter().map(|i| i.id.to_string()).collect();
    assert_eq!(ids, ["base", "game"]);
    assert_eq!(identities[0].version.to_string(), "1.0.0");
    assert_ne!(identities[0].content_hash, identities[1].content_hash);
}

#[test]
fn reloading_unchanged_packs_gives_the_same_hashes() {
    let (_dir, roots) = fixture();
    let first = loaded(&roots);
    let second = loaded(&roots);
    assert_eq!(first.pack_identities(), second.pack_identities());
    assert_eq!(first.set_hash(), second.set_hash());
}

#[test]
fn copies_of_the_same_packs_in_different_directories_match() {
    let (_a, roots_a) = fixture();
    let (_b, roots_b) = fixture();
    assert_eq!(loaded(&roots_a).set_hash(), loaded(&roots_b).set_hash());
}

#[test]
fn changing_a_definition_changes_the_hash() {
    let (dir, roots) = fixture();
    let before = loaded(&roots);
    let changed = BASE.replace("standable = true", "standable = false");
    write_pack(dir.path(), "base", "1.0.0", "", &changed);
    let after = loaded(&roots);
    assert_ne!(hash_of(&before, "base"), hash_of(&after, "base"));
    assert_eq!(hash_of(&before, "game"), hash_of(&after, "game"));
    assert_ne!(before.set_hash(), after.set_hash());
}

#[test]
fn changing_a_script_byte_changes_the_hash() {
    let (dir, roots) = fixture();
    let rules = dir.path().join("game/rules");
    std::fs::create_dir_all(&rules).unwrap();
    std::fs::write(rules.join("extra.lua"), "return 1").unwrap();
    let before = loaded(&roots);
    std::fs::write(rules.join("extra.lua"), "return 2").unwrap();
    let after = loaded(&roots);
    assert_ne!(hash_of(&before, "game"), hash_of(&after, "game"));
    assert_eq!(hash_of(&before, "base"), hash_of(&after, "base"));
    assert_ne!(before.set_hash(), after.set_hash());
}

#[test]
fn a_comment_in_data_lua_changes_the_hash_but_a_pack_test_does_not() {
    let (dir, roots) = fixture();
    let before = loaded(&roots);
    let tests = dir.path().join("game/tests");
    std::fs::create_dir_all(&tests).unwrap();
    std::fs::write(tests.join("a.lua"), "-- test").unwrap();
    assert_eq!(before.set_hash(), loaded(&roots).set_hash());
    write_pack(
        dir.path(),
        "game",
        "1.0.0",
        "base = '^1'",
        &format!("-- c\n{GAME}"),
    );
    assert_ne!(before.set_hash(), loaded(&roots).set_hash());
}

#[test]
fn a_version_bump_changes_the_set_hash() {
    let (dir, roots) = fixture();
    let before = loaded(&roots);
    write_pack(dir.path(), "game", "1.0.1", "base = '^1'", GAME);
    let after = loaded(&roots);
    assert_eq!(hash_of(&before, "base"), hash_of(&after, "base"));
    assert_ne!(before.set_hash(), after.set_hash());
    assert_eq!(after.pack_identities()[1].version.to_string(), "1.0.1");
}
