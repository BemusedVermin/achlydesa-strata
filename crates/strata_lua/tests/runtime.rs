use strata_kernel::{DefId, PackId, Sym, Value};
use strata_lua::{HostError, Limits, PackSource, RuntimeError, RuntimeLua};
use strata_model::ScriptRef;
use strata_rules::{EvalCtx, ScriptCaller};

const DATA: &str = "
unit {
  id = 'u', name = 'U',
  double = function(_, subject) return subject * 2 end,
  nested = { [1] = function() return 'one' end, deep = { f = function() return 7 end } },
  spin = function() while true do end end,
  halt = function() error(Halt) end,
  half = function() return 1 / 2 end,
  caught = function() pcall(error, Halt) return 'swallowed' end,
}
";

const PATHS: [&str; 7] = [
    "unit:u/double",
    "unit:u/nested/1",
    "unit:u/nested/deep/f",
    "unit:u/spin",
    "unit:u/halt",
    "unit:u/half",
    "unit:u/caught",
];

fn script(path: &str) -> ScriptRef {
    ScriptRef {
        pack: PackId::new("a"),
        def: DefId(0),
        path: Sym::new(path),
    }
}

fn scripts() -> Vec<ScriptRef> {
    PATHS.iter().map(|p| script(p)).collect()
}

/// Writes pack `a` with `data` and keeps the directory alive alongside the pack.
fn pack(data: &str) -> (tempfile::TempDir, Vec<PackSource>) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("data.lua"), data).unwrap();
    let source = PackSource {
        id: PackId::new("a"),
        root: dir.path().to_owned(),
        dependencies: Vec::new(),
    };
    (dir, vec![source])
}

fn runtime() -> (tempfile::TempDir, RuntimeLua) {
    let (dir, packs) = pack(DATA);
    let limits = Limits {
        instructions: 10_000,
        ..Limits::default()
    };
    (
        dir,
        RuntimeLua::with_limits(&scripts(), &packs, limits).unwrap(),
    )
}

#[test]
fn two_states_call_identical_functions_for_every_script_ref() {
    let (_a, first) = runtime();
    let (_b, second) = runtime();
    let args = [Value::Nil, Value::Int(21)];
    for path in PATHS {
        let (x, y) = (
            first.call_script(&script(path), &args),
            second.call_script(&script(path), &args),
        );
        assert_eq!(format!("{x:?}"), format!("{y:?}"), "{path}");
    }
    let double = first.call_script(&script("unit:u/double"), &args).unwrap();
    assert_eq!(double, Value::Int(42));
    let deep = first.call_script(&script("unit:u/nested/deep/f"), &[]);
    assert_eq!(deep.unwrap(), Value::Int(7));
    let one = first.call_script(&script("unit:u/nested/1"), &[]).unwrap();
    assert_eq!(one, Value::Str(Sym::new("one")));
}

#[test]
fn harvest_hash_mismatch_rejects_the_state() {
    let (_dir, packs) = pack(DATA);
    let mut missing = scripts();
    missing.pop();
    let result = RuntimeLua::new(&missing, &packs);
    assert!(matches!(result, Err(RuntimeError::HashMismatch { .. })));

    let mut extra = scripts();
    extra.push(script("unit:u/absent"));
    let result = RuntimeLua::new(&extra, &packs);
    assert!(matches!(result, Err(RuntimeError::HashMismatch { .. })));
}

#[test]
fn registry_order_does_not_change_the_hash() {
    let (_dir, packs) = pack(DATA);
    let mut reversed = scripts();
    reversed.reverse();
    assert!(RuntimeLua::new(&reversed, &packs).is_ok());
}

#[test]
fn runaway_script_hits_the_budget_and_the_state_stays_usable() {
    let (_dir, runtime) = runtime();
    let result = runtime.call_script(&script("unit:u/spin"), &[]);
    assert!(matches!(result, Err(HostError::Budget)));
    let after = runtime.call_script(&script("unit:u/double"), &[Value::Nil, Value::Int(2)]);
    assert_eq!(after.unwrap(), Value::Int(4));
}

#[test]
fn halt_propagates_and_scripts_cannot_swallow_it() {
    let (_dir, runtime) = runtime();
    for path in ["unit:u/halt", "unit:u/caught"] {
        let result = runtime.call_script(&script(path), &[]);
        assert!(matches!(result, Err(HostError::Halt)), "{path}");
    }
}

#[test]
fn float_results_are_rejected() {
    let (_dir, runtime) = runtime();
    let result = runtime.call_script(&script("unit:u/half"), &[]);
    assert!(matches!(result, Err(HostError::Convert(_))));
}

#[test]
fn script_caller_passes_source_subject_and_context() {
    let (_dir, runtime) = runtime();
    let (source, subject, context) = (Sym::new("src"), Value::Int(5), Value::Nil);
    let cx = EvalCtx {
        source: &source,
        subject: &subject,
        context: &context,
    };
    // `double` multiplies its second argument, which is the subject.
    let caller: &dyn ScriptCaller = &runtime;
    assert_eq!(
        caller.call(&script("unit:u/double"), &cx),
        Ok(Value::Int(10))
    );
    let error = caller.call(&script("unit:u/halt"), &cx).unwrap_err();
    assert!(error.0.contains("halted"));
    assert!(caller.call(&script("unit:u/absent"), &cx).is_err());
}
