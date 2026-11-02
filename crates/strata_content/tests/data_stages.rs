use strata_content::{discover_packs, resolve_load_order, run_data_stages};

fn manifest(id: &str, dependencies: &str) -> String {
    format!(
        "return {{ id = '{id}', version = '1.0.0', name = '{id}', engine = '>=0.1, <0.2', \
         dependencies = {{ {dependencies} }} }}"
    )
}

#[test]
fn runs_discovered_packs_in_dependency_order() {
    let dir = tempfile::tempdir().unwrap();
    // `aaa` sorts first but depends on `zzz`, so `zzz` must run first.
    let packs = [
        ("aaa", "zzz = '^1'", "unit { id = 'u', name = 'U' }"),
        ("zzz", "", ""),
    ];
    for (id, dependencies, data) in packs {
        let root = dir.path().join(id);
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("pack.lua"), manifest(id, dependencies)).unwrap();
        std::fs::write(root.join("data.lua"), data).unwrap();
    }
    std::fs::write(
        dir.path().join("zzz/data_updates.lua"),
        "error('zzz updates run after aaa data')",
    )
    .unwrap();
    let found = discover_packs(&[dir.path().to_owned()]).unwrap();
    let order = resolve_load_order(&found).unwrap();
    let error = run_data_stages(&found, &order).err().unwrap();
    assert!(
        error.to_string().contains("zzz/data_updates.lua:1:"),
        "{error}"
    );

    std::fs::remove_file(dir.path().join("zzz/data_updates.lua")).unwrap();
    let stage = run_data_stages(&found, &order).unwrap();
    assert_eq!(stage.records()[0].id, "aaa:u");
}
