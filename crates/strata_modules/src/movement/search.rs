//! Dijkstra over (space, rotation) nodes, with edge answers cached for one query (design §7.4).

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use strata_kernel::{DefId, Hex, Rotation, Space};
use strata_model::{FigureId, FootprintDef};
use strata_rules::{LogicError, MoveWorld, Reach, TerrainRules};

use super::body::{Body, roomy};

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
pub(super) struct Cache<'a> {
    pub(super) world: &'a dyn MoveWorld,
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

    pub(super) fn enters(&mut self, to: Space) -> Result<bool, LogicError> {
        if let Some(&hit) = self.enters.get(&to) {
            return Ok(hit);
        }
        let allowed = self.world.can_enter(self.figure, to)?;
        self.enters.insert(to, allowed);
        Ok(allowed)
    }

    pub(super) fn terrain(&mut self, terrain: DefId) -> TerrainRules {
        let world = self.world;
        *self
            .terrain
            .entry(terrain)
            .or_insert_with(|| world.terrain(terrain))
    }
}

/// A search node: the anchor space and the rotation.
type Key = (Space, Rotation);

/// A node found by the search, with how it was reached.
struct Node {
    cost: i64,
    from: Option<Key>,
    stops_here: bool,
    falls: bool,
    provokes_reprisal: bool,
    /// The spaces the footprint covers here.
    cells: Vec<Space>,
}

/// One legal step onto a surface.
struct Step {
    to: Space,
    cost: i64,
    stops_here: bool,
    falls: bool,
}

/// Every (space, rotation) `figure` can reach, ordered by space and then rotation.
///
/// Nodes are (anchor space, rotation) pairs. A step moves the anchor to a surface of an adjacent
/// column, priced and checked as a one-space step; the footprint's move mode then gives the
/// rotations it may end with, and every cell must stand there (see [`Body`]). Ties between
/// equal-cost routes go to whichever node settles first, which orders by cost, then space, then
/// rotation, so the answer never depends on the cache or on query order.
pub fn reachable(
    world: &dyn MoveWorld,
    figure: FigureId,
    limits: Limits,
) -> Result<Vec<Reach>, LogicError> {
    search(world, figure, limits, Body::new(world.footprint(figure)))
}

/// [`reachable`] for `figure` moving as `body`.
pub(super) fn search(
    world: &dyn MoveWorld,
    figure: FigureId,
    limits: Limits,
    body: Body<'_>,
) -> Result<Vec<Reach>, LogicError> {
    let Some(anchor) = world.location(figure) else {
        return Err(LogicError(format!(
            "figure {} is not on the board",
            figure.0
        )));
    };
    let start = (anchor, world.rotation(figure));
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
    let cells = match body {
        Body::One => vec![anchor],
        // Where the figure stands now, even if no legal placement is left under it.
        Body::Shaped(def) => body_spaces(world, def, start).unwrap_or_else(|| {
            let hexes = def.cells_at(anchor.hex(), start.1).into_iter();
            hexes.map(|hex| Space::at(hex, anchor.z)).collect()
        }),
    };
    let origin = Node {
        cost: 0,
        from: None,
        stops_here: false,
        falls: false,
        provokes_reprisal: false,
        cells,
    };
    let mut found = BTreeMap::from([(start, origin)]);
    let mut settled = BTreeSet::new();
    let mut heap = BinaryHeap::from([Reverse((0i64, start))]);
    while let Some(Reverse((cost, at))) = heap.pop() {
        if !settled.insert(at) || found[&at].stops_here || found[&at].falls {
            continue;
        }
        for hex in at.0.hex().neighbors() {
            for step in steps_into(&mut cache, limits, height, at.0, hex)? {
                let total = cost + step.cost;
                if total > allowance {
                    continue;
                }
                for rotation in body.turns(at, hex) {
                    let key = (step.to, rotation);
                    let better = found.get(&key).is_none_or(|n| total < n.cost);
                    if !better || settled.contains(&key) {
                        continue;
                    }
                    let placed = body.place(&mut cache, limits, height, step.to, rotation)?;
                    let Some(placed) = placed else {
                        continue;
                    };
                    // Asked only for steps that improve a route, so each edge is asked at most
                    // once.
                    let provokes = found[&at].provokes_reprisal
                        || leaves_engagement(world, figure, &found[&at].cells, &placed.cells)?;
                    let node = Node {
                        cost: total,
                        from: Some(at),
                        stops_here: step.stops_here || placed.stops_here,
                        falls: step.falls,
                        provokes_reprisal: provokes,
                        cells: placed.cells,
                    };
                    found.insert(key, node);
                    heap.push(Reverse((total, key)));
                }
            }
        }
    }
    found.remove(&start);
    let reach = |(&key, node): (&Key, &Node)| {
        let mut path = vec![key.0];
        let mut back = node.from;
        while let Some(prev) = back.filter(|p| *p != start) {
            path.push(prev.0);
            back = found[&prev].from;
        }
        path.reverse();
        let covered_by_other = |s: &Space| world.occupant(*s).is_some_and(|o| o != figure);
        Reach {
            space: key.0,
            rotation: key.1,
            cells: node.cells.clone(),
            cost: node.cost,
            path,
            stops_here: node.stops_here,
            falls: node.falls,
            provokes_reprisal: node.provokes_reprisal,
            occupied: node.cells.iter().any(covered_by_other),
        }
    };
    Ok(found.iter().map(reach).collect())
}

/// Where a footprint at `at` stands, if it can stand there at all.
fn body_spaces(world: &dyn MoveWorld, def: &FootprintDef, at: Key) -> Option<Vec<Space>> {
    let spaces = def.spaces(world.board(), at.0, at.1)?;
    Some(spaces.into_iter().map(|(space, _)| space).collect())
}

/// Whether the body leaves engagement with some enemy as it moves from `before` to `after`.
///
/// A one-cell figure asks [`MoveWorld::provokes_reprisal`] once about its step. A larger body
/// asks [`MoveWorld::body_provokes_reprisal`] about all its cells together: a cell stepping away
/// from an enemy does not leave it while another cell stays engaged with it.
fn leaves_engagement(
    world: &dyn MoveWorld,
    figure: FigureId,
    before: &[Space],
    after: &[Space],
) -> Result<bool, LogicError> {
    match (before, after) {
        ([from], [to]) => world.provokes_reprisal(figure, *from, *to),
        _ => world.body_provokes_reprisal(figure, before, after),
    }
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
        let roomy = roomy(limits, height, &surface);
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
