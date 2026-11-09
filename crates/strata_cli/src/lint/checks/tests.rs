use super::*;

/// `line: message` for every finding.
fn found(source: &str, logic: bool) -> Vec<String> {
    check(source, logic)
        .into_iter()
        .map(|d| format!("{}: {}", d.line, d.message))
        .collect()
}

fn upvalues(source: &str) -> Vec<String> {
    found(source, false)
}

#[test]
fn hooks_may_not_assign_upvalues() {
    let source =
        "local count = 0\nability { after = { hit = function(ctx) count = count + 1 end } }";
    assert_eq!(
        upvalues(source),
        vec!["2: hook assigns to the upvalue `count`"]
    );
}

#[test]
fn hook_parameters_and_locals_are_not_upvalues() {
    let source = "local a, b = 0, 0\nability { after = { hit = function(a)\n  local b\n  a = 1\n  b = 2\nend } }";
    assert!(upvalues(source).is_empty());
}

#[test]
fn nested_functions_inside_a_hook_see_its_locals_and_its_upvalues() {
    let source = "local n = 0\nf { h = function()\n  local mine = 0\n  local g = function() mine = 1\n n = 2 end\nend }";
    assert_eq!(upvalues(source), vec!["5: hook assigns to the upvalue `n`"]);
}

#[test]
fn hooks_may_not_assign_globals() {
    let source = "f { h = function()\n  calls = (calls or 0) + 1\n  local ok = 1\n  ok = 2\nend }";
    assert_eq!(
        upvalues(source),
        vec!["2: hook assigns to the global `calls`"]
    );
    let declared = "f { h = function()\n  function helper() end\nend }";
    assert_eq!(
        upvalues(declared),
        vec!["2: hook assigns to the global `helper`"]
    );
    // Outside hooks, the data stage may still set globals.
    assert!(upvalues("shared = {}\nlocal function g() other = 1 end").is_empty());
}

#[test]
fn hooks_may_not_write_into_captured_or_global_tables() {
    let source = "local cache = {}\nf { h = function(ctx, k)\n  cache[k] = true\n  cache.n.m = 1\n  seen.last = k\nend }";
    assert_eq!(
        upvalues(source),
        vec![
            "3: hook writes into the upvalue table `cache`",
            "4: hook writes into the upvalue table `cache`",
            "5: hook writes into the global table `seen`",
        ]
    );
}

#[test]
fn hooks_may_write_into_their_own_tables_and_call_results() {
    let source = "f { h = function(ctx, self)\n  local out = {}\n  out[1] = 2\n  self.x = 1\n  ctx.vars(self).n = 3\n  local function m() end\nend }";
    assert!(upvalues(source).is_empty(), "{:?}", upvalues(source));
}

