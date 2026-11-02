//! Strata's own Lua checks, built on a `full_moon` syntax tree.

use std::collections::BTreeSet;

use full_moon::LuaVersion;
use full_moon::ast::{
    AnonymousFunction, Assignment, BinOp, Block, Expression, Field, FunctionArgs, FunctionBody,
    GenericFor, LocalAssignment, LocalFunction, NumericFor, Parameter, TableConstructor, Var,
};
use full_moon::tokenizer::{TokenReference, TokenType};
use full_moon::visitors::{Visit, Visitor};

use super::{Diagnostic, Severity};

/// Fields of a definition that are not archetype parameters.
const NOT_PARAMETERS: [&str; 6] = ["text", "name", "id", "meta", "tags", "vars"];

/// Checks one script. `logic` is true for files under a pack's logic folders,
/// where `/` division is also an error.
pub fn check(source: &str, logic: bool) -> Vec<Diagnostic> {
    let parsed = full_moon::parse_fallible(source, LuaVersion::lua54());
    let mut out = Vec::new();
    for error in parsed.errors() {
        out.push(Diagnostic {
            severity: Severity::Error,
            line: error.range().0.line(),
            message: format!("syntax error: {}", error.error_message()),
            fix: "fix the syntax error; the other checks need a parsable file".to_owned(),
        });
    }
    if out.is_empty() {
        let mut checker = Checker::new(logic);
        parsed.ast().nodes().visit(&mut checker);
        out = checker.found;
    }
    out.sort_by_key(|d| d.line);
    out
}

fn line_of(token: &TokenReference) -> usize {
    token.token().start_position().line()
}

struct Checker {
    logic: bool,
    found: Vec<Diagnostic>,
    /// Local names per open scope, innermost last.
    scopes: Vec<Vec<String>>,
    /// Depth of call argument lists being walked.
    call_depth: usize,
    /// Depth of table constructors opened inside call arguments.
    definition_tables: usize,
    /// Scope index of the outermost enclosing hook function, if any.
    hook_base: Option<usize>,
    /// For each open anonymous function, whether it started the current hook.
    owners: Vec<bool>,
}

impl Checker {
    fn new(logic: bool) -> Self {
        Self {
            logic,
            found: Vec::new(),
            scopes: Vec::new(),
            call_depth: 0,
            definition_tables: 0,
            hook_base: None,
            owners: Vec::new(),
        }
    }

    fn declare(&mut self, name: &TokenReference) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(name.token().to_string());
        }
    }

    fn is_upvalue(&self, name: &str) -> bool {
        let Some(base) = self.hook_base else {
            return false;
        };
        let depth = self
            .scopes
            .iter()
            .rposition(|s| s.iter().any(|n| n == name));
        depth.is_some_and(|d| d < base)
    }

    fn check_text(&mut self, table: &TableConstructor) {
        let mut text = None;
        let mut numbers = BTreeSet::new();
        for field in table.fields() {
            let Field::NameKey { key, value, .. } = field else {
                continue;
            };
            let name = key.token().to_string();
            if name == "text" {
                if let Expression::String(literal) = &**value {
                    text = string_body(literal).map(|body| (body, line_of(key)));
                }
            } else if !NOT_PARAMETERS.contains(&name.as_str()) {
                let mut collector = Numbers::default();
                value.visit(&mut collector);
                numbers.extend(collector.found);
            }
        }
        let Some((body, line)) = text else {
            return;
        };
        let spoken = digit_runs(&body);
        for number in numbers.difference(&spoken) {
            self.found.push(Diagnostic {
                severity: Severity::Warning,
                line,
                message: format!("card text omits the parameter {number}"),
                fix: format!(
                    "mention {number} in `text`, or delete the `text` override to use the generated wording"
                ),
            });
        }
    }
}

impl Visitor for Checker {
    fn visit_block(&mut self, _: &Block) {
        self.scopes.push(Vec::new());
    }

    fn visit_block_end(&mut self, _: &Block) {
        self.scopes.pop();
    }

    fn visit_function_body(&mut self, body: &FunctionBody) {
        let mut scope = Vec::new();
        for parameter in body.parameters() {
            if let Parameter::Name(name) = parameter {
                scope.push(name.token().to_string());
            }
        }
        self.scopes.push(scope);
    }

