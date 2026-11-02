use std::collections::BTreeMap;

use strata_kernel::{Sym, Value};
use strata_model::{AbilityDef, Descriptor};

use super::*;

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

fn sel(name: &str, rest: &[(&str, Value)]) -> Value {
    tagged("sel", name, rest)
}

fn cond(name: &str, rest: &[(&str, Value)]) -> Value {
    tagged("cond", name, rest)
}

fn val(name: &str, rest: &[(&str, Value)]) -> Value {
    tagged("val", name, rest)
}

fn list(items: &[Value]) -> Value {
    Value::List(items.to_vec())
}

fn text(descriptor: Value) -> String {
    compile(&Descriptor(descriptor)).unwrap().phrase()
}

fn node(descriptor: Value) -> Node {
    compile(&Descriptor(descriptor)).unwrap()
}

#[test]
fn every_selector_compiles_to_its_typed_form() {
    for (name, base) in [
        ("self", Base::Own),
        ("allies", Base::Allies),
        ("enemies", Base::Enemies),
        ("my_units", Base::MyUnits),
        ("occupant", Base::Occupant),
    ] {
        let Node::Sel(sel) = node(sel(name, &[])) else {
            panic!("{name} is not a selector")
        };
        assert_eq!(sel.base, base);
    }
    assert_eq!(node(s("self")), Node::Sel(Sel::own()));
}

#[test]
fn selector_filters_compile() {
    let fields = [
        ("trait", s("soldier")),
        ("kind", s("hero")),
        ("adjacent_to", s("self")),
        ("within", n(3)),
        ("of", sel("allies", &[])),
    ];
    let Node::Sel(sel) = node(sel("enemies", &fields)) else {
        panic!("not a selector")
    };
    assert_eq!(sel.trait_value, Some(Sym::new("soldier")));
    assert_eq!(sel.kind, Some(Sym::new("hero")));
    assert_eq!(sel.adjacent_to.as_deref(), Some(&Sel::own()));
    assert_eq!(sel.within, Some(3));
    assert_eq!(sel.of.as_ref().map(|o| o.base), Some(Base::Allies));
}

#[test]
fn every_condition_compiles_to_its_typed_form() {
    let pair = list(&[Value::Bool(true), cond("phase", &[("is", s("turns"))])]);
    let enemies = sel("enemies", &[]);
    let cmp = [("a", n(1)), ("b", n(2))];
    let compiled = |name: &str, fields: &[(&str, Value)]| match node(cond(name, fields)) {
        Node::Cond(c) => c,
        other => panic!("{other:?} is not a condition"),
    };
    assert_eq!(node(Value::Bool(true)), Node::Cond(Cond::Const(true)));
    assert!(matches!(compiled("all", &[("of", pair.clone())]), Cond::All(v) if v.len() == 2));
    assert!(matches!(
        compiled("any", &[("of", pair.clone())]),
        Cond::Any(_)
    ));
    assert!(matches!(compiled("none", &[("of", pair)]), Cond::None(_)));
    assert!(matches!(
        compiled("not", &[("of", Value::Bool(false))]),
        Cond::Not(_)
    ));
    for (name, op) in [
        ("eq", Cmp::Eq),
        ("ne", Cmp::Ne),
        ("lt", Cmp::Lt),
        ("lte", Cmp::Lte),
        ("gt", Cmp::Gt),
        ("gte", Cmp::Gte),
    ] {
        assert!(matches!(compiled(name, &cmp), Cond::Compare(got, ..) if got == op));
    }
    let count = compiled("count", &[("of", enemies.clone())]);
    assert!(matches!(
        count,
        Cond::Count {
            min: 1,
            max: None,
            ..
        }
    ));
    assert_eq!(
        compiled("phase", &[("is", s("turns"))]),
        Cond::Phase(Sym::new("turns"))
    );
    let terrain = compiled("on_terrain", &[("terrain", s("rock"))]);
    assert!(matches!(terrain, Cond::OnTerrain { who, .. } if who == Sel::own()));
    let higher = compiled("higher_than", &[("than", enemies)]);
    assert!(matches!(higher, Cond::HigherThan { by: 1, .. }));
}

#[test]
fn every_value_compiles_to_its_typed_form() {
    let enemies = sel("enemies", &[]);
    let numbers = list(&[n(1), n(2)]);
    let compiled = |name: &str, fields: &[(&str, Value)]| match node(val(name, fields)) {
        Node::Val(v) => v,
        other => panic!("{other:?} is not a value"),
    };
    assert_eq!(node(n(4)), Node::Val(Val::Lit(4)));
    assert!(matches!(
        compiled("stat", &[("stat", s("defense"))]),
        Val::Stat { .. }
    ));
    assert!(matches!(
        compiled("count", &[("of", enemies.clone())]),
        Val::Count(_)
    ));
    assert!(matches!(
        compiled("distance", &[("to", enemies.clone())]),
        Val::Distance(_)
    ));
    assert!(matches!(
        compiled("level_difference", &[("to", enemies)]),
        Val::LevelDifference(_)
    ));
    assert!(matches!(compiled("sum", &[("of", numbers.clone())]), Val::Sum(v) if v.len() == 2));
    assert!(matches!(
        compiled("min", &[("of", numbers.clone())]),
        Val::Min(_)
    ));
    assert!(matches!(compiled("max", &[("of", numbers)]), Val::Max(_)));
}

