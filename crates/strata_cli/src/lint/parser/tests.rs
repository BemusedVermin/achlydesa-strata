use super::*;
use crate::lint::lexer::tokenize;

/// Records every event as a short line of text.
#[derive(Default)]
struct Recorder(Vec<String>);

impl Events for Recorder {
    fn open_scope(&mut self, names: Vec<String>) {
        self.0.push(format!("open {names:?}"));
    }
    fn close_scope(&mut self) {
        self.0.push("close".into());
    }
    fn declare(&mut self, name: String) {
        self.0.push(format!("declare {name}"));
    }
    fn function_start(&mut self) {
        self.0.push("function".into());
    }
    fn function_end(&mut self) {
        self.0.push("function end".into());
    }
    fn args_start(&mut self) {
        self.0.push("args".into());
    }
    fn args_end(&mut self) {
        self.0.push("args end".into());
    }
    fn table_start(&mut self) {
        self.0.push("table".into());
    }
    fn table_end(&mut self) {
        self.0.push("table end".into());
    }
    fn call_table(&mut self, at: usize, fields: Vec<Field>) {
        self.0.push(format!("call table @{at} {fields:?}"));
    }
    fn assignment(&mut self, at: usize, targets: Vec<Target>) {
        let shown: Vec<String> = targets
            .iter()
            .map(|t| {
                let field = if t.field { ".." } else { "" };
                format!("{}{field}:{}", t.name, t.line)
            })
            .collect();
        self.0.push(format!("assign @{at} {shown:?}"));
    }
    fn division(&mut self, at: usize, line: usize) {
        self.0.push(format!("divide @{at} line {line}"));
    }
}

fn events(source: &str) -> Vec<String> {
    let mut recorder = Recorder::default();
    parse(&tokenize(source), &mut recorder).unwrap();
    recorder.0
}

/// The events without the chunk's own scope.
fn inner(source: &str) -> Vec<String> {
    let mut all = events(source);
    assert_eq!(all.remove(0), "open []");
    assert_eq!(all.pop().unwrap(), "close");
    all
}

#[test]
fn nested_functions_open_parameter_then_block_scopes() {
    assert_eq!(
        inner("x = function(a, ...) return function(b) end end"),
        vec![
            "assign @0 [\"x:1\"]",
            "function",
            "open [\"a\"]",
            "open []",
            "function",
            "open [\"b\"]",
            "open []",
            "close",
            "close",
            "function end",
            "close",
            "close",
            "function end",
        ]
    );
}

#[test]
fn local_function_is_declared_before_its_body() {
    assert_eq!(
        inner("local function f(n) return f(n - 1) end"),
        vec![
            "declare f",
            "open [\"n\"]",
            "open []",
            "args",
            "args end",
            "close",
            "close",
        ]
    );
}

#[test]
fn locals_are_declared_before_their_values() {
    assert_eq!(
        inner("local a <const>, b = function() end"),
        vec![
            "declare a",
            "declare b",
            "function",
            "open []",
            "open []",
            "close",
            "close",
            "function end"
        ]
    );
}

#[test]
fn for_loop_variables_scope_their_expressions_and_body() {
    assert_eq!(
        inner("for i = 1, n do end"),
        vec!["open [\"i\"]", "open []", "close", "close"]
    );
    assert_eq!(
        inner("for k, v in pairs(t) do local x end"),
        vec![
            "open [\"k\", \"v\"]",
            "args",
            "args end",
            "open []",
            "declare x",
            "close",
            "close",
        ]
    );
}

#[test]
fn blocks_of_every_statement_open_scopes() {
    let opens = |source: &str| inner(source).iter().filter(|e| *e == "open []").count();
    assert_eq!(opens("do end"), 1);
    assert_eq!(opens("while x do end"), 1);
    assert_eq!(opens("repeat until x"), 1);
    assert_eq!(opens("if a then elseif b then else end"), 3);
    assert_eq!(opens("function m.a.b:c(x) end"), 1);
}

#[test]
fn method_calls_with_table_and_string_arguments() {
    let fields = "[Field { key: \"a\", line: 1, string: None, numbers: [\"1\"] }]";
    assert_eq!(
        inner("obj.list:add { a = 1 }"),
        vec![
            "args".to_owned(),
            "table".into(),
            "table end".into(),
            format!("call table @13 {fields}"),
            "args end".into(),
        ]
    );
    assert_eq!(
        inner("a:b 's' :c()"),
        vec!["args", "args end", "args", "args end"]
    );
}

#[test]
fn method_declarations_take_only_their_explicit_parameters() {
    assert_eq!(
        inner("function obj:m(x) end"),
        vec![
            "assign @0 [\"obj..:1\"]",
            "open [\"x\"]",
            "open []",
            "close",
            "close"
        ]
    );
}

