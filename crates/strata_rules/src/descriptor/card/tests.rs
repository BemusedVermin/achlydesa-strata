use std::collections::BTreeMap;

use strata_kernel::{Sym, Value};
use strata_model::Descriptor;

use super::super::compile;
use super::{CardTextError, card_text};

fn s(text: &str) -> Value {
    Value::Str(Sym::new(text))
}

fn n(i: i64) -> Value {
    Value::Int(i)
}

fn tagged(namespace: &str, name: &str, rest: &[(&str, Value)]) -> Value {
    let mut map = BTreeMap::from([(Sym::new(namespace), s(name))]);
    map.extend(rest.iter().map(|(k, v)| (Sym::new(k), v.clone())));
    Value::Map(map)
}

fn params(pairs: &[(&str, Value)]) -> BTreeMap<Sym, Value> {
    pairs
        .iter()
        .map(|(k, v)| (Sym::new(k), v.clone()))
        .collect()
}

fn phrase(value: Value) -> String {
    compile(&Descriptor(value)).unwrap().phrase()
}

/// One line per descriptor: its plain-data form, then its phrase.
fn phrases(all: &[Value]) -> String {
    let line = |v: &Value| format!("{v:?}\n  => {}", phrase(v.clone()));
    all.iter().map(line).collect::<Vec<_>>().join("\n")
}

#[test]
fn shieldwall_renders_as_the_design_says() {
    let to = tagged(
        "sel",
        "allies",
        &[("adjacent_to", s("self")), ("trait", s("soldier"))],
    );
    let text = card_text(
        "{to} add {amount} to their {stat}.",
        &params(&[("to", to), ("amount", n(1)), ("stat", s("defense"))]),
    );
    assert_eq!(
        text.unwrap(),
        "Adjacent friendly soldier figures add 1 to their defense."
    );
}

#[test]
fn every_effect_has_a_phrase() {
    let enemies = tagged("sel", "enemies", &[("adjacent_to", s("self"))]);
    let all = [
        tagged("eff", "wound", &[]),
        tagged(
            "eff",
            "wound",
            &[("amount", n(2)), ("who", enemies.clone())],
        ),
        tagged(
            "eff",
            "wound",
            &[("amount", tagged("val", "stat", &[("stat", s("attack"))]))],
        ),
        tagged("eff", "heal", &[("amount", n(2))]),
        tagged("eff", "place", &[("within", n(3))]),
        tagged(
            "eff",
            "place",
            &[("who", s("self")), ("within", n(1)), ("of", s("target"))],
        ),
        tagged("eff", "remove", &[]),
        tagged("eff", "move", &[("up_to", n(1))]),
        tagged("eff", "move", &[("who", enemies.clone()), ("up_to", n(2))]),
        tagged("eff", "grant_slot", &[("slot", s("attack"))]),
        tagged("eff", "grant_slot", &[("slot", s("move")), ("count", n(2))]),
        tagged("eff", "add_marker", &[("marker", s("stunned"))]),
        tagged(
            "eff",
            "add_marker",
            &[("marker", s("stunned")), ("rounds", n(1))],
        ),
        tagged(
            "eff",
            "add_marker",
            &[("marker", s("burning")), ("count", n(2)), ("rounds", n(3))],
        ),
        tagged("eff", "take_control", &[]),
        tagged("eff", "cancel_wounds", &[]),
        tagged("eff", "excess_blocks_wound_attacker", &[]),
    ];
    insta::assert_snapshot!(phrases(&all));
}

#[test]
fn every_selector_and_condition_that_needs_a_module_has_a_phrase() {
    let enemies = tagged("sel", "enemies", &[]);
    let all = [
        s("target"),
        tagged("sel", "enemies", &[("adjacent_to", s("target"))]),
        tagged("sel", "enemies", &[("engaged", Value::Bool(true))]),
        tagged(
            "sel",
            "allies",
            &[("engaged", Value::Bool(false)), ("trait", s("soldier"))],
        ),
        tagged("sel", "self", &[("engaged", Value::Bool(true))]),
        tagged("sel", "enemies", &[("in_los", s("self"))]),
        tagged("cond", "engaged", &[]),
        tagged("cond", "engaged", &[("who", enemies.clone())]),
        tagged("cond", "in_los", &[("to", s("target"))]),
        tagged(
            "cond",
            "in_los",
            &[("from", tagged("sel", "allies", &[])), ("to", enemies)],
        ),
        tagged("cond", "attack_kind", &[("is", s("melee"))]),
        tagged("cond", "attack_kind", &[("is", s("ranged"))]),
        tagged("cond", "higher_than", &[("than", s("target"))]),
    ];
    insta::assert_snapshot!(phrases(&all));
}

