//! Line of sight over small boards: walls, a bridge, cover, figures and rotation.

use std::collections::BTreeMap;

use proptest::prelude::*;
use strata_kernel::{Blocker, DefId, Hex, Rotation, Shape, Space, Sym, Value};
use strata_model::board::{BoardDefs, CompiledBoard, compile};
use strata_model::{
    FeatureCell, FeatureDef, FigureId, MapConstraints, MapDef, PlacedFeature, PlacedPiece, Segment,
    SourceLoc, Text, TileDef,
};
use strata_rules::{
    FigureVolume, LineOfSight, LosResult, LosWorld, Registrar, RuleModule, VolumeCell,
};

use super::{Los, Policy, Service, Threshold};

const GRASS: DefId = DefId(1);

struct Defs {
    tiles: BTreeMap<Sym, TileDef>,
    features: BTreeMap<Sym, FeatureDef>,
}

impl BoardDefs for Defs {
    fn tile(&self, name: &Sym) -> Option<&TileDef> {
        self.tiles.get(name)
    }
    fn feature(&self, name: &Sym) -> Option<&FeatureDef> {
        self.features.get(name)
    }
}

fn defs() -> Defs {
    let tile = TileDef {
        id: DefId(0),
        name: Text::default(),
        terrain: GRASS,
        thickness: 1,
        shape: Shape::new([Hex::new(0, 0)]),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    let wall = |height: u8| FeatureDef {
        id: DefId(0),
        name: Text::default(),
        blocks_los: true,
        cells: vec![FeatureCell {
            at: Hex::new(0, 0),
            segments: vec![Segment {
                from: 0,
                to: height,
                top: true,
            }],
        }],
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    Defs {
        tiles: BTreeMap::from([("g".into(), tile)]),
        features: (1..=3)
            .map(|h| (Sym::new(&format!("wall{h}")), wall(h)))
            .collect(),
    }
}

#[derive(Clone, Debug)]
struct Fig {
    at: Hex,
    /// The level the figure stands on.
    z: i64,
    target: i64,
    hit: (i64, i64),
    /// A second footprint cell and the level it stands on, for a two-space figure.
    rear: Option<(Hex, i64)>,
}

/// A short figure on row 0 whose eye is `target` sub-levels above its feet.
fn fig(q: i32, z: i64, target: i64) -> Fig {
    Fig {
        at: Hex::new(q, 0),
        z,
        target,
        hit: (0, 12),
        rear: None,
    }
}

struct Scene {
    board: CompiledBoard,
    figs: Vec<Fig>,
}

impl LosWorld for Scene {
    fn blockers(&self, hex: Hex) -> &[Blocker] {
        self.board.blockers(hex)
    }
    fn figures(&self) -> Vec<FigureId> {
        (0..self.figs.len() as u32).map(FigureId).collect()
    }
    fn volume(&self, figure: FigureId) -> Option<FigureVolume> {
        let f = self.figs.get(figure.index())?;
        let cell = |(at, z): (Hex, i64)| VolumeCell {
            space: Space::new(at.q, at.r, z as u32),
            bottom: z * 12 + f.hit.0,
            top: z * 12 + f.hit.1,
        };
        let cells = std::iter::once((f.at, f.z)).chain(f.rear);
        Some(FigureVolume {
            anchor: f.at,
            target: f.z * 12 + f.target,
            cells: cells.map(cell).collect(),
        })
    }
}

fn piece(at: Hex, level: u8) -> PlacedPiece {
    PlacedPiece {
        piece: "g".into(),
        at,
        rot: Rotation::from_steps(0),
        level,
    }
}

fn wall(at: Hex, height: u8) -> PlacedFeature {
    PlacedFeature {
        feature: Sym::new(&format!("wall{height}")),
        at,
        rot: Rotation::from_steps(0),
    }
}

fn scene(pieces: Vec<PlacedPiece>, features: Vec<PlacedFeature>, figs: Vec<Fig>) -> Scene {
    let map = MapDef {
        id: DefId(0),
        name: Text::default(),
        authors: Vec::new(),
        orientation: "pointy".into(),
        pieces,
        features,
        zones: Default::default(),
        relic_spots: Default::default(),
        constraints: MapConstraints::default(),
        tags: Default::default(),
        meta: Default::default(),
        source: SourceLoc::default(),
    };
    let board = compile(&map, &defs()).unwrap();
    Scene { board, figs }
}

fn ground() -> Vec<PlacedPiece> {
    (0..=6).map(|q| piece(Hex::new(q, 0), 0)).collect()
}

/// A row of ground from `q = 0` to `6` with `features` on it and figures at their feet.
fn row(features: Vec<PlacedFeature>, figs: Vec<Fig>) -> Scene {
    scene(ground(), features, figs)
}

fn ask(policy: Policy, threshold: Threshold, scene: &Scene) -> LosResult {
    let service = Service { policy, threshold };
    service.query(scene, FigureId(0), FigureId(1)).unwrap()
}

fn geometric(scene: &Scene) -> LosResult {
    ask(Policy::Geometric, Threshold::Any, scene)
}

fn fraction(result: &LosResult) -> (i128, i128) {
    (result.clear_fraction.numer(), result.clear_fraction.denom())
}

#[test]
fn open_ground_is_fully_visible_with_21_samples() {
    let open = row(vec![], vec![fig(0, 1, 10), fig(6, 1, 10)]);
    let result = geometric(&open);
    assert!(result.visible);
    assert_eq!(result.witnesses.len(), 21);
    assert_eq!(fraction(&result), (1, 1));
}

#[test]
fn a_wall_taller_than_every_sample_blocks_sight() {
    let figs = vec![fig(0, 1, 10), fig(6, 1, 10)];
    let walled = row(vec![wall(Hex::new(3, 0), 2)], figs);
    let result = geometric(&walled);
    assert!(!result.visible);
    assert!(result.witnesses.is_empty());
    assert_eq!(fraction(&result), (0, 1));
}

#[test]
fn a_head_over_a_low_wall_is_partly_covered() {
    // The wall is one level (12 sub-levels) high; the attacker's eye is 8 above it.
    let figs = vec![fig(0, 1, 20), fig(6, 1, 10)];
    let low = row(vec![wall(Hex::new(3, 0), 1)], figs);
    let result = geometric(&low);
    assert!(result.visible);
    assert!(result.witnesses.len() < 21 && !result.witnesses.is_empty());
    // Requiring every sample is stricter than `any`; `half` follows the count.
    assert!(!ask(Policy::Geometric, Threshold::All, &low).visible);
    let half = ask(Policy::Geometric, Threshold::Half, &low);
    assert_eq!(half.visible, 2 * half.witnesses.len() >= 21);
}

#[test]
fn touching_the_top_of_a_wall_does_not_block() {
    // The eye is level with the wall top, and the defender's whole volume is above it.
    let mut high = row(
        vec![wall(Hex::new(3, 0), 1)],
        vec![fig(0, 1, 12), fig(6, 1, 12)],
    );
    high.figs[1].hit = (12, 24);
    assert!(geometric(&high).visible);
}

#[test]
fn an_intervening_figure_blocks_and_leaving_unblocks() {
    let mut crowd = row(vec![], vec![fig(0, 1, 10), fig(6, 1, 10), fig(3, 1, 10)]);
    assert!(!geometric(&crowd).visible);
    crowd.figs.pop();
    assert!(geometric(&crowd).visible);
}

#[test]
fn a_tall_figure_beside_the_line_does_not_block() {
    let mut pieces = ground();
    pieces.push(piece(Hex::new(3, 2), 0));
    let mut aside = scene(pieces, vec![], vec![fig(0, 1, 10), fig(6, 1, 10)]);
    aside.figs.push(Fig {
        at: Hex::new(3, 2),
        z: 1,
        target: 10,
        hit: (0, 36),
        rear: None,
    });
    assert!(geometric(&aside).visible);
}

#[test]
fn a_bridge_deck_above_eye_height_does_not_block_a_view_beneath_it() {
    let mut pieces = ground();
    pieces.push(piece(Hex::new(3, 0), 4));
    let under = scene(pieces.clone(), vec![], vec![fig(0, 1, 10), fig(6, 1, 10)]);
    assert!(geometric(&under).visible);
    // A figure on the deck still sees the ground figure past the bridge, but a tall
    // wall beside the target hides it even from the deck.
    let figs = vec![fig(3, 5, 10), fig(6, 1, 10)];
    assert!(geometric(&scene(pieces.clone(), vec![], figs.clone())).visible);
    let hidden = scene(pieces, vec![wall(Hex::new(5, 0), 3)], figs);
    assert!(!geometric(&hidden).visible);
}

/// Ground from `q = 0` to `6`, with `raised` one level higher than the rest.
fn stepped(raised: Hex) -> Vec<PlacedPiece> {
    let ground = ground().into_iter().filter(|p| p.at != raised);
    ground.chain([piece(raised, 1)]).collect()
}

#[test]
fn a_rear_cell_a_level_up_is_sampled_at_its_own_level() {
    // A one-level wall hides everything on the defender's anchor level. The defender's rear
    // cell stands on (6, 0), a level up, and pokes over the wall.
    let figs = |rear_z| {
        let mut defender = fig(5, 1, 10);
        defender.rear = Some((Hex::new(6, 0), rear_z));
        vec![fig(0, 1, 10), defender]
    };
    let walls = || vec![wall(Hex::new(3, 0), 1)];
    let raised = scene(stepped(Hex::new(6, 0)), walls(), figs(2));
    let result = geometric(&raised);
    assert!(result.visible, "the raised rear cell is seen over the wall");
    assert!(result.witnesses.len() < 21, "the anchor cell stays hidden");
    // The same cell at the anchor's level, as when every prism rose from the anchor, is hidden.
    let flat = scene(stepped(Hex::new(6, 0)), walls(), figs(1));
    assert!(!geometric(&flat).visible);
}

#[test]
fn a_rear_cell_a_level_down_blocks_at_its_own_level() {
    // A bystander stands on raised ground at (3, 1), with its rear cell down on (3, 0), across
    // the line between two figures on the ground.
    let bystander = |rear_z| {
        let mut f = fig(3, 2, 10);
        f.at = Hex::new(3, 1);
        f.rear = Some((Hex::new(3, 0), rear_z));
        f
    };
    let mut pieces = ground();
    pieces.push(piece(Hex::new(3, 1), 1));
    let figs = |rear_z| vec![fig(0, 1, 10), fig(6, 1, 10), bystander(rear_z)];
    let low = scene(pieces.clone(), vec![], figs(1));
    assert!(
        !geometric(&low).visible,
        "the rear cell on the ground blocks"
    );
    // Raised to the anchor's level, the rear prism passes over the line.
    assert!(geometric(&scene(pieces, vec![], figs(2))).visible);
}

#[test]
fn the_columns_policy_casts_one_segment_between_target_points() {
    let figs = vec![fig(0, 1, 10), fig(6, 1, 10)];
    let open = row(vec![], figs.clone());
    assert_eq!(
        ask(Policy::Columns, Threshold::Any, &open).witnesses.len(),
        1
    );
    let walled = row(vec![wall(Hex::new(3, 0), 2)], figs);
    assert!(!ask(Policy::Columns, Threshold::Any, &walled).visible);
}

#[test]
fn a_figure_off_the_board_is_an_error() {
    let scene = row(vec![], vec![fig(0, 1, 10), fig(6, 1, 10)]);
    let service = Service {
        policy: Policy::Geometric,
        threshold: Threshold::Any,
    };
    assert!(service.query(&scene, FigureId(0), FigureId(7)).is_err());
}

#[test]
fn the_module_registers_and_rejects_unknown_options() {
    let config = |policy: &str, threshold: &str| {
        let fields = [
            (Sym::new("policy"), Value::Str(Sym::new(policy))),
            (Sym::new("threshold"), Value::Str(Sym::new(threshold))),
        ];
        Value::Map(fields.into())
    };
    let register =
        |policy, threshold| Los.register(&mut Registrar::default(), &config(policy, threshold));
    assert!(register("columns", "all").is_ok());
    assert!(register("camera", "any").is_err());
    assert!(register("geometric", "some").is_err());
}

/// A generated board: ground at random levels, walls on some hexes, and up to three figures.
#[derive(Clone, Debug)]
struct Layout {
    levels: Vec<((i32, i32), u8)>,
    walls: Vec<((i32, i32), u8)>,
    figs: [((i32, i32), i64); 3],
    third: bool,
}

fn cells() -> Vec<(i32, i32)> {
    Hex::ORIGIN.spiral(2).iter().map(|h| (h.q, h.r)).collect()
}

fn layouts() -> impl Strategy<Value = Layout> {
    let count = cells().len();
    let level = prop::collection::vec(0u8..3, count);
    let wall = prop::collection::vec(prop::option::of(1u8..4), count);
    let spots = prop::sample::subsequence((0..count).collect::<Vec<_>>(), 3).prop_shuffle();
    (level, wall, spots, any::<bool>()).prop_map(move |(level, wall, spots, third)| {
        let all = cells();
        let levels = all.iter().copied().zip(level).collect();
        let walls = (0..count)
            .filter(|i| !spots.contains(i))
            .filter_map(|i| wall[i].map(|h| (all[i], h)))
            .collect();
        let figs = [0, 1, 2].map(|k| (all[spots[k]], 10 + 3 * k as i64));
        Layout {
            levels,
            walls,
            figs,
            third,
        }
    })
}

/// Builds the scene with every hex rotated about the origin by `turns` steps.
fn build(layout: &Layout, turns: u8) -> Scene {
    let spin = |(q, r): (i32, i32)| Hex::new(q, r).rotate(Rotation::from_steps(turns.into()));
    let pieces: Vec<_> = layout
        .levels
        .iter()
        .map(|&(h, l)| piece(spin(h), l))
        .collect();
    let walls = layout
        .walls
        .iter()
        .map(|&(h, n)| wall(spin(h), n))
        .collect();
    let tops: BTreeMap<Hex, i64> = layout
        .levels
        .iter()
        .map(|&(h, l)| (spin(h), i64::from(l) + 1))
        .collect();
    let stand = |(hex, target): ((i32, i32), i64)| {
        let at = spin(hex);
        Fig {
            at,
            z: tops[&at],
            target,
            hit: (0, 12),
            rear: None,
        }
    };
    let mut figs: Vec<Fig> = layout.figs.iter().copied().map(stand).collect();
    if !layout.third {
        figs.pop();
    }
    scene(pieces, walls, figs)
}

proptest! {
    #[test]
    fn los_is_invariant_under_rotating_the_whole_board(layout in layouts(), turns in 1u8..6) {
        let before = geometric(&build(&layout, 0));
        let after = geometric(&build(&layout, turns));
        prop_assert_eq!(before.visible, after.visible);
        prop_assert_eq!(before.clear_fraction, after.clear_fraction);
        prop_assert_eq!(before.witnesses.len(), after.witnesses.len());
    }

    #[test]
    fn the_same_question_gives_the_same_answer_twice(layout in layouts()) {
        let scene = build(&layout, 0);
        prop_assert_eq!(geometric(&scene), geometric(&scene));
    }
}
