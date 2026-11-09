//! Multi-space footprints: snake turns, rigid translation, level spread, occupancy and
//! engagement, plus the one-space search staying exactly as it was.

use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use strata_kernel::{DefId, Hex, Rotation, Shape, Space, Sym};
use strata_model::board::{BoardDefs, CompiledBoard, compile};
use strata_model::{
    FeatureDef, FigureId, FootprintDef, MapConstraints, MapDef, MoveMode, PlacedPiece, SourceLoc,
    Text, TileDef,
};
use strata_rules::{LogicError, MoveWorld, Reach, TerrainRules};

use super::{Headroom, Limits, reachable};

const GRASS: DefId = DefId(1);
const MUD: DefId = DefId(2);

const LIMITS: Limits = Limits {
    headroom: Headroom::FigureHeight,
    max_climb: 1,
    max_fall: 2,
};

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

/// One tile per `(q, r, level, mud)`.
fn board(pieces: &[(i32, i32, u8, bool)]) -> CompiledBoard {
    let placed = pieces.iter().map(|&(q, r, level, mud)| PlacedPiece {
        piece: if mud { "m" } else { "g" }.into(),
        at: Hex::new(q, r),
        rot: Rotation::default(),
        level,
    });
    let map = MapDef {
        id: DefId(0),
        name: Text::default(),
        authors: Vec::new(),
        orientation: "pointy".into(),
        pieces: placed.collect(),
        features: Vec::new(),
        zones: Default::default(),
        relic_spots: Default::default(),
        constraints: MapConstraints::default(),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    let tiles = [("g".into(), tile(GRASS)), ("m".into(), tile(MUD))];
    compile(&map, &Defs(BTreeMap::from(tiles))).unwrap()
}

/// Flat grass over every hex within `radius` of the origin.
fn flat(radius: u32) -> CompiledBoard {
    let hexes = Hex::ORIGIN.spiral(radius).into_iter();
    board(&hexes.map(|h| (h.q, h.r, 0, false)).collect::<Vec<_>>())
}

fn footprint(cells: &[(i32, i32)], move_mode: MoveMode, max_spread: u8) -> FootprintDef {
    FootprintDef {
        id: DefId(0),
        name: Text::default(),
        cells: Shape::new(cells.iter().map(|&(q, r)| Hex::new(q, r))),
        move_mode,
        max_spread,
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    }
}

/// A figure: anchor, rotation and footprint.
struct Figure {
    at: Space,
    rotation: Rotation,
    footprint: Option<FootprintDef>,
}

struct Scene {
    board: CompiledBoard,
    figures: Vec<Figure>,
    allowance: i64,
    /// Steps out of this space provoke a reprisal.
    engaged_on: Option<Space>,
}

impl Scene {
    fn new(board: CompiledBoard, footprint: Option<FootprintDef>) -> Self {
        let mover = Figure {
            at: Space::new(0, 0, 1),
            rotation: Rotation::default(),
            footprint,
        };
        Self {
            board,
            figures: vec![mover],
            allowance: 1,
            engaged_on: None,
        }
    }

    fn cells(&self, figure: &Figure) -> Vec<Space> {
        match &figure.footprint {
            Some(def) => def
                .spaces(&self.board, figure.at, figure.rotation)
                .map_or_else(
                    || vec![figure.at],
                    |s| s.into_iter().map(|(space, _)| space).collect(),
                ),
            None => vec![figure.at],
        }
    }
}

impl MoveWorld for Scene {
    fn board(&self) -> &CompiledBoard {
        &self.board
    }
    fn location(&self, figure: FigureId) -> Option<Space> {
        self.figures.get(figure.index()).map(|f| f.at)
    }
    fn rotation(&self, figure: FigureId) -> Rotation {
        self.figures[figure.index()].rotation
    }
    fn footprint(&self, figure: FigureId) -> Option<&FootprintDef> {
        self.figures[figure.index()].footprint.as_ref()
    }
    fn occupant(&self, space: Space) -> Option<FigureId> {
        let at = self
            .figures
            .iter()
            .position(|f| self.cells(f).contains(&space));
        at.map(|i| FigureId(i as u32))
    }
    fn height(&self, _: FigureId) -> u32 {
        1
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
    fn move_cost(&self, _: FigureId, _: DefId, _: i32) -> Result<i64, LogicError> {
        Ok(1)
    }
    fn can_climb(&self, _: FigureId, _: DefId, _: i32) -> Result<bool, LogicError> {
        Ok(true)
    }
    fn can_enter(&self, _: FigureId, _: Space) -> Result<bool, LogicError> {
        Ok(true)
    }
    fn provokes_reprisal(&self, _: FigureId, from: Space, _: Space) -> Result<bool, LogicError> {
        Ok(self.engaged_on == Some(from))
    }
}

fn run(scene: &Scene) -> Vec<Reach> {
    reachable(scene, FigureId(0), LIMITS).unwrap()
}

/// The rotations the figure can end in at `(q, r)`.
fn turns(reach: &[Reach], q: i32, r: i32) -> Vec<u8> {
    let here = reach.iter().filter(|x| x.space.hex() == Hex::new(q, r));
    here.map(|x| x.rotation.steps()).collect()
}

const TWO: [(i32, i32); 2] = [(0, 0), (1, 0)];

#[test]
fn a_snake_turns_so_its_rear_follows_the_leader() {
    let scene = Scene::new(flat(3), Some(footprint(&TWO, MoveMode::Snake, 1)));
    let reach = run(&scene);
    // Facing east-to-west (rear at E): stepping SE swings the rear to NW, onto the old anchor.
    assert_eq!(turns(&reach, 0, 1), [4]);
    assert_eq!(turns(&reach, -1, 0), [0]);
    // Stepping onto its own rear reverses the figure.
    assert_eq!(turns(&reach, 1, 0), [3]);
    for step in &reach {
        assert!(step.cells.contains(&Space::new(0, 0, 1)), "{step:?}");
    }
}

#[test]
fn a_rigid_shape_only_translates() {
    let scene = Scene::new(flat(3), Some(footprint(&TWO, MoveMode::Rigid, 1)));
    let reach = run(&scene);
    assert!(reach.iter().all(|r| r.rotation == Rotation::default()));
    let se = reach
        .iter()
        .find(|r| r.space == Space::new(0, 1, 1))
        .unwrap();
    assert_eq!(se.cells, [Space::new(0, 1, 1), Space::new(1, 1, 1)]);
    // Stepping east would put the anchor on its own rear: legal, the shape slides along.
    let east = reach
        .iter()
        .find(|r| r.space == Space::new(1, 0, 1))
        .unwrap();
    assert_eq!(east.cells, [Space::new(1, 0, 1), Space::new(2, 0, 1)]);
}

#[test]
fn a_snake_walks_round_a_corner_over_several_steps() {
    // An L-shaped corridor: east along r = 0, then south-east down q = 2.
    let pieces = [(0, 0), (1, 0), (2, 0), (2, 1), (2, 2), (2, 3)];
    let board = board(&pieces.map(|(q, r)| (q, r, 0, false)));
    let mut scene = Scene::new(board, Some(footprint(&TWO, MoveMode::Snake, 1)));
    // Start facing west-to-east: anchor at (1, 0), rear at (0, 0).
    scene.figures[0].at = Space::new(1, 0, 1);
    scene.figures[0].rotation = Rotation::new(3).unwrap();
    scene.allowance = 4;
    let reach = run(&scene);
    let end = reach
        .iter()
        .find(|r| r.space == Space::new(2, 3, 1))
        .unwrap();
    assert_eq!(end.cells, [Space::new(2, 3, 1), Space::new(2, 2, 1)]);
    assert_eq!(end.cost, 4);
    // A rigid shape cannot: its rear would leave the corridor at the turn.
    scene.figures[0].footprint = Some(footprint(&TWO, MoveMode::Rigid, 1));
    assert!(run(&scene).iter().all(|r| r.space.r == 0));
}

#[test]
fn the_cells_may_not_spread_wider_than_the_footprint_allows() {
    // A one-level step at q = 1.
    let board = board(&[
        (-1, 0, 0, false),
        (0, 0, 0, false),
        (1, 0, 1, false),
        (2, 0, 1, false),
    ]);
    let mut scene = Scene::new(board, Some(footprint(&TWO, MoveMode::Rigid, 0)));
    scene.figures[0].at = Space::new(-1, 0, 1);
    scene.allowance = 3;
    assert!(
        run(&scene).is_empty(),
        "straddling the step needs a spread of one"
    );
    scene.figures[0].footprint = Some(footprint(&TWO, MoveMode::Rigid, 1));
    let reach = run(&scene);
    let up = reach
        .iter()
        .find(|r| r.space == Space::new(1, 0, 2))
        .unwrap();
    assert_eq!(up.cells, [Space::new(1, 0, 2), Space::new(2, 0, 2)]);
}

#[test]
fn a_rear_cell_on_stop_terrain_ends_the_move() {
    let board = board(&[
        (0, 0, 0, false),
        (1, 0, 0, false),
        (2, 0, 0, true),
        (3, 0, 0, false),
    ]);
    let mut scene = Scene::new(board, Some(footprint(&TWO, MoveMode::Rigid, 1)));
    scene.allowance = 3;
    let reach = run(&scene);
    assert!(
        reach
            .iter()
            .find(|r| r.space == Space::new(1, 0, 1))
            .unwrap()
            .stops_here
    );
    assert!(reach.iter().all(|r| r.space.q <= 1));
}

#[test]
fn another_figures_rear_cell_is_occupied() {
    let mut scene = Scene::new(flat(3), Some(footprint(&TWO, MoveMode::Rigid, 1)));
    scene.figures.push(Figure {
        at: Space::new(-2, 1, 1),
        rotation: Rotation::default(),
        footprint: Some(footprint(&TWO, MoveMode::Rigid, 1)),
    });
    let reach = run(&scene);
    // Moving SW puts the mover's rear on (0, 1), its anchor on (-1, 1): the other's rear.
    assert!(
        reach
            .iter()
            .find(|r| r.space == Space::new(-1, 1, 1))
            .unwrap()
            .occupied
    );
    assert!(
        !reach
            .iter()
            .find(|r| r.space == Space::new(-1, 0, 1))
            .unwrap()
            .occupied
    );
}

#[test]
fn any_cell_leaving_engagement_provokes_a_reprisal() {
    let mut scene = Scene::new(flat(3), Some(footprint(&TWO, MoveMode::Rigid, 1)));
    scene.engaged_on = Some(Space::new(1, 0, 1));
    assert!(run(&scene).iter().all(|r| r.provokes_reprisal));
    scene.engaged_on = Some(Space::new(5, 5, 1));
    assert!(run(&scene).iter().all(|r| !r.provokes_reprisal));
}

#[test]
fn a_one_cell_footprint_moves_exactly_like_no_footprint() {
    let pieces = [
        (0, 0, 0, false),
        (1, 0, 1, false),
        (2, 0, 0, true),
        (0, 1, 3, false),
    ];
    let mut bare = Scene::new(board(&pieces), None);
    bare.allowance = 4;
    let mut one = Scene::new(
        board(&pieces),
        Some(footprint(&[(0, 0)], MoveMode::Snake, 0)),
    );
    one.allowance = 4;
    assert_eq!(run(&bare), run(&one));
}

/// A ragged board: random levels, holes and mud over a radius-3 patch.
fn ragged(levels: &[u8], holes: &BTreeSet<usize>, mud: &BTreeSet<usize>) -> CompiledBoard {
    let hexes = Hex::ORIGIN.spiral(3).into_iter().enumerate();
    let kept = hexes.filter(|(i, _)| *i == 0 || !holes.contains(i));
    let pieces: Vec<_> = kept
        .map(|(i, h)| (h.q, h.r, levels[i % levels.len()], mud.contains(&i)))
        .collect();
    board(&pieces)
}

proptest! {
    #[test]
    fn no_reachable_placement_leaves_a_cell_over_empty_space(
        levels in proptest::collection::vec(0u8..3, 37),
        holes in proptest::collection::btree_set(1usize..37, 0..12),
        mud in proptest::collection::btree_set(1usize..37, 0..4),
        snake in any::<bool>(),
        spread in 0u8..3,
        rotation in 0u8..6,
    ) {
        let board = ragged(&levels, &holes, &mud);
        let mode = if snake { MoveMode::Snake } else { MoveMode::Rigid };
        let mut scene = Scene::new(board, Some(footprint(&TWO, mode, spread)));
        scene.figures[0].at = Space::new(0, 0, u32::from(levels[0]) + 1);
        scene.figures[0].rotation = Rotation::new(rotation).unwrap();
        scene.allowance = 4;
        let reach = run(&scene);
        for r in &reach {
            prop_assert_eq!(r.cells.len(), 2);
            for cell in &r.cells {
                let surfaces = scene.board.surfaces(cell.hex());
                prop_assert!(surfaces.iter().any(|s| s.z == cell.z), "{:?} floats", cell);
            }
            let low = r.cells.iter().map(|c| c.z).min().unwrap();
            let high = r.cells.iter().map(|c| c.z).max().unwrap();
            prop_assert!(high - low <= u32::from(spread));
            if !snake {
                prop_assert_eq!(r.rotation, Rotation::new(rotation).unwrap());
            }
        }
        prop_assert_eq!(run(&scene), reach);
    }

    #[test]
    fn a_one_cell_footprint_never_changes_the_answer(
        levels in proptest::collection::vec(0u8..3, 37),
        holes in proptest::collection::btree_set(1usize..37, 0..12),
        mud in proptest::collection::btree_set(1usize..37, 0..4),
    ) {
        let mut bare = Scene::new(ragged(&levels, &holes, &mud), None);
        bare.figures[0].at = Space::new(0, 0, u32::from(levels[0]) + 1);
        bare.allowance = 4;
        let mut one = Scene::new(ragged(&levels, &holes, &mud), None);
        one.figures[0].at = bare.figures[0].at;
        one.figures[0].footprint = Some(footprint(&[(0, 0)], MoveMode::Rigid, 0));
        one.allowance = 4;
        prop_assert_eq!(run(&bare), run(&one));
    }
}
