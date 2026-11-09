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
fn only_plain_names_are_checked() {
    let source = "local t = {}\nf { h = function() t.x = 1; t[1] = 2; global = 3 end }";
    assert!(upvalues(source).is_empty());
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
