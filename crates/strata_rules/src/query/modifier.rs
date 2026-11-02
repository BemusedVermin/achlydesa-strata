//! Modifiers collected for one query, in a registration-independent order.

use strata_kernel::{Sym, Value};
use strata_model::{ModifierDef, ModifierLayer};

use super::logic::{EvalCtx, Logics, QueryError};

/// A modifier with the identity it sorts by.
#[derive(Clone, Debug)]
pub struct SourcedModifier {
    /// The ability, terrain or relic that provides it.
    pub source: Sym,
    /// Orders modifiers within a layer; lower applies first.
    pub priority: i32,
    /// The definition.
    pub def: ModifierDef,
}

/// A modifier that applies to the current question, with its evaluated value.
#[derive(Clone, Debug)]
pub(crate) struct Applied {
    pub layer: ModifierLayer,
    pub priority: i32,
    pub source: Sym,
    pub value: Value,
    pub overrides: bool,
}

pub(crate) fn layer_rank(layer: ModifierLayer) -> u8 {
    match layer {
        ModifierLayer::Set => 0,
        ModifierLayer::Add => 1,
        ModifierLayer::Mul => 2,
        ModifierLayer::Final => 3,
    }
}

/// Evaluates every modifier on `query` and returns those that apply, sorted by
/// (layer, priority, source, value). The value is the last tie-break so the order
/// never depends on the order modifiers were registered in.
pub(crate) fn collect(
    modifiers: &[SourcedModifier],
    logics: Logics<'_>,
    query: &Sym,
    subject: &Value,
    context: &Value,
) -> Result<Vec<Applied>, QueryError> {
    let mut out = Vec::new();
    for m in modifiers.iter().filter(|m| &m.def.query == query) {
        let cx = EvalCtx {
            source: &m.source,
            subject,
            context,
        };
        let flag = |logic: &Option<_>, default: bool| match logic {
            None => Ok(default),
            Some(l) => logics.eval(l, &cx).map_err(QueryError::from).and_then(|v| {
                v.as_bool().map_err(|error| QueryError::Type {
                    source_id: m.source.clone(),
                    error,
                })
            }),
        };
        if !flag(&m.def.select, true)? || !flag(&m.def.when, true)? {
            continue;
        }
        out.push(Applied {
            layer: m.def.layer,
            priority: m.priority,
            source: m.source.clone(),
            value: logics.eval(&m.def.value, &cx)?,
            overrides: m.def.overrides,
        });
    }
    out.sort_by(|a, b| {
        (layer_rank(a.layer), a.priority, &a.source, &a.value).cmp(&(
            layer_rank(b.layer),
            b.priority,
            &b.source,
            &b.value,
        ))
    });
    Ok(out)
}
