use strata_kernel::{PackId, Value};
use strata_lua::PackSource;

use super::*;

fn pack(dir: &std::path::Path, id: &str, source: Option<&str>) -> PackSource {
    let root = dir.join(id);
    std::fs::create_dir_all(&root).unwrap();
    if let Some(source) = source {
        std::fs::write(root.join(ASSETS_FILE), source).unwrap();
    }
    PackSource {
        id: PackId::new(id),
        root,
        dependencies: Vec::new(),
    }
}

#[test]
fn each_pack_with_an_assets_file_is_run_in_load_order() {
    let dir = tempfile::tempdir().unwrap();
    let packs = [
        pack(
            dir.path(),
            "base",
            Some("pixels_per_level = 16\nsprite { key = 'a' }"),
        ),
        pack(dir.path(), "plain", None),
        pack(dir.path(), "broken", Some("sprite {")),
        pack(dir.path(), "odd", Some("terrain { key = 'grass' }")),
    ];
    let read = read_pack_assets(&packs);
    let ids: Vec<_> = read.iter().map(|a| a.pack.as_str()).collect();
    assert_eq!(
        ids,
        ["base", "broken", "odd"],
        "a pack without the file is skipped"
    );
    let base = read[0].declared.as_ref().unwrap();
    assert_eq!(base.globals["pixels_per_level"], Value::Int(16));
    assert_eq!(base.calls.len(), 1);
    assert_eq!(base.calls[0].kind, "sprite");
    assert_eq!(read[0].root, dir.path().join("base"));
    assert!(read[1].declared.is_err(), "a syntax error");
    assert!(read[2].declared.is_err(), "only `sprite` is a constructor");
}

#[test]
fn an_unreadable_assets_file_is_an_error_not_a_missing_one() {
    let dir = tempfile::tempdir().unwrap();
    let odd = pack(dir.path(), "odd", None);
    // A directory where the file should be cannot be read as text.
    std::fs::create_dir(odd.root.join(ASSETS_FILE)).unwrap();
    let read = read_pack_assets(&[odd]);
    assert_eq!(read.len(), 1);
    let error = read[0].declared.as_ref().unwrap_err();
    assert!(error.contains("cannot read"), "{error}");
}
