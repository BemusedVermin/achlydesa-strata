//! Movement over small boards: climbing, stop-on-enter, bridges, occupied spaces and caching.

use std::cell::Cell;
use std::collections::BTreeMap;

use proptest::prelude::*;
use strata_kernel::{DefId, Hex, Rotation, Shape, Space, Sym};
use strata_model::board::{BoardDefs, CompiledBoard, compile};
use strata_model::{
    FeatureDef, FigureId, MapConstraints, MapDef, PlacedPiece, SourceLoc, Text, TileDef,
};
use strata_rules::{LogicError, MoveWorld, Reach, TerrainRules};

use super::{Headroom, Limits, reachable};

const GRASS: DefId = DefId(1);
const MUD: DefId = DefId(2);

struct Defs(BTreeMap<Sym, TileDef>);

impl BoardDefs for Defs {
    fn tile(&self, name: &Sym) -> Option<&TileDef> {
        self.0.get(name)
    }
    fn feature(&self, _: &Sym) -> Option<&FeatureDef> {
        None
    }
}

fn tile(terrain: DefId) -> TileDef {
    TileDef {
        id: DefId(0),
        name: Text::default(),
        terrain,
        thickness: 1,
        shape: Shape::new([Hex::new(0, 0)]),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    }
}

/// A tile of grass, `g`, or mud, `m`, placed with its bottom at `level`.
fn piece(name: &str, q: i32, r: i32, level: u8) -> PlacedPiece {
    PlacedPiece {
        piece: name.into(),
        at: Hex::new(q, r),
        rot: Rotation::from_steps(0),
        level,
    }
}