#[test]
fn assignments_report_targets_rooted_at_a_name() {
    assert_eq!(
        inner("a, b.c, (d), e[1], f = 1"),
        vec!["assign @0 [\"a:1\", \"b..:1\", \"e..:1\", \"f:1\"]"]
    );
    assert_eq!(inner("x.y[1].z = 1"), vec!["assign @0 [\"x..:1\"]"]);
    let calls = inner("f().y, (g).z, h:m().k = 1, 2, 3");
    assert_eq!(calls.last().unwrap(), "assign @0 []");
}

#[test]
fn function_statements_assign_their_name() {
    let plain = inner("function f() end");
    assert_eq!(plain[0], "assign @0 [\"f:1\"]");
    let method = inner("function m.a:b() end");
    assert_eq!(method[0], "assign @0 [\"m..:1\"]");
    let local = inner("local function g() end");
    assert!(local.iter().all(|e| !e.starts_with("assign")), "{local:?}");
}

#[test]
fn division_is_keyed_by_its_left_operand() {
    assert_eq!(
        inner("x = a + b / c // d"),
        vec!["assign @0 [\"x:1\"]", "divide @8 line 1"]
    );
    assert_eq!(
        inner("x = -a / b /\n c"),
        vec![
            "assign @0 [\"x:1\"]",
            "divide @4 line 1",
            "divide @4 line 1"
        ]
    );
}

#[test]
fn call_tables_collect_strings_and_numbers_outside_functions() {
    let found = inner(
        "f { text = [[x]], a = -3, b = { 4, 0x5, 6.5 }, c = function() return 7 end, d = ('s'), [8] = 9 }",
    );
    let expected = "[Field { key: \"text\", line: 1, string: Some(\"x\"), numbers: [] }, \
        Field { key: \"a\", line: 1, string: None, numbers: [\"3\"] }, \
        Field { key: \"b\", line: 1, string: None, numbers: [\"4\"] }, \
        Field { key: \"c\", line: 1, string: None, numbers: [] }, \
        Field { key: \"d\", line: 1, string: None, numbers: [] }]";
    assert!(
        found.contains(&format!("call table @2 {expected}")),
        "{found:#?}"
    );
}

#[test]
fn numbers_inside_a_hook_belong_to_its_own_calls() {
    let found = inner("f { h = function() g { n = 2 } end }");
    assert!(
        found
            .iter()
            .any(|e| e.contains("key: \"n\", line: 1, string: None, numbers: [\"2\"]"))
    );
    assert!(
        found
            .iter()
            .any(|e| e.contains("key: \"h\", line: 1, string: None, numbers: []"))
    );
}

#[test]
fn valid_lua_parses() {
    let sources = [
        "goto done ::done::",
        "while x do break end repeat x = x - 1 until x == 0",
        "local a = 1 // 2 + 3 % 4 ^ 5 .. 'x' .. 'y' and b or c",
        "local z = a & b | c ~ d << 1 >> 2 local w = ~a + -b + #c + not d",
        "f(...) local function g(a, ...) return ... end",
        "x = 0x1p4 + 0xA.8p-1 + .5 + 3. + 1e+10; y = {} return 1, 2;",
        "f{}{}'s'[[t]]:m(1)",
        "x = { [1] = 2; 3, a = 4, }",
    ];
    for source in sources {
        assert!(
            parse(&tokenize(source), &mut Recorder::default()).is_ok(),
            "{source}"
        );
    }
}

