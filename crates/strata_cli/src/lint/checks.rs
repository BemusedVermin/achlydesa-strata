//! Strata's own Lua checks, driven by the events of the lint [`parser`](super::parser).

use std::collections::BTreeSet;

use super::lexer;
use super::parser::{self, Events, Field, Target};
use super::{Diagnostic, Severity};

/// Fields of a definition that are not archetype parameters.
const NOT_PARAMETERS: [&str; 6] = ["text", "name", "id", "meta", "tags", "vars"];
/// Library functions that write into the table passed as their first argument.
const MUTATORS: [&str; 4] = ["table.insert", "table.remove", "table.sort", "rawset"];

/// Checks one script. `logic` is true for files under a pack's logic folders,
/// where `/` division is also an error.
pub fn check(source: &str, logic: bool) -> Vec<Diagnostic> {
    let tokens = lexer::tokenize(source);
    // A first pass finds the names that definition tables use as values (`{ targets =
    // targets }`), so the functions declared under those names are checked as hooks.
    let mut first = Checker::new(logic, BTreeSet::new());
    let named = match parser::parse(&tokens, &mut first) {
        Ok(()) => first.field_names,
        Err(_) => BTreeSet::new(),
    };
    let mut checker = Checker::new(logic, named);
    if let Err(error) = parser::parse(&tokens, &mut checker) {
        return vec![Diagnostic {
            severity: Severity::Error,
            line: error.line,
            message: format!("syntax error: {}", error.message),
            fix: "fix the syntax error; the other checks need a parsable file".to_owned(),
        }];
    }
    let mut found = checker.found;
    found.sort_by_key(|(at, d)| (d.line, *at));
    found.into_iter().map(|(_, d)| d).collect()
}

/// A local variable in scope.
struct Local {
    name: String,
    /// The captured or global table it was set from, inside a hook (`local c = cache`).
    alias: Option<Hidden>,
}

/// A table a hook can reach that outlives the call.
#[derive(Clone)]
enum Hidden {
    /// A local of an enclosing function, captured as an upvalue.
    Upvalue(String),
    /// A global.
    Global(String),
}

/// The walk state saved when a function starts and restored when it ends.
struct Frame {
    call_depth: usize,
    definition_tables: usize,
    hook_base: Option<usize>,
}

struct Checker {
    logic: bool,
    /// Findings with the offset of the construct that produced them, for source order.
    found: Vec<(usize, Diagnostic)>,
    /// Locals per open scope, innermost last.
    scopes: Vec<Vec<Local>>,
    /// Depth of call argument lists being walked in the current function body.
    call_depth: usize,
    /// Depth of table constructors opened inside call arguments in the current function body.
    definition_tables: usize,
    /// Scope index of the innermost enclosing hook function, if any.
    hook_base: Option<usize>,
    /// One frame per open function.
    frames: Vec<Frame>,
    /// Names whose functions are hooks because a definition table names them as a value.
    hook_names: BTreeSet<String>,
    /// Names used as values in definition tables, for the next pass's `hook_names`.
    field_names: BTreeSet<String>,
}

impl Checker {
    fn new(logic: bool, hook_names: BTreeSet<String>) -> Self {
        Self {
            logic,
            found: Vec::new(),
            scopes: Vec::new(),
            call_depth: 0,
            definition_tables: 0,
            hook_base: None,
            frames: Vec::new(),
            hook_names,
            field_names: BTreeSet::new(),
        }
    }

    /// What `name` refers to inside a hook, if it outlives the call: a global, an upvalue,
    /// or the hook's own local set from one of those. The flag is true for such a local.
    fn hidden(&self, name: &str) -> Option<(Hidden, bool)> {
        let base = self.hook_base?;
        let found = self
            .scopes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(depth, scope)| {
                let local = scope.iter().rev().find(|l| l.name == name)?;
                Some((depth, local))
            });
        match found {
            None => Some((Hidden::Global(name.to_owned()), false)),
            Some((depth, _)) if depth < base => Some((Hidden::Upvalue(name.to_owned()), false)),
            Some((_, local)) => Some((local.alias.clone()?, true)),
        }
    }

    /// What a hook writing `target` would keep between calls, as a message and a fix, or
    /// `None` outside hooks and for the hook's own locals and parameters.
    fn hidden_state(&self, target: &Target) -> Option<(String, &'static str)> {
        let (hidden, via_alias) = self.hidden(&target.name)?;
        let through = if via_alias {
            format!(" through the local `{}`", target.name)
        } else {
            String::new()
        };
        let found = match (hidden, target.field) {
            (_, false) if via_alias => return None,
            (Hidden::Upvalue(name), false) => (
                format!("hook assigns to the upvalue `{name}`"),
                "keep per-instance state in `ctx.vars(self)` and declare its default in `vars = {}`",
            ),
            (Hidden::Upvalue(name), true) => (
                format!("hook writes into the upvalue table `{name}`{through}"),
                "build the table inside the hook, or keep per-instance state in `ctx.vars(self)`; a table shared between calls is hidden state",
            ),
            (Hidden::Global(name), false) => (
                format!("hook assigns to the global `{name}`"),
                "declare it `local` inside the hook; globals are frozen once the packs load, and per-instance state belongs in `ctx.vars(self)`",
            ),
            (Hidden::Global(name), true) => (
                format!("hook writes into the global table `{name}`{through}"),
                "copy what you need into a local table; tables reachable from globals are frozen once the packs load",
            ),
        };
        Some(found)
    }

    fn report(&mut self, at: usize, target: &Target) {
        if let Some((message, fix)) = self.hidden_state(target) {
            let diagnostic = Diagnostic {
                severity: Severity::Error,
                line: target.line,
                message,
                fix: fix.to_owned(),
            };
            self.found.push((at, diagnostic));
        }
    }
}