#[test]
fn unknown_descriptors_fail_to_compile() {
    let err = |v: Value| compile(&Descriptor(v)).unwrap_err();
    assert_eq!(
        err(sel("neighbors", &[])),
        DescriptorError::Unknown {
            namespace: "sel",
            name: "neighbors".into()
        }
    );
    assert!(matches!(
        err(cond("sulking", &[])),
        DescriptorError::Unknown {
            namespace: "cond",
            ..
        }
    ));
    assert!(matches!(
        err(val("vibes", &[])),
        DescriptorError::Unknown {
            namespace: "val",
            ..
        }
    ));
    assert!(matches!(
        err(sel("allies", &[("colour", s("red"))])),
        DescriptorError::UnknownField { .. }
    ));
    assert!(matches!(
        err(cond("phase", &[])),
        DescriptorError::Missing { field: "is", .. }
    ));
    assert!(matches!(
        err(sel("allies", &[("of", s("self"))])),
        DescriptorError::Invalid(_)
    ));
    assert!(matches!(err(Value::Nil), DescriptorError::Invalid(_)));
}

#[test]
fn unknown_descriptor_fails_at_load_not_at_runtime() {
    let ability = |when: Value| -> AbilityDef {
        let modifier = serde_json::json!({
            "query": "defense", "layer": "add", "value": { "native": { "Int": 1 } },
            "when": { "native": serde_json::to_value(when).unwrap() },
        });
        serde_json::from_value(serde_json::json!({
            "name": "Test", "text": "Test.", "scope": "figure", "modifiers": [modifier],
        }))
        .unwrap()
    };
    assert!(check_ability(&ability(cond("phase", &[("is", s("turns"))]))).is_ok());
    let err = check_ability(&ability(cond("sulking", &[]))).unwrap_err();
    assert_eq!(err.to_string(), "unknown descriptor `cond.sulking`");
}

#[test]
fn card_text_for_selectors() {
    let adjacent = [("adjacent_to", s("self")), ("trait", s("soldier"))];
    assert_eq!(
        text(sel("allies", &adjacent)),
        "adjacent friendly soldier figures"
    );
    assert_eq!(text(s("self")), "this figure");
    assert_eq!(
        text(sel("enemies", &[("kind", s("hero"))])),
        "enemy hero figures"
    );
    assert_eq!(text(sel("my_units", &[])), "your figures");
    assert_eq!(text(sel("occupant", &[])), "the occupant");
    let within = [("within", n(3)), ("of", sel("allies", &[]))];
    assert_eq!(
        text(sel("enemies", &within)),
        "enemy figures within 3 of friendly figures"
    );
    let near = [("adjacent_to", sel("allies", &[]))];
    assert_eq!(
        text(sel("enemies", &near)),
        "enemy figures adjacent to friendly figures"
    );
}

#[test]
fn card_text_for_conditions() {
    let (t, f) = (Value::Bool(true), Value::Bool(false));
    let pair = list(&[t.clone(), f]);
    assert_eq!(
        text(cond("all", &[("of", pair.clone())])),
        "always and never"
    );
    assert_eq!(
        text(cond("any", &[("of", pair.clone())])),
        "always or never"
    );
    assert_eq!(
        text(cond("none", &[("of", pair)])),
        "none of: always, never"
    );
    assert_eq!(text(cond("not", &[("of", t)])), "not (always)");
    assert_eq!(
        text(cond("phase", &[("is", s("turns"))])),
        "during the turns phase"
    );
    assert_eq!(
        text(cond("lte", &[("a", n(1)), ("b", n(2))])),
        "1 is at most 2"
    );
    let enemies = sel("enemies", &[]);
    let count = [("of", enemies.clone()), ("min", n(2))];
    assert_eq!(text(cond("count", &count)), "at least 2 of enemy figures");
    let capped = [("of", enemies.clone()), ("max", n(3))];
    assert_eq!(
        text(cond("count", &capped)),
        "between 1 and 3 of enemy figures"
    );
    let terrain = [("terrain", s("rock"))];
    assert_eq!(text(cond("on_terrain", &terrain)), "this figure on rock");
    let higher = [("than", enemies.clone())];
    assert_eq!(
        text(cond("higher_than", &higher)),
        "higher than enemy figures"
    );
    let by = [("than", enemies), ("by", n(2))];
    assert_eq!(
        text(cond("higher_than", &by)),
        "2 levels higher than enemy figures"
    );
}

#[test]
fn card_text_for_values() {
    assert_eq!(text(n(2)), "2");
    let stat = val("stat", &[("stat", s("defense"))]);
    assert_eq!(text(stat), "the defense of this figure");
    let enemies = sel("enemies", &[]);
    assert_eq!(
        text(val("count", &[("of", enemies.clone())])),
        "the number of enemy figures"
    );
    let to = [("to", enemies)];
    assert_eq!(text(val("distance", &to)), "the distance to enemy figures");
    assert_eq!(
        text(val("level_difference", &to)),
        "the level difference to enemy figures"
    );
    let of = [("of", list(&[n(1), n(2)]))];
    assert_eq!(text(val("sum", &of)), "the sum of 1, 2");
    assert_eq!(text(val("min", &of)), "the lowest of 1, 2");
    assert_eq!(text(val("max", &of)), "the highest of 1, 2");
}
