use std::collections::BTreeMap;

use strata_kernel::{DefId, Hex, Rotation, Shape, Space, Sym, Value};
use strata_model::board::{BoardDefs, CompiledBoard, compile as compile_board};
use strata_model::{
    AbilityDef, Descriptor, FeatureDef, FigureId, MapConstraints, MapDef, PlacedPiece, PlayerId,
    SourceLoc, Text, TileDef,
};

use super::*;
use crate::query::{EvalCtx, LogicError, LogicEvaluator};

const GRASS: DefId = DefId(1);
const ROCK: DefId = DefId(2);

struct Tiles(BTreeMap<Sym, TileDef>);

impl BoardDefs for Tiles {
    fn tile(&self, name: &Sym) -> Option<&TileDef> {
        self.0.get(name)
    }
    fn feature(&self, _: &Sym) -> Option<&FeatureDef> {
        None
    }
}

fn tile(terrain: DefId, thickness: u8, cells: &[(i32, i32)]) -> TileDef {
    TileDef {
        id: DefId(0),
        name: Text::default(),
        terrain,
        thickness,
        shape: Shape::new(cells.iter().map(|&(q, r)| Hex::new(q, r))),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    }
}

fn piece(name: &str, level: u8) -> PlacedPiece {
    PlacedPiece {
        piece: name.into(),
        at: Hex::new(0, 0),
        rot: Rotation::from_steps(0),
        level,
    }
}

