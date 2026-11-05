//! The rules inspector's model: a query trace laid out as base, modifiers, clamp and result, so a
//! dev panel can answer "why is this number 5?" (design §16.6).

use strata_kernel::Value;
use strata_model::ModifierLayer;
use strata_rules::{QueryTrace, TraceStep};

/// What one modifier did to the value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Replaced the value (`set` layer).
    Set(i64),
    /// Added an amount (`add` layer).
    Add(i64),
    /// Multiplied by `numerator / denominator`, truncating toward zero (`mul` layer). A zero
    /// denominator leaves the value unchanged.
    Mul(i64, i64),
    /// Added an amount after every other layer (`final` layer).
    Final(i64),
    /// Voted to allow (`true`) or deny (`false`) a yes/no question.
    Vote(bool),
    /// A contribution the inspector cannot read as any of the above.
    Other(String),
}

/// One modifier's line in the inspector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectStep {
    /// The ability, terrain, relic or ruleset that provides the modifier.
    pub source: String,
    /// What it did.
    pub change: Change,
    /// Whether its logic ran in Lua, the slow path, rather than natively.
    pub lua: bool,
    /// Whether it was marked `override`.
    pub overrides: bool,
    /// The running value after it, for stats.
    pub after: Option<i64>,
}

/// The bounds a stat was clamped to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clamp {
    /// Smallest allowed value.
    pub min: i64,
    /// Largest allowed value.
    pub max: i64,
    /// The value before clamping.
    pub unclamped: i64,
}

/// A whole trace, ready to display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Inspection {
    /// The query name, such as `defense`.
    pub query: String,
    /// The base value or default vote.
    pub base: Value,
    /// Each modifier in application order.
    pub steps: Vec<InspectStep>,
    /// The clamp, for stats.
    pub clamp: Option<Clamp>,
    /// The final answer.
    pub result: Value,
}

impl Inspection {
    /// Lays out `trace`.
    pub fn of(trace: &QueryTrace) -> Self {
        let clamp = trace
            .bounds
            .zip(trace.unclamped)
            .map(|(b, unclamped)| Clamp {
                min: i64::from(b.min),
                max: i64::from(b.max),
                unclamped,
            });
        Self {
            query: trace.query.as_str().to_owned(),
            base: trace.base.clone(),
            steps: trace.steps.iter().map(step).collect(),
            clamp,
            result: trace.result.clone(),
        }
    }

    /// How many steps ran in Lua: a hint at where a query spends its time.
    pub fn lua_steps(&self) -> usize {
        self.steps.iter().filter(|s| s.lua).count()
    }

    /// The trace as text, one line per stage.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = vec![self.query.clone(), format!("Base {}", show(&self.base))];
        for s in &self.steps {
            let mut tags = vec![if s.lua { "Lua" } else { "native" }];
            if s.overrides {
                tags.push("override");
            }
            let mut line = format!(
                "{} from {} ({})",
                word(&s.change),
                s.source,
                tags.join(", ")
            );
            if let Some(after) = s.after {
                line.push_str(&format!(" = {after}"));
            }
            lines.push(line);
        }
        if let Some(c) = self.clamp {
            let mut line = format!("Clamped to {}..{}", c.min, c.max);
            if !(c.min..=c.max).contains(&c.unclamped) {
                line.push_str(&format!(" (was {})", c.unclamped));
            }
            lines.push(line);
        }
        lines.push(format!("Result {}", show(&self.result)));
        lines
    }
}

fn step(s: &TraceStep) -> InspectStep {
    InspectStep {
        source: s.source.as_str().to_owned(),
        change: change(s),
        lua: s.scripted,
        overrides: s.overrides,
        after: s.after,
    }
}

/// Reads a step's contribution the way the stat and vote queries apply it.
fn change(s: &TraceStep) -> Change {
    let other = || Change::Other(show(&s.value));
    // Stat steps carry a running value; vote steps do not.
    if s.after.is_none() {
        return s.value.as_bool().map_or_else(|_| other(), Change::Vote);
    }
    let int = s.value.as_int().ok();
    match (s.layer, &s.value) {
        (ModifierLayer::Mul, Value::List(parts)) if parts.len() == 2 => {
            match (parts[0].as_int(), parts[1].as_int()) {
                (Ok(n), Ok(d)) => Change::Mul(n, d),
                _ => other(),
            }
        }
        (ModifierLayer::Set, _) => int.map_or_else(other, Change::Set),
        (ModifierLayer::Add, _) => int.map_or_else(other, Change::Add),
        (ModifierLayer::Mul, _) => int.map_or_else(other, |n| Change::Mul(n, 1)),
        (ModifierLayer::Final, _) => int.map_or_else(other, Change::Final),
    }
}

fn word(change: &Change) -> String {
    match change {
        Change::Set(n) => format!("Set {n}"),
        Change::Add(n) => format!("{n:+}"),
        Change::Final(n) => format!("Final {n:+}"),
        Change::Mul(n, 1) => format!("×{n}"),
        Change::Mul(n, d) => format!("×{n}/{d}"),
        Change::Vote(true) => "Allow".into(),
        Change::Vote(false) => "Deny".into(),
        Change::Other(text) => text.clone(),
    }
}

fn show(value: &Value) -> String {
    match value {
        Value::Int(n) => n.to_string(),
        Value::Bool(true) => "allow".into(),
        Value::Bool(false) => "deny".into(),
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests;