#[test]
fn functions_registered_inside_register_are_hooks_of_their_own() {
    // Issue #242: `register` runs once, so its locals outlive every call of `targets`.
    let source = "rule_module { id = 'm', register = function(r)
  local seen = {}
  r.action { id = 'a', targets = function(ctx, f)
    local mine = {}
    mine[1] = f
    seen[#seen + 1] = f
    return { #seen }
  end }
end }";
    assert_eq!(
        upvalues(source),
        vec!["6: hook writes into the upvalue table `seen`"]
    );
}

#[test]
fn library_calls_that_write_their_first_argument_are_checked() {
    let source = "local cache = {}
f { h = function(ctx, x)
  local out = {}
  table.insert(out, x)
  table.insert(cache, x)
  table.remove(cache.list)
  table.sort(seen)
  rawset(cache, x, true)
  table.concat(cache)
end }";
    assert_eq!(
        upvalues(source),
        vec![
            "5: hook writes into the upvalue table `cache`",
            "6: hook writes into the upvalue table `cache`",
            "7: hook writes into the global table `seen`",
            "8: hook writes into the upvalue table `cache`",
        ]
    );
}

#[test]
fn writes_through_a_local_alias_are_checked() {
    let source = "local cache = {}
f { h = function(ctx, k)
  local c, n = cache, 1
  c[k] = true
  local d = c.list
  table.insert(d, k)
  local g = Shared
  g.x = 1
  c = {}
  n = 2
end }";
    assert_eq!(
        upvalues(source),
        vec![
            "4: hook writes into the upvalue table `cache` through the local `c`",
            "6: hook writes into the upvalue table `cache` through the local `d`",
            "8: hook writes into the global table `Shared` through the local `g`",
        ]
    );
}

#[test]
fn functions_a_definition_table_names_are_hooks() {
    let source = "local cache = {}
local function targets(ctx)
  cache[1] = true
end
local filter = function() cache[2] = true end
local function helper() cache[3] = true end
ability { actions = {{ targets = targets, filter = filter }} }";
    assert_eq!(
        upvalues(source),
        vec![
            "3: hook writes into the upvalue table `cache`",
            "5: hook writes into the upvalue table `cache`",
        ]
    );
}

#[test]
fn only_functions_inside_call_tables_are_hooks() {
    assert!(upvalues("local n = 0\nf(function() n = 1 end)").is_empty());
    assert!(upvalues("local n = 0\nlocal t = { h = function() n = 1 end }").is_empty());
    assert_eq!(
        upvalues("local n = 0\nf({ function() n = 1 end })"),
        vec!["2: hook assigns to the upvalue `n`"]
    );
}

#[test]
fn local_function_recursion() {
    // Outside a hook, a helper may update its own state.
    let helper = "local state = 0\nlocal function bump() state = state + 1; bump = nil end";
    assert!(upvalues(helper).is_empty());
    // Inside a hook, a local function's name is the hook's own local.
    let inside = "f { h = function()\n  local function again() again = nil end\nend }";
    assert!(upvalues(inside).is_empty());
    let outside = "local function again() end\nf { h = function() again = nil end }";
    assert_eq!(
        upvalues(outside),
        vec!["2: hook assigns to the upvalue `again`"]
    );
}

#[test]
fn for_loop_variables_are_scoped_to_the_loop() {
    let inside = "f { h = function()\n  for i = 1, 3 do i = 2 end\n  for k, v in pairs(t) do k = v end\nend }";
    assert!(upvalues(inside).is_empty());
    let outside = "for i = 1, 3 do\n  f { h = function() i = 0 end }\nend";
    assert_eq!(
        upvalues(outside),
        vec!["2: hook assigns to the upvalue `i`"]
    );
    let after = "local i = 0\nf { h = function()\n  for i = 1, 2 do end\n  i = 5\nend }";
    assert_eq!(upvalues(after), vec!["4: hook assigns to the upvalue `i`"]);
}

#[test]
fn method_calls_define_hooks_too() {
    let source = "local n = 0\nobj.defs:add { h = function() n = 1 end }\nobj:on 'x'";
    assert_eq!(upvalues(source), vec!["2: hook assigns to the upvalue `n`"]);
}

#[test]
fn fields_of_captured_tables_and_globals_are_checked_too() {
    let source = "local t = {}\nf { h = function() t.x = 1; t[1] = 2; global = 3 end }";
    assert_eq!(
        upvalues(source),
        vec![
            "2: hook writes into the upvalue table `t`",
            "2: hook writes into the upvalue table `t`",
            "2: hook assigns to the global `global`",
        ]
    );
}

#[test]
fn division_is_an_error_only_in_logic_files() {
    let source = "local half = n / 2\nlocal whole = n // 2\nlocal s = '1/2' -- 3/4\n";
    assert_eq!(found(source, true), vec!["1: `/` always produces a float"]);
    assert!(found(source, false).is_empty());
}

#[test]
fn card_text_must_mention_every_parameter() {
    let source = "aura { amount = 2, range = 3,\n  text = \"add 2\" }";
    assert_eq!(
        found(source, false),
        vec!["2: card text omits the parameter 3"]
    );
}

#[test]
fn card_text_ignores_hooks_names_and_non_literal_text() {
    let ok = [
        "aura { amount = 2, h = function() return 9 end, text = 'add 2' }",
        "aura { amount = 2, name = 5, id = 6, meta = 7, tags = 8, vars = { n = 9 }, text = 'add 2' }",
        "aura { amount = 2, text = 'add ' .. x }",
        "aura { amount = 2 }",
        "local t = { amount = 2, text = 'x' }",
        "aura { amount = 7, text = [==[within 7]==] }",
    ];
    for source in ok {
        assert!(found(source, false).is_empty(), "{source}");
    }
}

#[test]
fn findings_on_one_line_keep_source_order() {
    let source = "local n = 0\nf { h = function() n = n / 2 end, text = 'x', a = 1 }";
    assert_eq!(
        found(source, true),
        vec![
            "2: card text omits the parameter 1",
            "2: hook assigns to the upvalue `n`",
            "2: `/` always produces a float",
        ]
    );
}

#[test]
fn a_syntax_error_is_the_only_finding() {
    let source = "local n = 0\nf { h = function() n = 1 / 2 end\n";
    let all = check(source, true);
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].line, 3);
    assert_eq!(
        all[0].message,
        "syntax error: expected `}` after last field"
    );
    assert_eq!(all[0].severity, Severity::Error);
}

#[test]
fn checks_are_deterministic() {
    let source = "local n = 0\nf { h = function() n = n / 2 end, text = 'x', a = 1, b = 2 }";
    assert_eq!(check(source, true), check(source, true));
}
