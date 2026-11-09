//! Strata's own Lua checks, driven by the events of the lint [`parser`](super::parser).

use std::collections::BTreeSet;

use super::lexer;
use super::parser::{self, Events, Field, Target};
use super::{Diagnostic, Severity};

/// Fields of a definition that are not archetype parameters.
const NOT_PARAMETERS: [&str; 6] = ["text", "name", "id", "meta", "tags", "vars"];

/// Checks one script. `logic` is true for files under a pack's logic folders,
/// where `/` division is also an error.
pub fn check(source: &str, logic: bool) -> Vec<Diagnostic> {
    let tokens = lexer::tokenize(source);
    let mut checker = Checker::new(logic);
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

struct Checker {
    logic: bool,
    /// Findings with the offset of the construct that produced them, for source order.
    found: Vec<(usize, Diagnostic)>,
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

    /// What a hook writing `target` would keep between calls, as a message and a fix, or
    /// `None` outside hooks and for the hook's own locals and parameters.
    fn hidden_state(&self, target: &Target) -> Option<(String, &'static str)> {
        let base = self.hook_base?;
        let depth = self.scopes.iter().rposition(|s| s.contains(&target.name));
        let name = &target.name;
        let found = match (depth, target.field) {
            (Some(d), false) if d < base => (
                format!("hook assigns to the upvalue `{name}`"),
                "keep per-instance state in `ctx.vars(self)` and declare its default in `vars = {}`",
            ),
            (Some(d), true) if d < base => (
                format!("hook writes into the upvalue table `{name}`"),
                "build the table inside the hook, or keep per-instance state in `ctx.vars(self)`; a table shared between calls is hidden state",
            ),
            (None, false) => (
                format!("hook assigns to the global `{name}`"),
                "declare it `local` inside the hook; globals are frozen once the packs load, and per-instance state belongs in `ctx.vars(self)`",
            ),
            (None, true) => (
                format!("hook writes into the global table `{name}`"),
                "copy what you need into a local table; tables reachable from globals are frozen once the packs load",
            ),
            _ => return None,
        };
        Some(found)
    }
}

impl Events for Checker {
    fn open_scope(&mut self, names: Vec<String>) {
        self.scopes.push(names);
    }

    fn close_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: String) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(name);
        }
    }

    fn function_start(&mut self) {
        let starts_hook = self.hook_base.is_none() && self.definition_tables > 0;
        if starts_hook {
            self.hook_base = Some(self.scopes.len());
        }
        self.owners.push(starts_hook);
    }

    fn function_end(&mut self) {
        if self.owners.pop() == Some(true) {
            self.hook_base = None;
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

    fn assignment(&mut self, at: usize, targets: Vec<Target>) {
        for target in targets {
            if let Some((message, fix)) = self.hidden_state(&target) {
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
