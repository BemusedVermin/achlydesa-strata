use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use strata_kernel::{Sym, Value};
use strata_model::{ArmyEntry, ArmyList, Text};
use strata_session::{Change, LaunchSpec, PlayerView, SimMessage, launch};

use super::*;
use crate::ui::cards::card_models;

/// The first view of a match on the `cards` fixture, whose ruleset adds 1 to every defense from
/// a Lua function.
fn launched_view() -> PlayerView {
    let army = Some(ArmyList {
        name: Text("test".into()),
        format: Sym::new("none"),
        packs: BTreeMap::new(),
        units: vec![ArmyEntry {
            unit: Sym::new("test_soldier"),
            copies: 1,
        }],
    });
    let root = env!("CARGO_MANIFEST_DIR");
    let handle = launch(LaunchSpec {
        roots: vec![PathBuf::from(root).join("../../fixtures/packs/cards")],
        scenario: "duel".into(),
        ruleset: Some("rs".into()),
        seed: 1,
        armies: vec![army.clone(), army],
        commands: 0,
    });
    match handle.recv_timeout(Duration::from_secs(60)) {
        Ok(SimMessage::View(view)) => *view,
        other => panic!("expected a view, got {other:?}"),
    }
}

#[test]
fn the_inspected_trace_reproduces_the_value_the_card_displays() {
    let view = launched_view();
    for card in card_models(&view) {
        for stat in &card.stats {
            let selected = Inspected(Some((card.unit, stat.label.clone())));
            let inspection = inspection(&view, &selected).expect("every stat has a trace");
            let mut value = inspection.base.as_int().unwrap();
            for step in &inspection.steps {
                value = match step.change {
                    Change::Set(n) => n,
                    Change::Add(n) | Change::Final(n) => value + n,
                    Change::Mul(_, 0) => value,
                    Change::Mul(n, d) => value * n / d,
                    ref other => panic!("not a stat change: {other:?}"),
                };
            }
            let clamp = inspection.clamp.unwrap();
            assert_eq!(
                value.clamp(clamp.min, clamp.max),
                stat.value,
                "{}",
                stat.label
            );
            assert_eq!(inspection.result, Value::Int(stat.value));
        }
    }
}

#[test]
fn the_inspector_marks_lua_steps_and_finds_nothing_for_an_unknown_stat() {
    let view = launched_view();
    let unit = view.cards[0].unit;
    let def = inspection(&view, &Inspected(Some((unit, "DEF".into())))).unwrap();
    assert_eq!(def.lines()[2], "+1 from rs (Lua) = 3");
    assert_eq!(def.lua_steps(), 1);
    assert_eq!(
        inspection(&view, &Inspected(Some((unit, "XYZ".into())))),
        None
    );
    assert_eq!(inspection(&view, &Inspected(None)), None);
}
