//! Boolean vote queries.

use strata_kernel::{Sym, Value};

use super::{Query, QueryEngine, QueryError, QueryKey, QueryTrace, TraceStep};

/// Asks a yes/no question. Modifiers vote `true` (allow) or `false` (deny); deny beats allow
/// beats the default, unless a vote is marked `override`, in which case only overriding votes
/// count and deny still beats allow among them.
#[derive(Clone, Debug)]
pub struct VoteQuery {
    /// The question, such as `can_enter` or a pack's custom key.
    pub key: Sym,
    /// The answer when nothing votes.
    pub default: bool,
    /// The figure or unit asked about.
    pub subject: Value,
    /// Extra inputs, such as the target.
    pub context: Value,
}

impl VoteQuery {
    fn named(key: &str, default: bool, subject: Value, context: Value) -> Self {
        Self {
            key: Sym::new(key),
            default,
            subject,
            context,
        }
    }

    /// May the subject enter a hex? Defaults to allowed.
    pub fn can_enter(subject: Value, context: Value) -> Self {
        Self::named("can_enter", true, subject, context)
    }

    /// May the subject climb a rise of `dz` levels onto a terrain? Defaults to allowed.
    pub fn can_climb(subject: Value, context: Value) -> Self {
        Self::named("can_climb", true, subject, context)
    }

    /// Is the subject engaged? Defaults to not engaged.
    pub fn is_engaged(subject: Value, context: Value) -> Self {
        Self::named("is_engaged", false, subject, context)
    }

    /// May the subject target something? Defaults to allowed.
    pub fn can_target(subject: Value, context: Value) -> Self {
        Self::named("can_target", true, subject, context)
    }

    /// Does leaving provoke a reprisal? Defaults to no.
    pub fn provokes_reprisal(subject: Value, context: Value) -> Self {
        Self::named("provokes_reprisal", false, subject, context)
    }
}

impl Query for VoteQuery {
    type Output = bool;

    fn key(&self) -> QueryKey {
        (
            self.key.clone(),
            Value::Bool(self.default),
            self.subject.clone(),
            self.context.clone(),
        )
    }

    fn resolve(&self, engine: &QueryEngine<'_>) -> Result<(bool, QueryTrace), QueryError> {
        let applied = engine.applied(&self.key, &self.subject, &self.context)?;
        let mut votes = Vec::new();
        for a in &applied {
            let vote = a.value.as_bool().map_err(|error| QueryError::Type {
                source_id: a.source.clone(),
                error,
            })?;
            votes.push((vote, a.overrides));
        }
        let any_override = votes.iter().any(|v| v.1);
        let mut pool = votes.iter().filter(|v| v.1 || !any_override).map(|v| v.0);
        let result = match pool.next() {
            None => self.default,
            Some(first) => first && pool.all(|v| v),
        };
        let trace = QueryTrace {
            query: self.key.clone(),
            subject: self.subject.clone(),
            context: self.context.clone(),
            base: Value::Bool(self.default),
            steps: applied.iter().map(TraceStep::from).collect(),
            unclamped: None,
            result: Value::Bool(result),
        };
        Ok((result, trace))
    }

    fn encode(output: &bool) -> Value {
        Value::Bool(*output)
    }

    fn decode(value: &Value) -> Option<bool> {
        value.as_bool().ok()
    }
}