/// The cards of design §8.3, with templates an archetype library might use.
#[test]
fn archetype_cards_render_from_templates() {
    let soldiers = tagged("sel", "my_units", &[("trait", s("soldier"))]);
    let cards = [
        (
            "{to} add {amount} to their {stat}.",
            params(&[
                (
                    "to",
                    tagged(
                        "sel",
                        "allies",
                        &[("adjacent_to", s("self")), ("trait", s("soldier"))],
                    ),
                ),
                ("amount", n(1)),
                ("stat", s("defense")),
            ]),
        ),
        (
            "After {after}ing, roll the d20; on {at_least} or more, {effect}.",
            params(&[
                ("after", s("attack")),
                ("at_least", n(16)),
                (
                    "effect",
                    tagged("eff", "grant_slot", &[("slot", s("attack"))]),
                ),
            ]),
        ),
        (
            "Add {amount} to {stat} while {when}.",
            params(&[
                ("amount", n(1)),
                ("stat", s("attack")),
                (
                    "when",
                    tagged("cond", "higher_than", &[("than", s("target"))]),
                ),
            ]),
        ),
        (
            "{when}, roll the d20; on {at_least} or more, {effect}.",
            params(&[
                (
                    "when",
                    tagged("cond", "attack_kind", &[("is", s("ranged"))]),
                ),
                ("at_least", n(17)),
                ("effect", tagged("eff", "cancel_wounds", &[])),
            ]),
        ),
        (
            "{when}, after the defense roll: {effect}.",
            params(&[
                ("when", tagged("cond", "attack_kind", &[("is", s("melee"))])),
                ("effect", tagged("eff", "excess_blocks_wound_attacker", &[])),
            ]),
        ),
        (
            "Ignores {ignores}.",
            params(&[(
                "ignores",
                Value::List(vec![s("elevation"), s("figures"), s("stop_terrain")]),
            )]),
        ),
        (
            "Before or after this unit's turn, you may activate {with}.",
            params(&[("with", soldiers)]),
        ),
        (
            "Instead of attacking, choose {targets:sel}: {effect}.",
            params(&[
                ("targets", tagged("sel", "allies", &[("within", n(1))])),
                ("effect", tagged("eff", "heal", &[("amount", n(2))])),
            ]),
        ),
        (
            "{who:sel} cannot be targeted. {{Braces}} stay.",
            params(&[("who", s("self"))]),
        ),
    ];
    let rendered: Vec<String> = cards
        .iter()
        .map(|(template, p)| format!("{template}\n  => {}", card_text(template, p).unwrap()))
        .collect();
    insta::assert_snapshot!(rendered.join("\n"));
}

#[test]
fn a_bare_literal_is_worded_as_the_kind_asked() {
    let p = params(&[("who", s("self")), ("when", Value::Bool(true)), ("n", n(3))]);
    assert_eq!(card_text("{who}", &p).unwrap(), "Self");
    assert_eq!(card_text("{who:sel}", &p).unwrap(), "This figure");
    assert_eq!(card_text("x {when:cond}", &p).unwrap(), "X always");
    assert_eq!(card_text("x {n:val}", &p).unwrap(), "X 3");
    let effect = params(&[("e", s("cancel_wounds"))]);
    assert_eq!(
        card_text("{e:eff}.", &effect).unwrap(),
        "The attack deals no wounds."
    );
}

#[test]
fn bad_templates_and_params_are_errors() {
    let p = params(&[
        ("a", n(1)),
        ("f", Value::Nil),
        ("d", tagged("sel", "wizards", &[])),
    ]);
    assert_eq!(card_text("{a", &p), Err(CardTextError::Unmatched(0)));
    assert_eq!(card_text("x }", &p), Err(CardTextError::Unmatched(2)));
    assert_eq!(
        card_text("{b}", &p),
        Err(CardTextError::Unknown("b".into()))
    );
    let kind = CardTextError::Kind {
        param: "a".into(),
        kind: "num".into(),
    };
    assert_eq!(card_text("{a:num}", &p), Err(kind));
    assert!(matches!(
        card_text("{f}", &p),
        Err(CardTextError::Unworded { .. })
    ));
    let unknown = card_text("{d}", &p).unwrap_err().to_string();
    assert!(unknown.contains("sel.wizards"), "{unknown}");
}

#[test]
fn rendering_is_pure() {
    let p = params(&[("to", tagged("sel", "enemies", &[("within", n(2))]))]);
    let once = card_text("{to} are slowed.", &p);
    assert_eq!(once, card_text("{to} are slowed.", &p));
    assert_eq!(
        once.unwrap(),
        "Enemy figures within 2 of this figure are slowed."
    );
}

#[test]
fn a_sentence_after_a_phrase_starts_with_a_capital() {
    let p = params(&[("e", tagged("eff", "remove", &[]))]);
    assert_eq!(
        card_text("Roll. {e}. Then {e}.", &p).unwrap(),
        "Roll. Remove the target from the game. Then remove the target from the game."
    );
}
