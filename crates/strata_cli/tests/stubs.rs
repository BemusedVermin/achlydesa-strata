use assert_cmd::Command;

fn run(out: &std::path::Path) {
    Command::cargo_bin("strata")
        .unwrap()
        .args(["stubs", "--out"])
        .arg(out)
        .assert()
        .success();
}

#[test]
fn writes_one_file_per_kind_identically() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    run(a.path());
    run(&b.path().join("nested"));
    let names = |dir: &std::path::Path| {
        let mut v: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        v.sort();
        v
    };
    let nested = b.path().join("nested");
    assert_eq!(names(a.path()), names(&nested));
    assert_eq!(names(a.path()).len(), 21);
    let unit = |d: &std::path::Path| std::fs::read_to_string(d.join("unit.lua")).unwrap();
    assert_eq!(unit(a.path()), unit(&nested));
}

#[test]
fn template_luarc_points_at_the_stubs() {
    let text = include_str!("../../../templates/pack/.luarc.json");
    assert!(text.contains(".strata/stubs"));
}
