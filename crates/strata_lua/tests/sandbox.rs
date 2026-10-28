use mlua::{Function, MultiValue, Table, Value as LuaValue};
use strata_kernel::{Sym, Value};
use strata_lua::{ConvertError, HostError, Limits, LuaHost, lua_to_value, value_to_lua};

fn host() -> LuaHost {
    LuaHost::new(Limits::default()).unwrap()
}

/// Runs `source` in a fresh pack environment and returns its results.
fn run_in(host: &LuaHost, source: &str) -> Result<MultiValue, HostError> {
    let env = host.new_env().unwrap();
    let f = host.load(&env, "test.lua", source).unwrap();
    host.call(&f, ())
}

fn run(source: &str) -> Result<MultiValue, HostError> {
    run_in(&host(), source)
}

fn run_str(source: &str) -> String {
    let host = host();
    let out = run_in(&host, source).unwrap();
    match out.into_iter().next() {
        Some(LuaValue::String(s)) => s.to_string_lossy(),
        other => panic!("expected a string, got {other:?}"),
    }
}

// ---- removed libraries and functions ----

#[test]
fn removed_libraries_are_absent() {
    for lib in ["io", "os", "debug", "package", "coroutine", "require"] {
        assert_eq!(run_str(&format!("return type({lib})")), "nil", "{lib}");
    }
}

#[test]
fn removed_functions_are_absent() {
    for name in [
        "dofile",
        "loadfile",
        "load",
        "collectgarbage",
        "next",
        "math.random",
        "math.randomseed",
    ] {
        assert_eq!(run_str(&format!("return type({name})")), "nil", "{name}");
    }
}

#[test]
fn kept_libraries_work() {
    assert_eq!(run_str("return ('ab'):upper() .. #table.pack(1, 2)"), "AB2");
    assert_eq!(
        run_str("return tostring(math.max(1, 2) + utf8.len('é'))"),
        "3"
    );
}

// ---- sorted pairs ----

