//! Interning of full IDs into `DefId`s, and resolution of authored references (design §4.4).

use std::collections::{BTreeMap, BTreeSet};

use strata_kernel::{DefId, PackId};
use strata_lua::{Record, Resolver};

/// Every definition's `DefId`, assigned per kind in sorted order of the full `pack:name` string.
pub(crate) struct Interner {
    ids: BTreeMap<(String, String), DefId>,
    dependencies: BTreeMap<PackId, BTreeSet<PackId>>,
}

impl Interner {
    pub(crate) fn new(
        records: &[Record],
        dependencies: BTreeMap<PackId, BTreeSet<PackId>>,
    ) -> Self {
        let full: BTreeSet<(&str, &str)> = records
            .iter()
            .map(|r| (r.kind.as_str(), r.id.as_str()))
            .collect();
        let mut next: BTreeMap<&str, u32> = BTreeMap::new();
        let mut ids = BTreeMap::new();
        for (kind, id) in full {
            let counter = next.entry(kind).or_default();
            ids.insert((kind.to_owned(), id.to_owned()), DefId(*counter));
            *counter += 1;
        }
        Self { ids, dependencies }
    }

    fn get(&self, kind: &str, id: &str) -> Option<DefId> {
        self.ids.get(&(kind.to_owned(), id.to_owned())).copied()
    }

    /// The kinds of the definitions that any of `candidates` names, in kind order.
    fn kinds_of(&self, candidates: &[String]) -> Vec<&str> {
        let named = self.ids.keys().filter(|(_, id)| candidates.contains(id));
        let kinds: BTreeSet<&str> = named.map(|(kind, _)| kind.as_str()).collect();
        kinds.into_iter().collect()
    }

    fn missing(&self, kind: &str, authored: &str, candidates: &[String]) -> String {
        match self.kinds_of(candidates).as_slice() {
            [] => format!("unknown {kind} `{authored}`"),
            others => format!(
                "`{authored}` is a {}, but a {kind} is required",
                others.join(" and a ")
            ),
        }
    }
}

impl Resolver for Interner {
    /// A full ID names itself. A bare one is the pack's own definition if it has one, otherwise
    /// the one definition among the pack's dependencies; two of those is ambiguous.
    fn resolve(&self, pack: &PackId, kind: &str, authored: &str) -> Result<DefId, String> {
        let candidates: Vec<String> = if authored.contains(':') {
            vec![authored.to_owned()]
        } else {
            let deps = self.dependencies.get(pack).into_iter().flatten();
            let own = format!("{pack}:{authored}");
            std::iter::once(own)
                .chain(deps.map(|dep| format!("{dep}:{authored}")))
                .collect()
        };
        let found: Vec<(&String, DefId)> = candidates
            .iter()
            .filter_map(|id| Some((id, self.get(kind, id)?)))
            .collect();
        match found.as_slice() {
            [] => Err(self.missing(kind, authored, &candidates)),
            [(_, def)] => Ok(*def),
            [(own, def), ..] if **own == candidates[0] => Ok(*def),
            _ => {
                let names: Vec<&str> = found.iter().map(|(id, _)| id.as_str()).collect();
                Err(format!(
                    "`{authored}` is ambiguous: it could be {}",
                    names.join(" or ")
                ))
            }
        }
    }
}
