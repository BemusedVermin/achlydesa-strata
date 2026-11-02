use super::{generate, kinds};
use crate::data::registry_kinds;

#[test]
fn covers_every_registered_kind() {
    assert_eq!(kinds(), registry_kinds());
}

#[test]
fn regenerates_identically() {
    assert_eq!(generate(), generate());
}

fn text(name: &str) -> String {
    generate()
        .into_iter()
        .find(|f| f.name == name)
        .unwrap()
        .text
}

#[test]
fn unit_stub() {
    insta::assert_snapshot!(text("unit.lua"));
}

#[test]
fn terrain_stub() {
    insta::assert_snapshot!(text("terrain.lua"));
}

#[test]
fn ruleset_stub() {
    insta::assert_snapshot!(text("ruleset.lua"));
}
