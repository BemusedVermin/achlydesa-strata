//! The query engine: resolves queries and memoizes within a resolution step.

use std::cell::RefCell;
use std::collections::BTreeMap;

use strata_kernel::{Sym, Value};
use strata_model::StatDef;

use super::logic::{Logics, QueryError};
use super::modifier::{Applied, SourcedModifier, collect};
use super::{Query, QueryKey, QueryTrace};

/// Inclusive bounds a stat result is clamped to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatBounds {
    /// Smallest allowed value.
    pub min: i32,
    /// Largest allowed value.
    pub max: i32,
}

impl From<&StatDef> for StatBounds {
    fn from(def: &StatDef) -> Self {
        Self {
            min: def.min,
            max: def.max,
        }
    }
}

/// Resolves queries against a fixed set of modifiers.
pub struct QueryEngine<'a> {
    modifiers: &'a [SourcedModifier],
    bounds: &'a BTreeMap<Sym, StatBounds>,
    logics: Logics<'a>,
    cache: RefCell<BTreeMap<QueryKey, Value>>,
}

impl<'a> QueryEngine<'a> {
    /// Creates an engine with an empty cache.
    pub fn new(
        modifiers: &'a [SourcedModifier],
        bounds: &'a BTreeMap<Sym, StatBounds>,
        logics: Logics<'a>,
    ) -> Self {
        Self {
            modifiers,
            bounds,
            logics,
            cache: RefCell::default(),
        }
    }

    /// Answers `query`, using the memo cache when the same question was already asked.
    pub fn query<Q: Query>(&self, query: &Q) -> Result<Q::Output, QueryError> {
        let key = query.key();
        let hit = self.cache.borrow().get(&key).and_then(Q::decode);
        if let Some(output) = hit {
            return Ok(output);
        }
        let (output, _) = query.resolve(self)?;
        self.cache.borrow_mut().insert(key, Q::encode(&output));
        Ok(output)
    }

    /// Answers `query` afresh and returns how, bypassing the cache.
    pub fn trace<Q: Query>(&self, query: &Q) -> Result<(Q::Output, QueryTrace), QueryError> {
        query.resolve(self)
    }

    /// Drops every memoized answer. Call after each applied event.
    pub fn invalidate(&self) {
        self.cache.borrow_mut().clear();
    }

    /// Number of memoized answers.
    pub fn cached(&self) -> usize {
        self.cache.borrow().len()
    }

    pub(crate) fn bounds(&self, stat: &Sym) -> Result<StatBounds, QueryError> {
        self.bounds
            .get(stat)
            .copied()
            .ok_or_else(|| QueryError::UnknownStat(stat.clone()))
    }

    pub(crate) fn applied(
        &self,
        query: &Sym,
        subject: &Value,
        context: &Value,
    ) -> Result<Vec<Applied>, QueryError> {
        collect(self.modifiers, self.logics, query, subject, context)
    }
}
