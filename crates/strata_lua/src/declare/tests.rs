use std::collections::BTreeMap;

use strata_kernel::{Sym, Value};

use super::*;

const ASSETS: &str = r#"
pixels_per_level = 12
hex_width_px = 64

sprite {
  key = "sprite.test_soldier",
  frame = { w = 8, h = 16 },
}
sprite { key = "sprite.test_archer" }
"#;

#[test]
fn constructor_calls_and_requested_globals_come_back_in_order() {
    let out = declare(
        "t/assets.lua",
        ASSETS,
        &["sprite"],
        &["pixels_per_level", "unset"],
    )
    .unwrap();
    assert_eq!(
        out.globals.len(),
        1,
        "unset globals and unrequested ones are left out"
    );
    assert_eq!(out.globals["pixels_per_level"], Value::Int(12));
    let keys: Vec<_> = out
        .calls
        .iter()
        .map(|c| c.value.get("key").unwrap().clone())
        .collect();
    let key = |k: &str| Value::Str(Sym::new(k));
    assert_eq!(
        keys,
        vec![key("sprite.test_soldier"), key("sprite.test_archer")]
    );
    assert_eq!(
        (out.calls[0].kind.as_str(), out.calls[0].line),
        ("sprite", 5)
    );
    let frame = out.calls[0].value.get("frame").unwrap();
    let expected = BTreeMap::from([("h".into(), Value::Int(16)), ("w".into(), Value::Int(8))]);
    assert_eq!(*frame, Value::Map(expected));
}

#[test]
fn unknown_constructors_floats_and_bare_values_are_errors() {
    for source in ["terrain { key = 'x' }", "sprite { fps = 1.5 }", "sprite(3)"] {
        let error = declare("t/assets.lua", source, &["sprite"], &[]);
        assert!(error.is_err(), "{source} should fail");
    }
}

#[test]
fn files_cannot_reach_outside_the_sandbox() {
    assert!(declare("t/assets.lua", "io.open('x')", &[], &[]).is_err());
    assert!(declare("t/assets.lua", "while true do end", &[], &[]).is_err());
}
