//! The immutable registry the rules read (design §4.3 step 5, §4.4).

use std::collections::BTreeMap;
use std::sync::Arc;

use strata_kernel::{DefId, PackId};
use strata_lua::{Def, FrozenRecord};

/// Every frozen definition, indexed by kind and `DefId`.
///
/// Cloning is cheap: clones share one immutable copy. Two registries are equal when they hold
/// the same packs in the same order and the same definitions with the same `DefId`s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentRegistry(Arc<Inner>);

#[derive(Debug, PartialEq, Eq)]
struct Inner {
    packs: Vec<PackId>,
    /// Records per kind in full-ID order, so a record's index is its `DefId`.
    kinds: BTreeMap<String, Vec<FrozenRecord>>,
}

impl ContentRegistry {
    /// Builds the registry from records in `(kind, ID)` order, as `freeze_resolved` returns them.
    pub(crate) fn new(packs: Vec<PackId>, records: Vec<FrozenRecord>) -> Self {
        let mut kinds: BTreeMap<String, Vec<FrozenRecord>> = BTreeMap::new();
        for record in records {
            let kind = record.record.kind.clone();
            kinds.entry(kind).or_default().push(record);
        }
        Self(Arc::new(Inner { packs, kinds }))
    }

    /// The packs in the order they were loaded: dependencies before their dependents.
    pub fn load_order(&self) -> &[PackId] {
        &self.0.packs
    }

    /// The `T` with this `DefId`.
    pub fn get<T: Def>(&self, id: DefId) -> Option<&T> {
        let record = self.records::<T>().get(id.0 as usize)?;
        T::from_frozen(&record.def)
    }

    /// The `DefId` and definition of the `T` with this full `pack:name` ID.
    pub fn find<T: Def>(&self, full_id: &str) -> Option<(DefId, &T)> {
        let records = self.records::<T>();
        let by_id = |r: &FrozenRecord| r.record.id.as_str().cmp(full_id);
        let index = records.binary_search_by(by_id).ok()?;
        Some((DefId(index as u32), T::from_frozen(&records[index].def)?))
    }

    /// The full ID of the `T` with this `DefId`.
    pub fn name<T: Def>(&self, id: DefId) -> Option<&str> {
        Some(self.records::<T>().get(id.0 as usize)?.record.id.as_str())
    }

    /// Every `T` as `(DefId, full ID, definition)`, in ID order.
    pub fn iter<'a, T: Def + 'a>(&'a self) -> impl Iterator<Item = (DefId, &'a str, &'a T)> {
        let records = self.records::<T>().iter().enumerate();
        records.filter_map(|(index, r)| {
            let id = DefId(index as u32);
            Some((id, r.record.id.as_str(), T::from_frozen(&r.def)?))
        })
    }

    fn records<T: Def>(&self) -> &[FrozenRecord] {
        self.0.kinds.get(T::KIND).map_or(&[], Vec::as_slice)
    }
}
