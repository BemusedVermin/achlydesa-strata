//! Numeric stat queries.

use strata_kernel::{Sym, Value, ValueError};
use strata_model::ModifierLayer;

use super::modifier::Applied;
use super::{Query, QueryEngine, QueryError, QueryKey, QueryTrace, TraceStep};

/// Asks for a stat's current value: base, then modifiers by layer, then clamped.
#[derive(Clone, Debug)]
pub struct StatQuery {
    /// The stat, such as `defense`.
    pub stat: Sym,
    /// The value before modifiers, from the unit's definition.
    pub base: i64,
    /// The figure or unit asked about.
    pub subject: Value,
    /// Extra inputs, such as the attacker.
    pub context: Value,
}

impl Query for StatQuery {
    type Output = i64;

    fn key(&self) -> QueryKey {
        (
            self.stat.clone(),
            Value::Int(self.base),
            self.subject.clone(),
            self.context.clone(),
        )
    }

    fn resolve(&self, engine: &QueryEngine<'_>) -> Result<(i64, QueryTrace), QueryError> {
        let bounds = engine.bounds(&self.stat)?;
        let applied = engine.applied(&self.stat, &self.subject, &self.context)?;
        let mut value = self.base;
        let mut steps = Vec::new();
        for a in &applied {
            value = apply(value, a).map_err(|error| QueryError::Type {
                source_id: a.source.clone(),
                error,
            })?;
            steps.push(TraceStep {
                after: Some(value),
                ..TraceStep::from(a)
            });
        }
        let result = value.clamp(i64::from(bounds.min), i64::from(bounds.max));
        let trace = QueryTrace {
            query: self.stat.clone(),
            subject: self.subject.clone(),
            context: self.context.clone(),
            base: Value::Int(self.base),
            steps,
            unclamped: Some(value),
            bounds: Some(bounds),
            result: Value::Int(result),
        };
        Ok((result, trace))
    }

    fn encode(output: &i64) -> Value {
        Value::Int(*output)
    }

    fn decode(value: &Value) -> Option<i64> {
        value.as_int().ok()
    }
}

/// `mul` takes a whole multiplier or a `[numerator, denominator]` fraction, truncating toward
/// zero. `final` adds after every other layer.
fn apply(value: i64, a: &Applied) -> Result<i64, ValueError> {
    Ok(match a.layer {
        ModifierLayer::Set => a.value.as_int()?,
        ModifierLayer::Add | ModifierLayer::Final => value.saturating_add(a.value.as_int()?),
        ModifierLayer::Mul => match &a.value {
            Value::List(parts) if parts.len() == 2 => {
                let (n, d) = (parts[0].as_int()?, parts[1].as_int()?);
                if d == 0 {
                    value
                } else {
                    let scaled = i128::from(value) * i128::from(n) / i128::from(d);
                    scaled.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
                }
            }
            other => value.saturating_mul(other.as_int()?),
        },
    })
}
