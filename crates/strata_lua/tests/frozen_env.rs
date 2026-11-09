//! Pack environments are frozen once the packs load, so runtime scripts keep no state
//! (design §9.3, §9.4).

use strata_kernel::{DefId, PackId, Sym, Value};
use strata_lua::{DataError, DataStage, HostError, PackSource, RuntimeLua};
use strata_model::ScriptRef;

const DATA: &str = "
local util = require('lib.util')
shared = { count = 0 }
unit {
  id = 'u', name = 'U', stats = { hp = 3 },
  pure = function(_, n)
    local seen, total = {}, 0
    for i = 1, n do seen[i] = i total = total + i end
    return total
  end,
  global = function() calls = (calls or 0) + 1 return calls end,
  shared = function() shared.count = shared.count + 1 return shared.count end,
  module = function() util.list[3] = 3 return #util.list end,
  record = function() data.unit.u.stats.hp = 9 end,
  lookup = function() return data.unit.u.name .. #util.list .. shared.count end,
  define = function() unit { id = 'v', name = 'V' } end,
  late = function() return require('lib.other') end,
  strings = function() getmetatable('').__add = function() return 1 end end,
  raw = function() rawset(_ENV, 'calls', 1) end,
}
";

const PATHS: [&str; 10] = [
    "unit:u/pure",
    "unit:u/global",
    "unit:u/shared",
    "unit:u/module",
    "unit:u/record",
    "unit:u/lookup",
    "unit:u/define",
    "unit:u/late",
    "unit:u/strings",
    "unit:u/raw",
];

fn script(path: &str) -> ScriptRef {
    ScriptRef {
        pack: PackId::new("a"),
        def: DefId(0),
        path: Sym::new(path),
    }
}

/// Writes pack `a` with `data` and its `lib` modules, keeping the directory alive.
fn pack(data: &str) -> (tempfile::TempDir, Vec<PackSource>) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("data.lua"), data).unwrap();
    std::fs::create_dir(dir.path().join("lib")).unwrap();
    std::fs::write(
        dir.path().join("lib/util.lua"),
        "return { list = { 1, 2 } }",
    )
    .unwrap();
    std::fs::write(dir.path().join("lib/other.lua"), "return 1").unwrap();
    let source = PackSource {
        id: PackId::new("a"),
        root: dir.path().to_owned(),
        dependencies: Vec::new(),
    };
    (dir, vec![source])
}

fn runtime() -> (tempfile::TempDir, RuntimeLua) {
    let (dir, packs) = pack(DATA);
    let scripts: Vec<ScriptRef> = PATHS.iter().map(|p| script(p)).collect();
    (dir, RuntimeLua::new(&scripts, &packs).unwrap())
}

fn call(rt: &RuntimeLua, path: &str) -> Result<Value, HostError> {
    rt.call_script(&script(path), &[Value::Nil, Value::Int(4)])
}

fn error(rt: &RuntimeLua, path: &str) -> String {
    match call(rt, path) {
        Err(HostError::Script(message)) => message,
        other => panic!("{path}: expected a script error, got {other:?}"),
    }
}

#[test]
fn setting_a_global_at_runtime_fails_and_names_the_global_and_script() {
    let (_dir, rt) = runtime();
    let message = error(&rt, "unit:u/global");
    assert!(message.contains("cannot set global `calls`"), "{message}");
    assert!(message.contains("a/data.lua:"), "{message}");
    let raw = error(&rt, "unit:u/raw");
    assert!(raw.contains("frozen table"), "{raw}");
}

#[test]
fn writing_into_a_module_level_table_at_runtime_fails() {
    let (_dir, rt) = runtime();
    for path in ["unit:u/shared", "unit:u/module", "unit:u/record"] {
        let message = error(&rt, path);
        assert!(
            message.contains("attempt to modify a frozen table"),
            "{path}: {message}"
        );
        assert!(message.contains("a/data.lua:"), "{path}: {message}");
    }
}

#[test]
fn constructors_require_and_the_string_metatable_are_closed_at_runtime() {
    let (_dir, rt) = runtime();
    let define = error(&rt, "unit:u/define");
    assert!(
        define.contains("only be called while the packs load"),
        "{define}"
    );
    let late = error(&rt, "unit:u/late");
    assert!(
        late.contains("modules load only while the packs load"),
        "{late}"
    );
    assert!(call(&rt, "unit:u/strings").is_err());
}

#[test]
fn locals_and_reads_still_work_after_the_freeze() {
    let (_dir, rt) = runtime();
    assert_eq!(call(&rt, "unit:u/pure").unwrap(), Value::Int(10));
    let read = call(&rt, "unit:u/lookup").unwrap();
    assert_eq!(read, Value::Str(Sym::new("U20")));
}

#[test]
fn rejected_writes_leave_nothing_behind_so_every_call_and_state_agree() {
    let run = || {
        let (_dir, rt) = runtime();
        let mut seen = Vec::new();
        for _ in 0..3 {
            for path in PATHS {
                seen.push(format!("{path}: {:?}", call(&rt, path)));
            }
        }
        seen
    };
    let first = run();
    assert_eq!(first, run());
    let rounds: Vec<&[String]> = first.chunks(PATHS.len()).collect();
    assert_eq!(rounds[0], rounds[1]);
    assert_eq!(rounds[1], rounds[2]);
}

#[test]
fn the_data_stage_freezes_environments_and_records_for_every_reader() {
    let (_dir, packs) = pack(DATA);
    let stage = DataStage::run(&packs).unwrap();
    let env = stage.host().new_env().unwrap();
    let probe = "local ok, e = pcall(function() data.unit.u.name = 'x' end) return e";
    let f = stage.host().load(&env, "a/probe.lua", probe).unwrap();
    let out = stage.host().call(&f, ()).unwrap();
    let message = format!("{:?}", out.into_iter().next());
    assert!(message.contains("frozen table"), "{message}");
    assert!(stage.table("unit", "a:u").is_some());
}

#[test]
fn a_global_table_with_a_metatable_cannot_be_frozen_and_fails_the_load() {
    let (_dir, packs) = pack("lib = setmetatable({}, { __call = function() end })");
    let Err(DataError::Script { stage, message, .. }) = DataStage::run(&packs) else {
        panic!("expected a freeze error");
    };
    assert_eq!(stage, "freeze");
    assert!(message.contains("global `lib`"), "{message}");
    assert!(message.contains("metatable"), "{message}");
}
