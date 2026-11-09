use mlua::Table;
use strata_kernel::PackId;
use strata_lua::{DataError, DataStage, HostError, Limits, PackSource};

type Files<'a> = &'a [(&'a str, &'a str)];
type Pack<'a> = (&'a str, &'a [&'a str], Files<'a>);

/// Writes packs `(id, dependencies, files)` under a temp dir and runs the stages in that order.
fn run(packs: &[Pack]) -> Result<DataStage, DataError> {
    let dir = tempfile::tempdir().unwrap();
    let mut sources = Vec::new();
    for (id, dependencies, files) in packs {
        let root = dir.path().join(id);
        for (name, text) in *files {
            let path = root.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        sources.push(PackSource {
            id: PackId::new(id),
            root,
            dependencies: dependencies.iter().map(|d| PackId::new(d)).collect(),
        });
    }
    DataStage::run(&sources)
}

fn message(result: Result<DataStage, DataError>) -> String {
    match result {
        Err(error) => error.to_string(),
        Ok(_) => panic!("expected the data stage to fail"),
    }
}

fn field(stage: &DataStage, kind: &str, id: &str, name: &str) -> String {
    let record: Table = stage.table(kind, id).unwrap();
    record.get(name).unwrap()
}

const DEFINE: &str = "unit { id = 'u', name = 'U', order = 'start' }";

fn append(label: &str) -> String {
    format!("local u = data.unit['a:u']\nu.order = u.order .. ',{label}'")
}

#[test]
fn stages_run_in_order_across_packs() {
    let (b_data, b_updates, b_final) = (append("b:data"), append("b:updates"), append("b:final"));
    let (a_updates, a_final) = (append("a:updates"), append("a:final"));
    let stage = run(&[
        (
            "a",
            &[],
            &[
                ("data.lua", DEFINE),
                ("data_updates.lua", &a_updates),
                ("data_final.lua", &a_final),
            ],
        ),
        (
            "b",
            &["a"],
            &[
                ("data.lua", &b_data),
                ("data_updates.lua", &b_updates),
                ("data_final.lua", &b_final),
            ],
        ),
    ])
    .unwrap();
    assert_eq!(
        field(&stage, "unit", "a:u", "order"),
        "start,b:data,a:updates,b:updates,a:final,b:final"
    );
}

#[test]
fn later_stages_are_optional_but_data_lua_is_not() {
    run(&[("a", &[], &[("data.lua", DEFINE)])]).unwrap();
    let error = message(run(&[("a", &[], &[("data_updates.lua", "")])]));
    assert!(error.contains("pack `a` has no data.lua"), "{error}");
}

#[test]
fn updates_are_visible_to_later_packs_and_to_the_freeze() {
    let base = "unit { id = 'soldier', name = 'S', stats = { points = 10 } }";
    let cheaper =
        "data.unit['base:soldier'].stats.points = data.unit['base:soldier'].stats.points - 5";
    let check = "
        local u = data.unit['base:soldier']
        if u.stats.points ~= 5 then error('saw ' .. u.stats.points) end
        u.checked = true";
    let stage = run(&[
        ("base", &[], &[("data.lua", base)]),
        (
            "balance",
            &["base"],
            &[("data.lua", ""), ("data_updates.lua", cheaper)],
        ),
        (
            "watcher",
            &["balance"],
            &[("data.lua", ""), ("data_updates.lua", check)],
        ),
    ])
    .unwrap();
    let soldier = stage.table("unit", "base:soldier").unwrap();
    let stats: Table = soldier.get("stats").unwrap();
    assert_eq!(stats.get::<i64>("points").unwrap(), 5);
    assert!(soldier.get::<bool>("checked").unwrap());
}

#[test]
fn every_record_knows_its_file_and_line() {
    let data =
        "\n\nterrain { id = 'rock', name = 'Rock' }\nunit {\n  id = 'u',\n  name = 'U',\n}\n";
    let stage = run(&[(
        "a",
        &[],
        &[
            ("data.lua", data),
            ("lib/more.lua", "\nstat { id = 's', name = 'S' }"),
        ],
    )])
    .unwrap();
    let at = |kind: &str| {
        let record = stage
            .records()
            .into_iter()
            .find(|r| r.kind == kind)
            .unwrap();
        (record.id, record.source.file, record.source.line)
    };
    assert_eq!(at("terrain"), ("a:rock".into(), "a/data.lua".into(), 3));
    assert_eq!(at("unit"), ("a:u".into(), "a/data.lua".into(), 4));
}

#[test]
fn records_defined_in_required_modules_point_at_the_module() {
    let stage = run(&[(
        "a",
        &[],
        &[
            ("data.lua", "require('lib.more')"),
            ("lib/more.lua", "\nstat { id = 's', name = 'S' }"),
        ],
    )])
    .unwrap();
    let record = &stage.records()[0];
    assert_eq!(
        (record.source.file.as_str(), record.source.line),
        ("a/lib/more.lua", 2)
    );
}

#[test]
fn constructors_check_required_fields_and_names() {
    let failing = |source: &str| message(run(&[("a", &[], &[("data.lua", source)])]));
    assert!(failing("unit { name = 'U' }").contains("a/data.lua:1: unit needs a string `id`"));
    assert!(failing("\nunit { id = 'u' }").contains("a/data.lua:2: unit needs a string `name`"));
    assert!(failing("unit { id = 'b:u', name = 'U' }").contains("must be a bare name or `a:name`"));
    let twice = failing("unit { id = 'u', name = 'U' }\nunit { id = 'a:u', name = 'V' }");
    assert!(
        twice.contains("a/data.lua:2: unit `a:u` is already defined at a/data.lua:1"),
        "{twice}"
    );
}

#[test]
fn the_same_id_can_exist_in_different_kinds() {
    let stage = run(&[(
        "a",
        &[],
        &[(
            "data.lua",
            "terrain { id = 'x', name = 'X' }\nrelic { id = 'x', name = 'X' }",
        )],
    )])
    .unwrap();
    assert_eq!(stage.records().len(), 2);
}

#[test]
fn bare_ids_prefer_the_current_pack_then_one_dependency() {
    let defs = "terrain { id = 'grass', name = 'G' }\nterrain { id = 'rock', name = 'R' }";
    let only_y = "terrain { id = 'moss', name = 'M' }";
    let own = "terrain { id = 'grass', name = 'Mine' }";
    let check = "
        assert(data.terrain['grass'] == data.terrain['z:grass'], 'own wins')
        assert(data.terrain['rock'] == data.terrain['x:rock'], 'dependency')
        assert(data.terrain['moss'] == data.terrain['y:moss'], 'other dependency')
        assert(data.terrain['nothing'] == nil)";
    run(&[
        ("x", &[], &[("data.lua", defs)]),
        ("y", &[], &[("data.lua", only_y)]),
        (
            "z",
            &["x", "y"],
            &[("data.lua", own), ("data_updates.lua", check)],
        ),
    ])
    .unwrap();
}

#[test]
fn ambiguous_bare_ids_are_an_error() {
    let rock = "terrain { id = 'rock', name = 'R' }";
    let error = message(run(&[
        ("x", &[], &[("data.lua", rock)]),
        ("y", &[], &[("data.lua", rock)]),
        (
            "z",
            &["x", "y"],
            &[("data.lua", "local _ = data.terrain['rock']")],
        ),
    ]));
    assert!(
        error.contains("ambiguous terrain `rock`: defined by x:rock, y:rock"),
        "{error}"
    );
}

#[test]
fn each_iterates_in_id_order_and_filters() {
    let defs = "
        unit { id = 'c', name = 'C', side = 'red' }
        unit { id = 'a', name = 'A', side = 'red' }
        unit { id = 'b', name = 'B', side = 'blue' }
        local all, red = {}, {}
        for id in data.each('unit') do all[#all + 1] = id end
        for id in data.each('unit', { side = 'red' }) do red[#red + 1] = id end
        unit { id = 'log', name = table.concat(all, ',') .. '|' .. table.concat(red, ',') }";
    let stage = run(&[("a", &[], &[("data.lua", defs)])]).unwrap();
    assert_eq!(
        field(&stage, "unit", "a:log", "name"),
        "a:a,a:b,a:c|a:a,a:c"
    );
}

#[test]
fn raw_pairs_over_data_points_at_each() {
    let error = message(run(&[(
        "a",
        &[],
        &[("data.lua", "for _ in pairs(data) do end")],
    )]));
    assert!(error.contains("use data.each(kind, filter)"), "{error}");
    let unknown = message(run(&[("a", &[], &[("data.lua", "data.each('nope')")])]));
    assert!(unknown.contains("unknown kind `nope`"), "{unknown}");
}

#[test]
fn require_loads_caches_and_freezes_modules() {
    let main = "
        local m = require('lib.util')
        assert(m == require('lib.util'), 'cached')
        assert(not pcall(function() m.value = 2 end), 'frozen')
        unit { id = 'u', name = 'U', value = m.value, loads = LOADS }";
    let util = "LOADS = (LOADS or 0) + 1\nreturn { value = 1 }";
    let stage = run(&[(
        "a",
        &[],
        &[
            ("data.lua", &main.replace("LOADS", "1")),
            ("lib/util.lua", util),
        ],
    )])
    .unwrap();
    assert_eq!(
        stage
            .table("unit", "a:u")
            .unwrap()
            .get::<i64>("value")
            .unwrap(),
        1
    );
}

#[test]
fn require_reaches_declared_dependencies_only() {
    let helper = ("lib/shared.lua", "return { n = 7 }");
    let ok = run(&[
        ("lib_pack", &[], &[("data.lua", ""), helper]),
        (
            "a",
            &["lib_pack"],
            &[(
                "data.lua",
                "unit { id = 'u', name = 'U', n = require('lib_pack:lib.shared').n }",
            )],
        ),
    ])
    .unwrap();
    assert_eq!(ok.table("unit", "a:u").unwrap().get::<i64>("n").unwrap(), 7);

    let outsider = message(run(&[
        ("lib_pack", &[], &[("data.lua", ""), helper]),
        ("a", &[], &[("data.lua", "require('lib_pack:lib.shared')")]),
    ]));
    assert!(
        outsider.contains("`a` does not declare `lib_pack` as a dependency"),
        "{outsider}"
    );
}

#[test]
fn require_cannot_escape_the_pack() {
    for name in ["..", "../other", "a/b", "lib..x", ""] {
        let source = format!("require('{name}')");
        let error = message(run(&[("a", &[], &[("data.lua", &source)])]));
        assert!(
            error.contains("dot-separated lowercase words"),
            "{name}: {error}"
        );
    }
    let missing = message(run(&[("a", &[], &[("data.lua", "require('nope')")])]));
    assert!(missing.contains("a/nope.lua"), "{missing}");
    let cycle = run(&[(
        "a",
        &[],
        &[
            ("data.lua", "require('x')"),
            ("x.lua", "require('y')"),
            ("y.lua", "require('x')"),
        ],
    )]);
    assert!(message(cycle).contains("in a cycle"));
}

#[test]
fn data_stage_is_deterministic() {
    let packs: &[Pack] = &[(
        "a",
        &[],
        &[(
            "data.lua",
            "unit { id = 'b', name = 'B' }\nunit { id = 'a', name = 'A' }\nstat { id = 's', name = 'S' }",
        )],
    )];
    assert_eq!(run(packs).unwrap().records(), run(packs).unwrap().records());
}

#[test]
fn require_freezes_nested_tables_and_survives_cycles() {
    let util = "
        local m = { inner = { list = { 1, 2 } }, run = function() return 1 end }
        m.self = m
        m.inner.up = m
        return m";
    let main = "
        local m = require('util')
        assert(not pcall(function() m.inner.x = 1 end), 'inner frozen')
        assert(not pcall(function() m.inner.list[1] = 9 end), 'deep frozen')
        assert(m.self.inner.list[2] == 2 and m.run() == 1)
        unit { id = 'u', name = 'U' }";
    run(&[("a", &[], &[("data.lua", main), ("util.lua", util)])]).unwrap();
}

#[test]
fn run_with_applies_the_instruction_budget_to_each_stage_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("data.lua"), "while true do end").unwrap();
    let pack = PackSource {
        id: PackId::new("a"),
        root: dir.path().to_owned(),
        dependencies: Vec::new(),
    };
    let limits = Limits {
        instructions: 10_000,
        ..Limits::default()
    };
    let error = DataStage::run_with(&[pack], limits).err().unwrap();
    assert!(
        matches!(error, DataError::Host(HostError::Budget)),
        "{error}"
    );
}

#[test]
fn card_text_fills_a_template_from_descriptors() {
    let data = r#"
local to = { sel = 'allies', adjacent_to = 'self', trait = 'soldier' }
ability {
  id = 'shieldwall', name = 'Shieldwall', scope = 'figure',
  text = card_text('{to} add {amount} to their {stat}.', { to = to, amount = 1, stat = 'defense' }),
}
"#;
    let stage = run(&[("t", &[], &[("data.lua", data)])]).unwrap();
    assert_eq!(
        field(&stage, "ability", "t:shieldwall", "text"),
        "Adjacent friendly soldier figures add 1 to their defense."
    );
}

#[test]
fn card_text_raises_for_a_template_it_cannot_fill() {
    let data = "card_text('{missing} wins.', {})";
    let text = message(run(&[("t", &[], &[("data.lua", data)])]));
    assert!(
        text.contains("card_text: the template names `{missing}`"),
        "{text}"
    );
}