/// Four grass hexes in a row with a two-level rock hill on the third.
fn board() -> CompiledBoard {
    let row = tile(GRASS, 1, &[(0, 0), (1, 0), (2, 0), (3, 0)]);
    let tiles = Tiles(BTreeMap::from([
        ("row".into(), row),
        ("hill".into(), tile(ROCK, 2, &[(2, 0)])),
    ]));
    let map = MapDef {
        id: DefId(0),
        name: Text::default(),
        authors: Vec::new(),
        orientation: "pointy".into(),
        pieces: vec![piece("row", 0), piece("hill", 1)],
        features: Vec::new(),
        zones: Default::default(),
        relic_spots: Default::default(),
        constraints: MapConstraints::default(),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    compile_board(&map, &tiles).unwrap()
}

struct Fig {
    at: Space,
    player: u8,
    class: &'static str,
    kind: &'static str,
    defense: i64,
}

struct TestWorld {
    board: CompiledBoard,
    figs: Vec<Fig>,
}

/// Figures 0 and 1 are team 0 soldier squads; 2 is a team 1 archer hero on the hill; 3 is a
/// team 1 soldier squad. All stand in one row.
fn world() -> TestWorld {
    let fig = |q, z, player, class, kind, defense| Fig {
        at: Space::new(q, 0, z),
        player,
        class,
        kind,
        defense,
    };
    TestWorld {
        board: board(),
        figs: vec![
            fig(0, 1, 0, "soldier", "squad", 3),
            fig(1, 1, 0, "soldier", "squad", 4),
            fig(2, 3, 1, "archer", "hero", 5),
            fig(3, 1, 1, "soldier", "squad", 6),
        ],
    }
}

impl World for TestWorld {
    fn figures(&self) -> Vec<FigureId> {
        // Reversed on purpose: results must come back sorted whatever order the world uses.
        (0..self.figs.len() as u32).rev().map(FigureId).collect()
    }
    fn location(&self, f: FigureId) -> Option<Space> {
        Some(self.figs[f.index()].at)
    }
    fn controller(&self, f: FigureId) -> PlayerId {
        PlayerId(self.figs[f.index()].player)
    }
    fn team(&self, p: PlayerId) -> u8 {
        p.0
    }
    fn has_trait(&self, f: FigureId, value: &str) -> bool {
        self.figs[f.index()].class == value
    }
    fn kind(&self, f: FigureId) -> Sym {
        Sym::new(self.figs[f.index()].kind)
    }
    fn stat(&self, f: FigureId, stat: &str) -> Result<i64, LogicError> {
        match stat {
            "defense" => Ok(self.figs[f.index()].defense),
            other => Err(LogicError(format!("no stat {other}"))),
        }
    }
    fn terrain_at(&self, space: Space) -> Option<Sym> {
        let surfaces = self.board.surfaces(space.hex());
        let surface = surfaces.iter().find(|s| s.z == space.z)?;
        Some(Sym::new(if surface.terrain == ROCK {
            "rock"
        } else {
            "grass"
        }))
    }
    fn occupant(&self, space: Space) -> Option<FigureId> {
        let index = self.figs.iter().position(|f| f.at == space)?;
        Some(FigureId(index as u32))
    }
    fn phase(&self) -> Sym {
        Sym::new("turns")
    }
    /// Everyone sees everyone except figure 3.
    fn in_los(&self, from: FigureId, to: FigureId) -> Result<bool, LogicError> {
        Ok(from.0 != 3 && to.0 != 3)
    }
}

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

fn ids(ids: &[i64]) -> Value {
    list(&ids.iter().map(|i| n(*i)).collect::<Vec<_>>())
}

fn space(z: i64) -> Value {
    Value::Map(BTreeMap::from([
        (Sym::new("q"), n(2)),
        (Sym::new("r"), n(0)),
        (Sym::new("z"), n(z)),
    ]))
}

fn try_eval(subject: Value, descriptor: Value) -> Result<Value, LogicError> {
    let world = world();
    let evaluator = NativeEvaluator { world: &world };
    let source = Sym::new("test");
    let cx = EvalCtx {
        source: &source,
        subject: &subject,
        context: &Value::Nil,
    };
    evaluator.eval(&Descriptor(descriptor), &cx)
}

fn eval(subject: i64, descriptor: Value) -> Value {
    try_eval(n(subject), descriptor).unwrap()
}

fn yes(subject: i64, descriptor: Value) -> bool {
    eval(subject, descriptor) == Value::Bool(true)
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
fn sel_self_allies_enemies_my_units() {
    assert_eq!(eval(0, s("self")), ids(&[0]));
    assert_eq!(eval(0, sel("allies", &[])), ids(&[1]));
    assert_eq!(eval(0, sel("enemies", &[])), ids(&[2, 3]));
    assert_eq!(eval(1, sel("my_units", &[])), ids(&[0, 1]));
}

#[test]
fn sel_occupant_of_a_figure_and_of_a_space() {
    assert_eq!(eval(2, sel("occupant", &[])), ids(&[2]));
    assert_eq!(try_eval(space(3), sel("occupant", &[])).unwrap(), ids(&[2]));
    assert_eq!(try_eval(space(1), sel("occupant", &[])).unwrap(), ids(&[]));
}

#[test]
fn sel_adjacent_to_filter() {
    let adjacent = [("adjacent_to", s("self"))];
    assert_eq!(eval(0, sel("allies", &adjacent)), ids(&[1]));
    assert_eq!(eval(1, sel("enemies", &adjacent)), ids(&[2]));
    assert_eq!(eval(0, sel("enemies", &adjacent)), ids(&[]));
}

#[test]
fn sel_within_and_of_filters() {
    assert_eq!(eval(0, sel("enemies", &[("within", n(2))])), ids(&[2]));
    assert_eq!(eval(0, sel("enemies", &[("within", n(3))])), ids(&[2, 3]));
    let of_allies = [("within", n(1)), ("of", sel("allies", &[]))];
    assert_eq!(eval(0, sel("enemies", &of_allies)), ids(&[2]));
}

#[test]
fn sel_trait_and_kind_filters() {
    assert_eq!(
        eval(0, sel("enemies", &[("trait", s("soldier"))])),
        ids(&[3])
    );
    assert_eq!(eval(0, sel("enemies", &[("kind", s("hero"))])), ids(&[2]));
    assert_eq!(
        eval(0, sel("my_units", &[("trait", s("archer"))])),
        ids(&[])
    );
}

#[test]
fn cond_literal_all_any_none_not() {
    let (t, f) = (Value::Bool(true), Value::Bool(false));
    let pair = |a: &Value, b: &Value| list(&[a.clone(), b.clone()]);
    assert!(yes(0, t.clone()));
    assert!(yes(0, cond("all", &[("of", pair(&t, &t))])));
    assert!(!yes(0, cond("all", &[("of", pair(&t, &f))])));
    assert!(yes(0, cond("any", &[("of", pair(&f, &t))])));
    assert!(!yes(0, cond("any", &[("of", pair(&f, &f))])));
    assert!(yes(0, cond("none", &[("of", pair(&f, &f))])));
    assert!(!yes(0, cond("none", &[("of", pair(&f, &t))])));
    assert!(yes(0, cond("not", &[("of", f)])));
    assert!(!yes(0, cond("not", &[("of", t)])));
}

#[test]
fn cond_comparisons() {
    let compare = |op: &str, a: i64, b: i64| yes(0, cond(op, &[("a", n(a)), ("b", n(b))]));
    assert!(compare("eq", 2, 2) && !compare("eq", 2, 3));
    assert!(compare("ne", 2, 3) && !compare("ne", 2, 2));
    assert!(compare("lt", 2, 3) && !compare("lt", 3, 3));
    assert!(compare("lte", 3, 3) && !compare("lte", 4, 3));
    assert!(compare("gt", 4, 3) && !compare("gt", 3, 3));
    assert!(compare("gte", 3, 3) && !compare("gte", 2, 3));
}

#[test]
fn cond_count() {
    let enemies = sel("enemies", &[]);
    let count =
        |bound: &str, at: i64| yes(0, cond("count", &[("of", enemies.clone()), (bound, n(at))]));
    assert!(count("min", 2) && !count("min", 3));
    assert!(count("max", 2) && !count("max", 0));
    assert!(yes(0, cond("count", &[("of", enemies)])));
}

#[test]
fn cond_phase() {
    assert!(yes(0, cond("phase", &[("is", s("turns"))])));
    assert!(!yes(0, cond("phase", &[("is", s("setup"))])));
}

#[test]
fn cond_on_terrain() {
    let on = |who: i64, terrain: &str| yes(who, cond("on_terrain", &[("terrain", s(terrain))]));
    assert!(on(2, "rock") && !on(2, "grass"));
    assert!(on(0, "grass") && !on(0, "rock"));
    let heroes = sel("enemies", &[("kind", s("hero"))]);
    assert!(yes(
        0,
        cond("on_terrain", &[("terrain", s("rock")), ("who", heroes)])
    ));
    let everyone = sel("enemies", &[]);
    assert!(!yes(
        0,
        cond("on_terrain", &[("terrain", s("rock")), ("who", everyone)])
    ));
}

#[test]
fn cond_higher_than() {
    let than = |who: i64, by: i64| {
        let fields = [("than", sel("allies", &[])), ("by", n(by))];
        yes(who, cond("higher_than", &fields))
    };
    // Figure 2 stands on the hill at level 3; its only ally, 3, stands at level 1.
    assert!(than(2, 1) && than(2, 2) && !than(2, 3));
    assert!(!than(3, 1));
    let nobody = sel("enemies", &[("kind", s("nobody"))]);
    assert!(!yes(0, cond("higher_than", &[("than", nobody)])));
}

#[test]
fn val_literal_and_stat() {
    assert_eq!(eval(0, n(7)), n(7));
    assert_eq!(eval(1, val("stat", &[("stat", s("defense"))])), n(4));
    let hero = sel("enemies", &[("kind", s("hero"))]);
    assert_eq!(
        eval(0, val("stat", &[("stat", s("defense")), ("of", hero)])),
        n(5)
    );
    assert!(try_eval(n(0), val("stat", &[("stat", s("luck"))])).is_err());
}

#[test]
fn val_count_distance_level_difference() {
    assert_eq!(eval(0, val("count", &[("of", sel("enemies", &[]))])), n(2));
    let hero = sel("enemies", &[("kind", s("hero"))]);
    assert_eq!(eval(0, val("distance", &[("to", hero.clone())])), n(2));
    assert_eq!(eval(0, val("level_difference", &[("to", hero)])), n(-2));
    let squad = sel("enemies", &[("kind", s("squad"))]);
    assert_eq!(eval(2, val("level_difference", &[("to", squad)])), n(2));
}

#[test]
fn val_sum_min_max() {
    let count = val("count", &[("of", sel("enemies", &[]))]);
    let of = list(&[n(4), count, n(5)]);
    assert_eq!(eval(0, val("sum", &[("of", of.clone())])), n(11));
    assert_eq!(eval(0, val("min", &[("of", of.clone())])), n(2));
    assert_eq!(eval(0, val("max", &[("of", of)])), n(5));
}

#[test]
fn empty_selection_is_an_empty_list_and_a_value_error() {
    let nobody = sel("enemies", &[("kind", s("nobody"))]);
    assert_eq!(eval(0, nobody.clone()), ids(&[]));
    assert!(try_eval(n(0), val("distance", &[("to", nobody)])).is_err());
}

#[test]
fn figure_selectors_need_a_figure_subject() {
    assert!(try_eval(space(1), sel("allies", &[])).is_err());
    assert!(try_eval(Value::Nil, s("self")).is_err());
}

#[test]
fn evaluation_is_repeatable() {
    let d = sel("enemies", &[("within", n(3))]);
    assert_eq!(eval(0, d.clone()), eval(0, d));
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

#[test]
fn sel_in_los_filter_asks_the_world() {
    assert_eq!(eval(0, sel("enemies", &[])), ids(&[2, 3]));
    let seen = [("in_los", s("self"))];
    assert_eq!(eval(0, sel("enemies", &seen)), ids(&[2]));
    // Ally 1 is the only seer, and it cannot see figure 3.
    let by_allies = [("in_los", sel("allies", &[]))];
    assert_eq!(eval(0, sel("enemies", &by_allies)), ids(&[2]));
    // A figure is never its own seer.
    assert_eq!(eval(0, sel("my_units", &seen)), ids(&[1]));
}

#[test]
fn sel_in_los_has_a_phrase_and_rejects_bad_selectors() {
    let seen = sel("enemies", &[("in_los", s("self"))]);
    assert_eq!(text(seen), "enemy figures in line of sight");
    let of_allies = sel("enemies", &[("in_los", sel("allies", &[]))]);
    assert_eq!(
        text(of_allies),
        "enemy figures in line of sight of friendly figures"
    );
    let bad = sel("enemies", &[("in_los", s("nobody"))]);
    assert!(compile(&Descriptor(bad)).is_err());
}