impl Events for Checker {
    fn open_scope(&mut self, names: Vec<String>) {
        let locals = names.into_iter().map(|name| Local { name, alias: None });
        self.scopes.push(locals.collect());
    }

    fn close_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: String) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(Local { name, alias: None });
        }
    }

    /// A function written directly in a definition table, or named by one, is a hook, and so
    /// is one nested in another hook's definition table (`r.action { targets = ... }` inside
    /// `register`): the enclosing function's locals are then upvalues it keeps between calls.
    fn function_start(&mut self, name: Option<String>) {
        let named = name.is_some_and(|n| self.hook_names.contains(&n));
        let starts_hook = named || self.definition_tables > 0;
        self.frames.push(Frame {
            call_depth: self.call_depth,
            definition_tables: self.definition_tables,
            hook_base: self.hook_base,
        });
        self.call_depth = 0;
        self.definition_tables = 0;
        if starts_hook {
            self.hook_base = Some(self.scopes.len());
        }
    }

    fn function_end(&mut self) {
        if let Some(frame) = self.frames.pop() {
            self.call_depth = frame.call_depth;
            self.definition_tables = frame.definition_tables;
            self.hook_base = frame.hook_base;
        }
    }

    fn args_start(&mut self) {
        self.call_depth += 1;
    }

    fn args_end(&mut self) {
        self.call_depth -= 1;
    }

    fn table_start(&mut self) {
        if self.call_depth > 0 {
            self.definition_tables += 1;
        }
    }

    fn table_end(&mut self) {
        if self.call_depth > 0 {
            self.definition_tables -= 1;
        }
    }

    fn call_table(&mut self, at: usize, fields: Vec<Field>) {
        let mut text = None;
        let mut numbers = BTreeSet::new();
        for field in fields {
            if field.key == "text" {
                if let Some(body) = field.string {
                    text = Some((body, field.line));
                }
            } else if !NOT_PARAMETERS.contains(&field.key.as_str()) {
                numbers.extend(field.numbers);
            }
        }
        let Some((body, line)) = text else {
            return;
        };
        let spoken = digit_runs(&body);
        for number in numbers.difference(&spoken) {
            let diagnostic = Diagnostic {
                severity: Severity::Warning,
                line,
                message: format!("card text omits the parameter {number}"),
                fix: format!(
                    "mention {number} in `text`, or delete the `text` override to use the generated wording"
                ),
            };
            self.found.push((at, diagnostic));
        }
    }

    fn field_name(&mut self, name: String) {
        if self.definition_tables > 0 {
            self.field_names.insert(name);
        }
    }

    fn call(&mut self, at: usize, callee: String, first: Target) {
        if MUTATORS.contains(&callee.as_str()) {
            self.report(
                at,
                &Target {
                    field: true,
                    ..first
                },
            );
        }
    }

    fn alias(&mut self, name: String, source: Target) {
        // The local is already declared, so `local t = t` resolves to itself and is not tracked.
        let hidden = self.hidden(&source.name).map(|(hidden, _)| hidden);
        let Some(scope) = self.scopes.last_mut() else {
            return;
        };
        if let Some(local) = scope.iter_mut().rev().find(|l| l.name == name) {
            local.alias = hidden;
        }
    }

    fn assignment(&mut self, at: usize, targets: Vec<Target>) {
        for target in targets {
            self.report(at, &target);
        }
    }

    fn division(&mut self, at: usize, line: usize) {
        if self.logic {
            let diagnostic = Diagnostic {
                severity: Severity::Error,
                line,
                message: "`/` always produces a float".to_owned(),
                fix: "use `//` for integer division".to_owned(),
            };
            self.found.push((at, diagnostic));
        }
    }
}

/// The maximal runs of ASCII digits in `text`.
fn digit_runs(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_ascii_digit())
        .filter(|run| !run.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests;
