//! The immutable registry the rules read (design §4.3 step 5, §4.4).

use std::collections::BTreeMap;
use std::sync::Arc;

use strata_kernel::{DefId, PackId};
use strata_lua::{Def, FrozenRecord};
use strata_model::ScriptRef;

use crate::identity::{ContentHash, PackIdentity, set_hash};

/// Every frozen definition, indexed by kind and `DefId`.
///
/// Cloning is cheap: clones share one immutable copy. Two registries are equal when they hold
/// the same packs in the same order and the same definitions with the same `DefId`s.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentRegistry(Arc<Inner>);

#[derive(Debug, PartialEq, Eq)]
struct Inner {
    packs: Vec<PackId>,
    identities: Vec<PackIdentity>,
    set_hash: ContentHash,
    /// Records per kind in full-ID order, so a record's index is its `DefId`.
    kinds: BTreeMap<String, Vec<FrozenRecord>>,
}

impl ContentRegistry {
    /// Builds the registry from records in `(kind, ID)` order, as `freeze_resolved` returns them.
    pub(crate) fn new(
        packs: Vec<PackId>,
        identities: Vec<PackIdentity>,
        records: Vec<FrozenRecord>,
    ) -> Self {
        let mut kinds: BTreeMap<String, Vec<FrozenRecord>> = BTreeMap::new();
        for record in records {
            let kind = record.record.kind.clone();
            kinds.entry(kind).or_default().push(record);
        }
        let set_hash = set_hash(&identities);
        Self(Arc::new(Inner {
            packs,
            identities,
            set_hash,
            kinds,
        }))
    }

    /// The packs in the order they were loaded: dependencies before their dependents.
    pub fn load_order(&self) -> &[PackId] {
        &self.0.packs
    }

    /// Each pack's ID, version and content hash, in load order.
    pub fn pack_identities(&self) -> &[PackIdentity] {
        &self.0.identities
    }

    /// A hash of the whole pack set: every pack's identity, in load order.
    pub fn set_hash(&self) -> ContentHash {
        self.0.set_hash
    }

    /// Every function that became a `ScriptRef`, across all records.
    pub fn scripts(&self) -> Vec<ScriptRef> {
        let records = self.0.kinds.values().flatten();
        records.flat_map(|r| r.scripts.iter().cloned()).collect()
    }

    /// The frozen records of the definitions whose kind is `kind`, in ID order.
    pub fn records_of(&self, kind: &str) -> &[FrozenRecord] {
        self.0.kinds.get(kind).map_or(&[], Vec::as_slice)
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

    /// The `T` an authored `name` in `pack` refers to: the name as written, then `pack:name`,
    /// then the first `T` in any pack whose ID ends in `:name`.
    pub fn lookup<T: Def>(&self, pack: &str, name: &str) -> Option<(DefId, &T)> {
        let suffix = format!(":{name}");
        self.find::<T>(name)
            .or_else(|| self.find::<T>(&format!("{pack}:{name}")))
            .or_else(|| {
                let mut hits = self.iter::<T>().filter(|(_, id, _)| id.ends_with(&suffix));
                hits.next().map(|(id, _, def)| (id, def))
            })
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
