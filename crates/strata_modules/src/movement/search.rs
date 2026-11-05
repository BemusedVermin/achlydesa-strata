//! Dijkstra over surfaces, with edge answers cached for one query (design §7.4).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use strata_kernel::{DefId, Hex, Space};
use strata_model::FigureId;
use strata_rules::{LogicError, MoveWorld, Reach, TerrainRules};

/// How headroom under an overhang is checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Headroom {
    /// Overhangs never stop a figure.
    Ignore,
    /// The free levels above a surface must be at least the figure's height.
    FigureHeight,
}

/// The module's settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// How headroom is checked.
    pub headroom: Headroom,
    /// The highest rise, in levels, one step may climb.
    pub max_climb: u32,
    /// The deepest drop, in levels, a step may descend without falling.
    pub max_fall: u32,
}

/// The answers one query has already asked of the rules.
struct Cache<'a> {
    world: &'a dyn MoveWorld,
    figure: FigureId,
    costs: BTreeMap<(DefId, i32), i64>,
    climbs: BTreeMap<(DefId, i32), bool>,
    enters: BTreeMap<Space, bool>,
    terrain: BTreeMap<DefId, TerrainRules>,
}

impl Cache<'_> {
    fn cost(&mut self, terrain: DefId, dz: i32) -> Result<i64, LogicError> {
        if let Some(&hit) = self.costs.get(&(terrain, dz)) {
            return Ok(hit);
        }
        // A step always costs something, so no path is cheaper than its length.
        let cost = self.world.move_cost(self.figure, terrain, dz)?.max(1);
        self.costs.insert((terrain, dz), cost);
        Ok(cost)
    }

    fn climbs(&mut self, terrain: DefId, dz: i32) -> Result<bool, LogicError> {
        if let Some(&hit) = self.climbs.get(&(terrain, dz)) {
            return Ok(hit);
        }
        let allowed = self.world.can_climb(self.figure, terrain, dz)?;
        self.climbs.insert((terrain, dz), allowed);
        Ok(allowed)
    }

    fn enters(&mut self, to: Space) -> Result<bool, LogicError> {
        if let Some(&hit) = self.enters.get(&to) {
            return Ok(hit);
        }
        let allowed = self.world.can_enter(self.figure, to)?;
        self.enters.insert(to, allowed);
        Ok(allowed)
    }

    fn terrain(&mut self, terrain: DefId) -> TerrainRules {
        let world = self.world;
        *self
            .terrain
            .entry(terrain)
            .or_insert_with(|| world.terrain(terrain))
    }
}

/// A space found by the search, with how it was reached.
struct Node {
    cost: i64,
    from: Option<Space>,
    stops_here: bool,
    falls: bool,
}

/// One legal step onto a surface.
struct Step {
    to: Space,
    cost: i64,
    stops_here: bool,
    falls: bool,
}

/// Every space `figure` can reach, ordered by [`Space`].
///
/// Ties between equal-cost routes go to whichever space settles first, which orders by cost and
/// then [`Space`], so the answer never depends on the cache or on query order.
pub fn reachable(
    world: &dyn MoveWorld,
    figure: FigureId,
    limits: Limits,
) -> Result<Vec<Reach>, LogicError> {
    let Some(start) = world.location(figure) else {
        return Err(LogicError(format!(
            "figure {} is not on the board",
            figure.0
        )));
    };
    let allowance = world.allowance(figure)?;
    let height = world.height(figure);
    let mut cache = Cache {
        world,
        figure,
        costs: BTreeMap::new(),
        climbs: BTreeMap::new(),
        enters: BTreeMap::new(),
        terrain: BTreeMap::new(),
    };
    let origin = Node {
        cost: 0,
        from: None,
        stops_here: false,
        falls: false,
    };
    let mut found = BTreeMap::from([(start, origin)]);
    let mut settled = BTreeSet::new();
    let mut heap = BinaryHeap::from([Reverse((0i64, start))]);
    while let Some(Reverse((cost, at))) = heap.pop() {
        if !settled.insert(at) || found[&at].stops_here || found[&at].falls {
            continue;
        }
        for hex in at.hex().neighbors() {
            for step in steps_into(&mut cache, limits, height, at, hex)? {
                let total = cost + step.cost;
                let better = found.get(&step.to).is_none_or(|n| total < n.cost);
                if total > allowance || !better || settled.contains(&step.to) {
                    continue;
                }
                let node = Node {
                    cost: total,
                    from: Some(at),
                    stops_here: step.stops_here,
                    falls: step.falls,
                };
                found.insert(step.to, node);
                heap.push(Reverse((total, step.to)));
            }
        }
    }
    found.remove(&start);
    let reach = |(&space, node): (&Space, &Node)| {
        let mut path = vec![space];
        let mut back = node.from;
        while let Some(prev) = back.filter(|p| *p != start) {
            path.push(prev);
            back = found[&prev].from;
        }
        path.reverse();
        Reach {
            space,
            cost: node.cost,
            path,
            stops_here: node.stops_here,
            falls: node.falls,
            occupied: world.occupant(space).is_some_and(|o| o != figure),
        }
    };
    Ok(found.iter().map(reach).collect())
}

/// The legal steps from `at` onto the surfaces of `hex`.
fn steps_into(
    cache: &mut Cache<'_>,
    limits: Limits,
    height: u32,
    at: Space,
    hex: Hex,
) -> Result<Vec<Step>, LogicError> {
    let mut steps = Vec::new();
    let surfaces = cache.world.board().surfaces(hex).to_vec();
    for surface in surfaces {
        let rules = cache.terrain(surface.terrain);
        let roomy = match (limits.headroom, surface.headroom) {
            (Headroom::FigureHeight, Some(free)) => free >= height,
            _ => true,
        };
        let dz = i32::try_from(i64::from(surface.z) - i64::from(at.z)).unwrap_or(i32::MAX);
        let drop = dz.unsigned_abs();
        let in_reach = dz < 0 || drop <= limits.max_climb;
        if !rules.passable || !roomy || !in_reach {
            continue;
        }
        let to = Space::at(hex, surface.z);
        if dz > 0 && !cache.climbs(surface.terrain, dz)? || !cache.enters(to)? {
            continue;
        }
        steps.push(Step {
            to,
            cost: cache.cost(surface.terrain, dz)?,
            stops_here: rules.stops_here,
            falls: dz < 0 && drop > limits.max_fall,
        });
    }
    Ok(steps)
}
