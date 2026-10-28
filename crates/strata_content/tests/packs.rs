use std::path::PathBuf;

use strata_content::{
    ENGINE_VERSION, LoadedPack, PackError, Visibility, discover_packs, resolve_load_order,
};
use strata_kernel::PackId;

fn root(scenario: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/packs")
        .join(scenario)
}

fn discover(scenario: &str) -> Result<Vec<LoadedPack>, PackError> {
    discover_packs(&[root(scenario)])
}

fn order(scenario: &str) -> Result<Vec<String>, PackError> {
    let ids = resolve_load_order(&discover(scenario)?)?;
    Ok(ids.iter().map(|id| id.to_string()).collect())
}

#[test]
fn engine_version_is_valid_semver() {
    semver::Version::parse(ENGINE_VERSION).unwrap();
}

#[test]
fn parses_every_manifest_field_and_skips_non_packs() {
    let packs = discover("full").unwrap();
    assert_eq!(packs.len(), 1);
    let m = &packs[0].manifest;
    assert_eq!(m.id, PackId::new("p"));
    assert_eq!(m.version.to_string(), "0.3.0");
    assert_eq!(m.name, "Full pack");
    assert_eq!(m.engine.to_string(), ">=0.1, <0.2");
    assert_eq!(m.dependencies[&PackId::new("q")].to_string(), "^0.1");
    assert_eq!(m.visibility, Visibility::Private);
    assert!(packs[0].file.ends_with("pack.lua"));
}

#[test]
fn optional_fields_default() {
    let packs = discover("diamond").unwrap();
    let base = packs.iter().find(|p| p.manifest.id.as_str() == "base");
    let base = base.unwrap();
    assert!(base.manifest.dependencies.is_empty());
    assert_eq!(base.manifest.visibility, Visibility::Public);
}

#[test]
fn diamond_orders_dependencies_first_and_breaks_ties_by_id() {
    assert_eq!(
        order("diamond").unwrap(),
        ["alpha", "base", "left", "right", "top"]
    );
}

#[test]
fn resolution_ignores_input_order() {
    let packs = discover("diamond").unwrap();
    let expected = resolve_load_order(&packs).unwrap();
    let mut reversed = packs.clone();
    reversed.reverse();
    assert_eq!(resolve_load_order(&reversed).unwrap(), expected);
    for shift in 1..packs.len() {
        let mut rotated = packs.clone();
        rotated.rotate_left(shift);
        assert_eq!(resolve_load_order(&rotated).unwrap(), expected);
    }
}

#[test]
fn errors_do_not_depend_on_input_order() {
    let mut packs = discover("cycle").unwrap();
    let first = resolve_load_order(&packs).unwrap_err().to_string();
    packs.reverse();
    assert_eq!(resolve_load_order(&packs).unwrap_err().to_string(), first);
}

#[test]
fn several_roots_are_merged() {
    let packs = discover_packs(&[root("full"), root("diamond")]).unwrap();
    assert_eq!(packs.len(), 6);
}

#[test]
fn missing_root_is_an_io_error() {
    let err = discover("no_such_scenario").unwrap_err();
    assert!(matches!(err, PackError::Io { .. }));
}

#[test]
fn missing_dependency_names_pack_and_file() {
    let err = order("missing_dependency").unwrap_err();
    assert!(
        matches!(&err, PackError::MissingDependency { pack, dependency, .. }
        if pack.as_str() == "a" && dependency.as_str() == "ghost")
    );
    assert!(err.to_string().contains("pack.lua"));
}

#[test]
fn version_mismatch_names_both_versions() {
    let err = order("version_mismatch").unwrap_err();
    let message = err.to_string();
    assert!(matches!(err, PackError::VersionMismatch { .. }));
    assert!(
        message.contains("^2") && message.contains("1.5.0"),
        "{message}"
    );
}

#[test]
fn duplicate_ids_name_both_files() {
    let err = order("duplicate").unwrap_err();
    let message = err.to_string();
    assert!(matches!(err, PackError::Duplicate { .. }));
    assert!(message.contains("same") && message.contains("one") && message.contains("two"));
}

#[test]
fn cycle_is_reported_without_the_packs_that_merely_depend_on_it() {
    let err = order("cycle").unwrap_err();
    assert!(matches!(&err, PackError::Cycle { cycle, .. }
        if cycle == &["a", "b", "c", "a"]));
    assert!(err.to_string().contains("a -> b -> c -> a"));
}

#[test]
fn engine_mismatch_names_pack_and_range() {
    let err = order("engine_mismatch").unwrap_err();
    assert!(matches!(&err, PackError::EngineMismatch { pack, .. } if pack.as_str() == "a"));
    assert!(err.to_string().contains(">=0.2, <0.3"));
}

#[test]
fn invalid_manifests_are_rejected_with_the_file_named() {
    for (scenario, expect) in [
        ("unknown_field", "unknown field `colour`"),
        ("missing_field", "missing field `name`"),
        ("bad_version", "`version`"),
        ("bad_id", "`id`"),
        ("bad_visibility", "`visibility`"),
    ] {
        let err = discover(scenario).unwrap_err();
        assert!(
            matches!(&err, PackError::Manifest { .. }),
            "{scenario}: {err}"
        );
        let message = err.to_string();
        assert!(message.contains(expect), "{scenario}: {message}");
        assert!(message.contains("pack.lua"), "{scenario}: {message}");
    }
}

#[test]
fn manifests_must_return_one_table() {
    let err = discover("two_values").unwrap_err();
    assert!(matches!(err, PackError::Script { .. }));
    let err = discover("not_a_table").unwrap_err();
    assert!(matches!(err, PackError::Manifest { .. }));
}

#[test]
fn manifest_runs_with_no_globals() {
    let err = discover("empty_environment").unwrap_err();
    assert!(matches!(&err, PackError::Script { message, .. } if message.contains("string")));
}

#[test]
fn floats_in_a_manifest_are_rejected() {
    let err = discover("float_value").unwrap_err();
    assert!(matches!(err, PackError::Script { .. }));
}
