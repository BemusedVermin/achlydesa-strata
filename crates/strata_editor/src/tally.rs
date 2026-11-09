//! Counting a map's tiles and features against its inventory, shared by validation and the
//! palette's stock.

use std::collections::BTreeMap;

use strata_kernel::{DefId, Hex, Sym};

/// What a name counts against: its definition, or the name itself when it does not resolve.
pub(crate) type Key<'a> = Result<DefId, &'a Sym>;

/// One definition's count: how many the inventory holds, and where the map uses it.
#[derive(Debug, Default)]
pub(crate) struct Count<'a> {
    /// How many the inventory holds, over every spelling.
    pub(crate) available: u32,
    /// The name as the map first spells it, if the map uses it at all.
    pub(crate) name: Option<&'a Sym>,
    /// The anchors of every use.
    pub(crate) uses: Vec<Hex>,
}

/// Counts `placed` against the inventory's `caps`. Names are compared by what `resolve` makes
/// of them, so `grass` and `pack:grass` are one tile.
pub(crate) fn tally<'a>(
    placed: impl Iterator<Item = (&'a Sym, Hex)>,
    caps: &'a BTreeMap<Sym, u32>,
    resolve: impl Fn(&Sym) -> Option<DefId>,
) -> BTreeMap<Key<'a>, Count<'a>> {
    let key = |name: &'a Sym| -> Key<'a> { resolve(name).ok_or(name) };
    let mut counts: BTreeMap<Key<'a>, Count<'a>> = BTreeMap::new();
    for (name, &cap) in caps {
        counts.entry(key(name)).or_default().available += cap;
    }
    for (name, at) in placed {
        let count = counts.entry(key(name)).or_default();
        count.name.get_or_insert(name);
        count.uses.push(at);
    }
    counts
}
