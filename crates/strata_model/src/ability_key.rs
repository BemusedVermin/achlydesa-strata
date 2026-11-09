//! Which ability definition an attached instance runs (design §10.5, §12.1).

use serde::{Deserialize, Serialize};
use strata_kernel::{DefId, StableHash};

/// Names one ability definition, wherever it was authored.
///
/// `ability` records have a `DefId` of their own. Abilities written inline in a unit or a relic
/// do not, so they are named by the record that holds them and their position in its list.
/// `DefId`s are numbered per kind, so the variant also says which kind the `DefId` is.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, StableHash,
)]
pub enum AbilityKey {
    /// An `ability` record.
    Def(DefId),
    /// Entry `index` (from zero) of a unit's `abilities`.
    Unit {
        /// The `unit` record.
        unit: DefId,
        /// The position in its `abilities`.
        index: u16,
    },
    /// Entry `index` (from zero) of a relic's `grants.abilities`.
    Relic {
        /// The `relic` record.
        relic: DefId,
        /// The position in its `grants.abilities`.
        index: u16,
    },
}