/// Lines and messages `full_moon` 3.0.0 reported for these scripts, captured before
/// it was replaced.
const FULL_MOON_ERRORS: &[(&str, usize, &str)] = &[
    ("unit { id = \"broken\",\n", 1, "expected a field"),
    (
        "unit { id = \"broken\"\n",
        2,
        "expected `}` after last field",
    ),
    ("unit {\n", 1, "expected a field"),
    ("x = { a = { b = 1,\n c = 2 },\n", 2, "expected a field"),
    ("x = \"abc\n", 1, "unclosed string (1:5 to 2:1)"),
    (
        "--[[ never closed\nx = 1\n",
        1,
        "unclosed comment (1:1 to 3:1)",
    ),
    ("x = 1 $\n", 1, "unexpected character $ (1:7 to 1:8)"),
    ("x = 1.2.3\n", 1, "invalid number (1:5 to 1:10)"),
    (
        "local = 1\n",
        1,
        "expected either a variable name or `function`",
    ),
    ("local function () end\n", 1, "expected a function name"),
    ("local function f end\n", 1, "expected a function body"),
    ("function () end\n", 1, "expected function name"),
    ("function a.() end\n", 1, "expected name after `.`"),
    ("function a:b.c() end\n", 1, "expected a function body"),
    ("for = 1, 2 do end\n", 1, "expected name after `for`"),
    (
        "for i = , 2 do end\n",
        1,
        "expected start expression after `=`",
    ),
    (
        "for i = 1 2 do end\n",
        1,
        "expected `,` after start expression",
    ),
    (
        "for i = 1, do end\n",
        1,
        "expected end expression after `,`",
    ),
    (
        "for i = 1, 2, do end\n",
        1,
        "expected step expression after `,`",
    ),
    (
        "for i = 1, 2 end\n",
        1,
        "expected `do` after step expression",
    ),
    (
        "for i = 1, 2 do\n",
        1,
        "expected `end` to close numeric for loop block",
    ),
    ("for a, b do end\n", 1, "expected `in` after name list"),
    ("for a, b in do end\n", 1, "expected expressions after `in`"),
    (
        "for a, b in c end\n",
        1,
        "expected `do` after expression list",
    ),
    ("for a, in c do end\n", 1, "trailing commas are not allowed"),
    ("if then end\n", 1, "expected condition after `if`"),
    ("if x end\n", 1, "expected `then` after condition"),
    ("if x then\n", 2, "expected `end` to conclude `if`"),
    (
        "if x then y() elseif then end\n",
        1,
        "expected condition after `elseif`",
    ),
    ("while do end\n", 1, "expected a condition after `while`"),
    ("while x end\n", 1, "expected `do` after condition"),
    (
        "while x do\n  y()\n",
        2,
        "expected `end` to close while loop block",
    ),
    ("repeat x() \n", 2, "expected `until` after block"),
    (
        "repeat x() until\n",
        1,
        "expected a condition after `until`",
    ),
    ("do x()\n  y()\n", 2, "expected `end` to close do block"),
    ("goto\n", 1, "expected label name after `goto`"),
    (":: x\n", 2, "expected `::` after label"),
    ("::\n", 1, "expected label name after `::`"),
    (
        "x\n",
        2,
        "unexpected expression when looking for a statement",
    ),
    (
        "x y\n",
        1,
        "unexpected expression when looking for a statement",
    ),
    ("x, = 1\n", 1, "expected another variable"),
    ("x, f() = 1\n", 1, "can't assign to the result of a call"),
    ("x, y\n", 2, "expected `=` after name"),
    ("x =\n", 1, "expected values to set to"),
    ("x = 1,\n", 1, "trailing commas are not allowed"),
    ("local x =\n", 1, "expected an expression"),
    ("local x, = 1\n", 1, "trailing commas are not allowed"),
    (
        "local x <> = 1\n",
        1,
        "expected identifier after `<` for attribute",
    ),
    ("local x <const = 1\n", 1, "expected `>` to close attribute"),
    ("f(\n", 1, "expected `)` to close function call"),
    ("f(1,)\n", 1, "trailing commas are not allowed"),
    ("a.\n", 1, "expected identifier after `.`"),
    ("a[\n", 1, "expected expression after `[`"),
    ("a[1\n", 1, "expected `]` to close index expression"),
    ("a:\n", 1, "expected identifier after `:`"),
    ("a:b.c()\n", 1, "expected arguments after `:`"),
    ("(\n", 1, "expected an expression after `(`"),
    ("(1\n", 2, "expected `)` after expression"),
    ("x = -\n", 1, "expected an expression after -"),
    ("x = 1 +\n", 1, "expected expression after binary operator"),
    ("x = not\n", 1, "expected an expression after not"),
    ("x = function end\n", 1, "expected a function body"),
    (
        "x = function(a,) end\n",
        1,
        "trailing commas in arguments are not allowed",
    ),
    (
        "x = function(1) end\n",
        1,
        "expected a parameter name or `)`",
    ),
    ("x = function(... x) end\n", 1, "expected a `)`"),
    ("x = function(a b) end\n", 1, "expected a `)`"),
    ("x = { [1 \n", 1, "expected `]` after expression"),
    ("x = { [1] \n", 1, "expected `=` after expression"),
    ("x = { [1] = }\n", 1, "expected an expression after `=`"),
    ("x = { a = }\n", 1, "expected an expression after `=`"),
    ("x = { 1 2 }\n", 1, "expected `}` after last field"),
    ("x = { , }\n", 1, "expected a field"),
    ("end\n", 1, "unexpected token, this needs to be a statement"),
    (
        "return 1\nx = 2\n",
        2,
        "unexpected statement after last statement",
    ),
    (
        "return 1\nx  2\n",
        2,
        "unexpected expression when looking for a statement",
    ),
    (
        "break\nx = 1\n",
        2,
        "unexpected statement after last statement",
    ),
    (
        "function f() return 1 x = 2 end\n",
        1,
        "expected `end` to close function body block",
    ),
    (
        "x = 1;;\n",
        1,
        "unexpected token, this needs to be a statement",
    ),
];

#[test]
fn syntax_errors_match_full_moon() {
    for &(source, line, message) in FULL_MOON_ERRORS {
        let error = parse(&tokenize(source), &mut Recorder::default()).unwrap_err();
        assert_eq!(
            (error.line, error.message.as_str()),
            (line, message),
            "{source:?}"
        );
    }
}

#[test]
fn statements_after_a_final_return_are_errors() {
    // `full_moon` accepted this silently; Lua rejects it.
    let error = parse(&tokenize("return 1 return 2"), &mut Recorder::default()).unwrap_err();
    assert_eq!(
        error.message,
        "unexpected token, this needs to be a statement"
    );
}