    fn visit_function_body_end(&mut self, _: &FunctionBody) {
        self.scopes.pop();
    }

    fn visit_anonymous_function(&mut self, _: &AnonymousFunction) {
        let starts_hook = self.hook_base.is_none() && self.definition_tables > 0;
        if starts_hook {
            self.hook_base = Some(self.scopes.len());
        }
        self.owners.push(starts_hook);
    }

    fn visit_anonymous_function_end(&mut self, _: &AnonymousFunction) {
        if self.owners.pop() == Some(true) {
            self.hook_base = None;
        }
    }

    fn visit_function_args(&mut self, args: &FunctionArgs) {
        self.call_depth += 1;
        if let FunctionArgs::TableConstructor(table) = args {
            self.check_text(table);
        }
    }

    fn visit_function_args_end(&mut self, _: &FunctionArgs) {
        self.call_depth -= 1;
    }

    fn visit_table_constructor(&mut self, _: &TableConstructor) {
        if self.call_depth > 0 {
            self.definition_tables += 1;
        }
    }

    fn visit_table_constructor_end(&mut self, _: &TableConstructor) {
        if self.call_depth > 0 {
            self.definition_tables -= 1;
        }
    }

    fn visit_local_assignment(&mut self, local: &LocalAssignment) {
        for name in local.names() {
            self.declare(name);
        }
    }

    fn visit_local_function(&mut self, local: &LocalFunction) {
        self.declare(local.name());
    }

    fn visit_numeric_for(&mut self, numeric: &NumericFor) {
        let name = numeric.index_variable().token().to_string();
        self.scopes.push(vec![name]);
    }

    fn visit_numeric_for_end(&mut self, _: &NumericFor) {
        self.scopes.pop();
    }

    fn visit_generic_for(&mut self, generic: &GenericFor) {
        let names = generic.names().iter().map(|n| n.token().to_string());
        self.scopes.push(names.collect());
    }

    fn visit_generic_for_end(&mut self, _: &GenericFor) {
        self.scopes.pop();
    }

    fn visit_assignment(&mut self, assignment: &Assignment) {
        for var in assignment.variables() {
            let Var::Name(name) = var else {
                continue;
            };
            if self.is_upvalue(&name.token().to_string()) {
                self.found.push(Diagnostic {
                    severity: Severity::Error,
                    line: line_of(name),
                    message: format!("hook assigns to the upvalue `{}`", name.token()),
                    fix: "keep per-instance state in `ctx.vars(self)` and declare its default in `vars = {}`"
                        .to_owned(),
                });
            }
        }
    }

    fn visit_expression(&mut self, expression: &Expression) {
        let Expression::BinaryOperator {
            binop: BinOp::Slash(token),
            ..
        } = expression
        else {
            return;
        };
        if self.logic {
            self.found.push(Diagnostic {
                severity: Severity::Error,
                line: line_of(token),
                message: "`/` always produces a float".to_owned(),
                fix: "use `//` for integer division".to_owned(),
            });
        }
    }
}

/// Collects integer literals outside function bodies.
#[derive(Default)]
struct Numbers {
    found: BTreeSet<String>,
    in_function: usize,
}

impl Visitor for Numbers {
    fn visit_anonymous_function(&mut self, _: &AnonymousFunction) {
        self.in_function += 1;
    }

    fn visit_anonymous_function_end(&mut self, _: &AnonymousFunction) {
        self.in_function -= 1;
    }

    fn visit_expression(&mut self, expression: &Expression) {
        if let Expression::Number(token) = expression {
            let text = token.token().to_string();
            if self.in_function == 0 && text.bytes().all(|b| b.is_ascii_digit()) {
                self.found.insert(text);
            }
        }
    }
}

fn string_body(token: &TokenReference) -> Option<String> {
    match token.token().token_type() {
        TokenType::StringLiteral { literal, .. } => Some(literal.to_string()),
        _ => None,
    }
}

/// The maximal runs of ASCII digits in `text`.
fn digit_runs(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|run| !run.is_empty())
        .map(str::to_owned)
        .collect()
}
