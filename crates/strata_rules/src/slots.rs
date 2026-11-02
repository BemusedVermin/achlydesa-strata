//! The twelve standard slot names (design §6.1).

/// Round loop, initiative and turn order.
pub const TURNS: &str = "turns";
/// Hidden numbered tokens and decoys.
pub const TOKENS: &str = "tokens";
/// A unit's turn: steps and per-figure slots.
pub const ACTIVATION: &str = "activation";
/// Path cost, climbing, stop-on-enter terrain and falling.
pub const MOVEMENT: &str = "movement";
/// Who is engaged, and reprisals for leaving.
pub const ENGAGEMENT: &str = "engagement";
/// Attack and defense pools, hits, blocks and wounds.
pub const COMBAT: &str = "combat";
/// The line-of-sight service.
pub const LOS: &str = "los";
/// Relic spots, claiming, reveal and duration.
pub const RELICS: &str = "relics";
/// Heroes and squads, wounds, destruction and removal.
pub const UNITS: &str = "units";
/// Start zones, placement order and setup steps.
pub const DEPLOYMENT: &str = "deployment";
/// Army legality: points, uniqueness and copies.
pub const ARMY: &str = "army";
/// Evaluating victory conditions.
pub const VICTORY: &str = "victory";

/// Every standard slot, in design order.
pub const STANDARD: [&str; 12] = [
    TURNS, TOKENS, ACTIVATION, MOVEMENT, ENGAGEMENT, COMBAT, LOS, RELICS, UNITS, DEPLOYMENT, ARMY,
    VICTORY,
];

/// Returns whether `name` is one of the twelve standard slots.
pub fn is_standard(name: &str) -> bool {
    STANDARD.contains(&name)
}