#[test]
fn pairs_orders_numbers_before_strings_ascending() {
    let order = run_str(
        "local t = { z = 1, b = 2, [10] = 3, a = 4, [2] = 5, [-1] = 6 }
         local out = {}
         for k in pairs(t) do out[#out + 1] = tostring(k) end
         return table.concat(out, ',')",
    );
    assert_eq!(order, "-1,2,10,a,b,z");
}

#[test]
fn pairs_is_independent_of_insertion_order() {
    let keys: Vec<String> = (0..200).map(|i| format!("key{}", i * 7919 % 211)).collect();
    let build = |reverse: bool| {
        let host = host();
        let mut ordered = keys.clone();
        if reverse {
            ordered.reverse();
        }
        let inserts: String = ordered.iter().map(|k| format!("t.{k} = 1\n")).collect();
        run_in(
            &host,
            &format!(
                "local t = {{}}\n{inserts}
                 local out = {{}}
                 for k in pairs(t) do out[#out + 1] = k end
                 return table.concat(out, ',')"
            ),
        )
        .unwrap()
        .into_iter()
        .next()
        .map(|v| v.to_string().unwrap())
    };
    assert_eq!(build(false), build(true));
}

#[test]
fn pairs_rejects_unorderable_keys() {
    let err = run("local t = { [true] = 1 } for _ in pairs(t) do end").unwrap_err();
    assert!(matches!(err, HostError::Script(m) if m.contains("cannot order")));
}

// ---- budget ----

fn loop_count(limits: Limits) -> (i64, HostError) {
    let host = LuaHost::new(limits).unwrap();
    let env = host.new_env().unwrap();
    env.set("n", 0).unwrap();
    let f = host
        .load(&env, "loop.lua", "while true do n = n + 1 end")
        .unwrap();
    let err = host.call(&f, ()).unwrap_err();
    (env.get("n").unwrap(), err)
}

#[test]
fn infinite_loop_aborts_at_the_same_count_every_run() {
    let limits = Limits {
        instructions: 10_000,
        ..Limits::default()
    };
    let (first, err) = loop_count(limits);
    assert!(matches!(err, HostError::Budget));
    assert!(first > 0);
    for _ in 0..3 {
        assert_eq!(loop_count(limits).0, first);
    }
}

#[test]
fn budget_resets_for_each_call() {
    let host = LuaHost::new(Limits {
        instructions: 10_000,
        ..Limits::default()
    })
    .unwrap();
    let env = host.new_env().unwrap();
    let f = host
        .load(&env, "t.lua", "for i = 1, 500 do end return 1")
        .unwrap();
    for _ in 0..50 {
        host.call(&f, ()).unwrap();
    }
}

#[test]
fn pcall_cannot_swallow_the_budget() {
    let limits = Limits {
        instructions: 10_000,
        ..Limits::default()
    };
    let host = LuaHost::new(limits).unwrap();
    let err = run_in(
        &host,
        "while true do pcall(function() while true do end end) end",
    )
    .unwrap_err();
    assert!(matches!(err, HostError::Budget));
}

// ---- memory ----

#[test]
fn memory_limit_stops_runaway_allocation() {
    let host = LuaHost::new(Limits {
        memory: 4 * 1024 * 1024,
        ..Limits::default()
    })
    .unwrap();
    let err = run_in(&host, "local s = 'x' while true do s = s .. s end").unwrap_err();
    assert!(
        matches!(&err, HostError::Script(m) if m.contains("memory")),
        "{err:?}"
    );
}

// ---- Halt ----

#[test]
fn halt_surfaces_as_halt() {
    assert!(matches!(run("error(Halt)").unwrap_err(), HostError::Halt));
}

#[test]
fn pcall_rethrows_halt() {
    let err = run("local ok = pcall(error, Halt) return 'swallowed'").unwrap_err();
    assert!(matches!(err, HostError::Halt));
    let err = run("pcall(function() error(Halt) end) return 'swallowed'").unwrap_err();
    assert!(matches!(err, HostError::Halt));
}

#[test]
fn xpcall_rethrows_halt_without_running_the_handler() {
    let err = run(
        "xpcall(function() error(Halt) end, function(e) return 'handled' end)
         return 'swallowed'",
    )
    .unwrap_err();
    assert!(matches!(err, HostError::Halt));
}

#[test]
fn pcall_still_catches_ordinary_errors() {
    assert_eq!(
        run_str("local ok, e = pcall(error, 'boom', 0) return tostring(ok) .. e"),
        "falseboom"
    );
    assert_eq!(
        run_str("local ok, e = xpcall(error, function(m) return 'h:' .. m end, 'x', 0) return e"),
        "h:x"
    );
}

// ---- freeze and environments ----

#[test]
fn frozen_tables_reject_writes_but_read_normally() {
    assert_eq!(
        run_str(
            "local t = freeze { 10, 20, a = 'x' }
             local ok, e = pcall(function() t.a = 1 end)
             local ok2 = pcall(function() t[3] = 1 end)
             local ok3 = pcall(rawset, t, 'a', 1)
             local ok4 = pcall(table.insert, t, 5)
             return table.concat({ tostring(ok), tostring(ok2), tostring(ok3), tostring(ok4),
                 #t, t[2], t.a, rawget(t, 'a'), rawlen(t) }, ',')"
        ),
        "false,false,false,false,2,20,x,x,2"
    );
}

#[test]
fn frozen_tables_still_iterate_in_order() {
    assert_eq!(
        run_str(
            "local t = freeze { b = 1, a = 2, [1] = 3 }
             local out = {}
             for k in pairs(t) do out[#out + 1] = tostring(k) end
             return table.concat(out, ',')"
        ),
        "1,a,b"
    );
}

#[test]
fn standard_libraries_are_frozen() {
    for lib in ["string", "table", "math", "utf8"] {
        let src = format!("{lib}.extra = 1");
        assert!(run(&src).is_err(), "{lib}");
    }
    assert!(run("getmetatable('').__index.extra = 1").is_err());
}

#[test]
fn environments_are_isolated_and_freezable() {
    let host = host();
    let a = host.new_env().unwrap();
    let b = host.new_env().unwrap();
    let set = host.load(&a, "a.lua", "shared = 1").unwrap();
    host.call(&set, ()).unwrap();
    let get = host.load(&b, "b.lua", "return shared").unwrap();
    let out = host.call(&get, ()).unwrap();
    assert!(matches!(out.into_iter().next(), None | Some(LuaValue::Nil)));

    host.freeze(&a).unwrap();
    let write = host.load(&a, "a2.lua", "other = 2").unwrap();
    assert!(matches!(host.call(&write, ()), Err(HostError::Script(_))));
    let read = host.load(&a, "a3.lua", "return shared").unwrap();
    let out = host.call(&read, ()).unwrap();
    assert!(matches!(out.into_iter().next(), Some(LuaValue::Integer(1))));
}

#[test]
fn script_errors_report_the_chunk_name() {
    let err = run("error('bad')").unwrap_err();
    assert!(matches!(err, HostError::Script(m) if m.contains("test.lua")));
}

// ---- conversion ----

fn eval(host: &LuaHost, source: &str) -> Result<Value, ConvertError> {
    let env = host.new_env().unwrap();
    let f = host.load(&env, "v.lua", source).unwrap();
    let out = host.call(&f, ()).unwrap();
    lua_to_value(host.lua(), &out.into_iter().next().unwrap_or(LuaValue::Nil))
}

#[test]
fn converts_plain_data() {
    let host = host();
    let value = eval(
        &host,
        "return { name = 'x', hp = 3, tags = { 'a', 'b' }, ok = true }",
    )
    .unwrap();
    assert_eq!(value.get("name").unwrap(), &Value::from("x"));
    assert_eq!(value.get("hp").unwrap(), &Value::Int(3));
    assert_eq!(value.get("ok").unwrap(), &Value::Bool(true));
    assert_eq!(
        value.get("tags").unwrap(),
        &Value::List(vec![Value::from("a"), Value::from("b")])
    );
    assert_eq!(eval(&host, "return {}").unwrap(), Value::List(vec![]));
    assert_eq!(eval(&host, "return nil").unwrap(), Value::Nil);
}

#[test]
fn converts_frozen_tables() {
    let host = host();
    let value = eval(&host, "return freeze { a = 1 }").unwrap();
    assert_eq!(value.get("a").unwrap(), &Value::Int(1));
}

#[test]
fn rejects_floats_and_nan() {
    let host = host();
    for src in [
        "return 1.5",
        "return 2.0",
        "return 1 / 2",
        "return 0/0",
        "return { x = 0.5 }",
    ] {
        assert_eq!(eval(&host, src), Err(ConvertError::Float), "{src}");
    }
    assert_eq!(eval(&host, "return 4 // 2").unwrap(), Value::Int(2));
}

#[test]
fn rejects_functions_and_other_types() {
    let host = host();
    assert_eq!(eval(&host, "return print"), Err(ConvertError::Function));
    assert_eq!(
        eval(&host, "return { f = function() end }"),
        Err(ConvertError::Function)
    );
    assert_eq!(
        eval(&host, "return Halt"),
        Err(ConvertError::Unsupported("userdata"))
    );
}

#[test]
fn rejects_ambiguous_tables() {
    let host = host();
    for src in [
        "return { 1, 2, x = 3 }",
        "return { [1] = 1, [3] = 3 }",
        "return { [true] = 1 }",
        "return { [0] = 1 }",
    ] {
        assert_eq!(eval(&host, src), Err(ConvertError::BadTable), "{src}");
    }
}

#[test]
fn rejects_cycles() {
    let host = host();
    assert_eq!(
        eval(&host, "local t = {} t.self = t return t"),
        Err(ConvertError::TooDeep)
    );
}

#[test]
fn value_round_trips() {
    let host = host();
    let value = Value::Map(
        [
            (
                Sym::new("list"),
                Value::List(vec![Value::Int(1), Value::Bool(false)]),
            ),
            (Sym::new("s"), Value::from("hi")),
        ]
        .into_iter()
        .collect(),
    );
    let lua_value = value_to_lua(host.lua(), &value).unwrap();
    assert_eq!(lua_to_value(host.lua(), &lua_value).unwrap(), value);
    assert!(matches!(lua_value, LuaValue::Table(_)));
    assert_eq!(
        value_to_lua(host.lua(), &Value::Id(strata_kernel::DefId(1))).unwrap_err(),
        ConvertError::Unsupported("id")
    );
}

#[test]
fn host_lua_accessor_exposes_the_state() {
    let host = host();
    let t: Table = host.lua().create_table().unwrap();
    let f: Function = host.lua().create_function(|_, ()| Ok(1)).unwrap();
    assert!(t.is_empty() && f.call::<i64>(()).unwrap() == 1);
}
