use strata_kernel::Value;

use super::*;
use crate::ui::cards::card_models;
use crate::ui::tests::launched_view;

#[test]
fn the_inspected_trace_reproduces_the_value_the_card_displays() {
    let view = launched_view();
    for card in card_models(&view) {
        for stat in &card.stats {
            let selected = Inspected(Some((card.unit, stat.label.clone())));
            let inspection = inspection(&view, &selected).expect("every stat has a trace");
            // `recompute` checks every running value, the unclamped value and the result.
            assert_eq!(inspection.recompute(), Ok(stat.value), "{}", stat.label);
            assert_eq!(inspection.result, Value::Int(stat.value));
        }
    }
}

#[test]
fn the_inspector_marks_lua_steps_and_names_the_traced_figure() {
    let view = launched_view();
    let unit = view.cards[0].unit;
    let def = inspection(&view, &Inspected(Some((unit, "DEF".into())))).unwrap();
    let figure = view.cards[0].figures[0].figure.0;
    assert_eq!(def.lines()[0], format!("defense of figure {figure}"));
    assert_eq!(def.lines()[2], "+1 from rs (Lua) = 3");
    assert_eq!(def.lua_steps(), 1);
}

#[test]
fn the_inspector_tells_a_missing_selection_from_an_untraced_stat() {
    let mut view = launched_view();
    let unit = view.cards[0].unit;
    let missing = |view: &PlayerView, label: &str| {
        inspection(view, &Inspected(Some((unit, label.into())))).unwrap_err()
    };
    assert_eq!(missing(&view, "XYZ"), NoTrace::NotFound);
    assert_eq!(inspection(&view, &Inspected(None)), Err(NoTrace::NotFound));
    view.cards[0].stats[0].trace = None;
    let label = view.cards[0].stats[0].label.clone();
    assert_eq!(missing(&view, &label), NoTrace::Untraced);
    assert_ne!(NoTrace::NotFound.message(), NoTrace::Untraced.message());
}