fn board(pieces: Vec<PlacedPiece>) -> CompiledBoard {
    let tiles = [("g".into(), tile(GRASS)), ("m".into(), tile(MUD))];
    let map = MapDef {
        id: DefId(0),
        name: Text::default(),
        authors: Vec::new(),
        orientation: "pointy".into(),
        pieces,
        features: Vec::new(),
        zones: Default::default(),
        relic_spots: Default::default(),
        constraints: MapConstraints::default(),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    compile(&map, &Defs(BTreeMap::from(tiles))).unwrap()
}

struct Scene {
    board: CompiledBoard,
    figures: Vec<Space>,
    height: u32,
    allowance: i64,
    /// Entry cost by terrain.
    costs: BTreeMap<DefId, i64>,
    /// Rises of this many levels or more are refused by `can_climb`.
    climb_veto: i32,
    asked: Cell<usize>,
}

impl Scene {
    fn new(pieces: Vec<PlacedPiece>, figures: Vec<Space>) -> Self {
        Self {
            board: board(pieces),
            figures,
            height: 1,
            allowance: 20,
            costs: BTreeMap::new(),
            climb_veto: i32::MAX,
            asked: Cell::new(0),
        }
    }
}

impl MoveWorld for Scene {
    fn board(&self) -> &CompiledBoard {
        &self.board
    }
    fn location(&self, figure: FigureId) -> Option<Space> {
        self.figures.get(figure.index()).copied()
    }
    fn occupant(&self, space: Space) -> Option<FigureId> {
        let at = self.figures.iter().position(|f| *f == space);
        at.map(|i| FigureId(i as u32))
    }
    fn height(&self, _: FigureId) -> u32 {
        self.height
    }
    fn allowance(&self, _: FigureId) -> Result<i64, LogicError> {
        Ok(self.allowance)
    }
    fn terrain(&self, terrain: DefId) -> TerrainRules {
        TerrainRules {
            passable: true,
            stops_here: terrain == MUD,
        }
    }
    fn move_cost(&self, _: FigureId, terrain: DefId, _: i32) -> Result<i64, LogicError> {
        self.asked.set(self.asked.get() + 1);
        Ok(self.costs.get(&terrain).copied().unwrap_or(1))
    }
    fn can_climb(&self, _: FigureId, _: DefId, dz: i32) -> Result<bool, LogicError> {
        Ok(dz < self.climb_veto)
    }
    fn can_enter(&self, _: FigureId, _: Space) -> Result<bool, LogicError> {
        Ok(true)
    }
}

const LIMITS: Limits = Limits {
    headroom: Headroom::FigureHeight,
    max_climb: 1,
    max_fall: 2,
};

fn run(scene: &Scene, limits: Limits) -> Vec<Reach> {
    reachable(scene, FigureId(0), limits).unwrap()
}

fn find(reach: &[Reach], q: i32, z: u32) -> Option<&Reach> {
    reach.iter().find(|r| r.space == Space::new(q, 0, z))
}

/// A row of grass along `r = 0` for `q` in `0..len`, standing at `q = 0`.
fn row(len: i32) -> Scene {
    let pieces = (0..len).map(|q| piece("g", q, 0, 0)).collect();
    Scene::new(pieces, vec![Space::new(0, 0, 1)])
}

#[test]
fn a_flat_row_costs_one_per_step_and_keeps_its_path() {
    let reach = run(&row(4), LIMITS);
    let far = find(&reach, 3, 1).unwrap();
    assert_eq!(far.cost, 3);
    let path: Vec<_> = (1..=3).map(|q| Space::new(q, 0, 1)).collect();
    assert_eq!(far.path, path);
    assert!(
        find(&reach, 0, 1).is_none(),
        "the start is not a destination"
    );
}

#[test]
fn the_allowance_bounds_the_search() {
    let mut scene = row(6);
    scene.allowance = 2;
    let reach = run(&scene, LIMITS);
    assert_eq!(reach.len(), 2);
}

#[test]
fn climbing_is_limited_by_max_climb_and_can_climb() {
    // Level steps of one and two: the first is within the limit, the second is not.
    let pieces = vec![
        piece("g", 0, 0, 0),
        piece("g", 1, 0, 1),
        piece("g", 2, 0, 3),
    ];
    let mut scene = Scene::new(pieces, vec![Space::new(0, 0, 1)]);
    let reach = run(&scene, LIMITS);
    assert!(find(&reach, 1, 2).is_some());
    assert!(find(&reach, 2, 4).is_none());
    let wide = Limits {
        max_climb: 2,
        ..LIMITS
    };
    assert!(find(&run(&scene, wide), 2, 4).is_some());
    scene.climb_veto = 1;
    assert!(find(&run(&scene, wide), 1, 2).is_none());
}

#[test]
fn stop_on_enter_terrain_ends_the_move() {
    let pieces = vec![
        piece("g", 0, 0, 0),
        piece("m", 1, 0, 0),
        piece("g", 2, 0, 0),
    ];
    let scene = Scene::new(pieces, vec![Space::new(0, 0, 1)]);
    let reach = run(&scene, LIMITS);
    assert!(find(&reach, 1, 1).unwrap().stops_here);
    assert!(find(&reach, 2, 1).is_none());
}

/// Ground along `q = 0..=3`, with a deck at level 3 over `q = 2`.
fn bridge() -> Vec<PlacedPiece> {
    let mut pieces: Vec<_> = (0..=3).map(|q| piece("g", q, 0, 0)).collect();
    pieces.push(piece("g", 2, 0, 3));
    pieces
}

#[test]
fn a_figure_walks_under_a_bridge_only_if_it_fits() {
    let mut scene = Scene::new(bridge(), vec![Space::new(0, 0, 1)]);
    scene.height = 3;
    assert!(find(&run(&scene, LIMITS), 2, 1).is_none());
    scene.height = 2;
    assert!(find(&run(&scene, LIMITS), 3, 1).is_some());
    scene.height = 3;
    let ignore = Limits {
        headroom: Headroom::Ignore,
        ..LIMITS
    };
    assert!(find(&run(&scene, ignore), 3, 1).is_some());
}

#[test]
fn a_figure_on_the_deck_walks_off_and_falls() {
    let scene = Scene::new(bridge(), vec![Space::new(2, 0, 4)]);
    let reach = run(&scene, LIMITS);
    let down = find(&reach, 3, 1).unwrap();
    assert!(
        down.falls,
        "a drop of three levels exceeds the limit of two"
    );
    assert!(find(&reach, 4, 1).is_none());
    let steady = Limits {
        max_fall: 3,
        ..LIMITS
    };
    assert!(!find(&run(&scene, steady), 3, 1).unwrap().falls);
}

#[test]
fn the_deck_is_reached_from_the_ground_only_by_climbing() {
    let mut scene = Scene::new(bridge(), vec![Space::new(1, 0, 1)]);
    scene.height = 1;
    assert!(find(&run(&scene, LIMITS), 2, 4).is_none());
    let tall = Limits {
        max_climb: 3,
        ..LIMITS
    };
    assert!(find(&run(&scene, tall), 2, 4).is_some());
}

#[test]
fn occupied_spaces_are_flagged_and_passed_through() {
    let mut scene = row(3);
    scene.figures.push(Space::new(1, 0, 1));
    let reach = run(&scene, LIMITS);
    assert!(find(&reach, 1, 1).unwrap().occupied);
    let beyond = find(&reach, 2, 1).unwrap();
    assert!(!beyond.occupied);
    assert_eq!(beyond.cost, 2);
}

#[test]
fn costs_are_asked_once_per_terrain_and_level_change() {
    let scene = row(6);
    run(&scene, LIMITS);
    assert_eq!(scene.asked.get(), 1, "grass on the flat, asked once");
}

#[test]
fn results_do_not_depend_on_query_order_or_cache_state() {
    let mut scene = Scene::new(bridge(), vec![Space::new(0, 0, 1), Space::new(3, 0, 1)]);
    scene.costs.insert(MUD, 3);
    let first = run(&scene, LIMITS);
    // Ask for the other figure, then repeat; each query starts with a cold cache.
    let other = reachable(&scene, FigureId(1), LIMITS).unwrap();
    assert!(!other.is_empty());
    assert_eq!(run(&scene, LIMITS), first);
}

#[test]
fn equal_cost_routes_break_ties_by_space_order() {
    // Two routes of equal cost round a hole at (1, 0): through (0, 1) or through (1, -1).
    let pieces = [(0, 0), (0, 1), (1, -1), (1, 1)]
        .iter()
        .map(|&(q, r)| piece("g", q, r, 0))
        .collect();
    let scene = Scene::new(pieces, vec![Space::new(0, 0, 1)]);
    let a = run(&scene, LIMITS);
    let b = run(&scene, LIMITS);
    assert_eq!(a, b);
    let end = a.iter().find(|r| r.space == Space::new(1, 1, 1)).unwrap();
    assert_eq!(end.cost, 2);
    assert_eq!(end.path[0], Space::new(0, 1, 1));
}

proptest! {
    #[test]
    fn path_cost_is_never_less_than_hex_distance(
        costs in proptest::collection::vec(1i64..5, 2),
        holes in proptest::collection::btree_set((-3i32..=3, -3i32..=3), 0..8),
    ) {
        let pieces = Hex::ORIGIN.spiral(3).into_iter()
            .filter(|h| !holes.contains(&(h.q, h.r)) || *h == Hex::ORIGIN)
            .map(|h| piece(if (h.q + h.r) % 2 == 0 { "g" } else { "m" }, h.q, h.r, 0))
            .collect();
        let mut scene = Scene::new(pieces, vec![Space::new(0, 0, 1)]);
        scene.costs = BTreeMap::from([(GRASS, costs[0]), (MUD, costs[1])]);
        for r in run(&scene, LIMITS) {
            let distance = Hex::ORIGIN.distance(r.space.hex());
            prop_assert!(r.cost >= i64::from(distance));
            prop_assert_eq!(r.path.len() as u32 >= distance, true);
            prop_assert_eq!(r.path.last().copied(), Some(r.space));
        }
    }
}
