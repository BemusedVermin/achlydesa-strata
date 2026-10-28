//! Golden determinism check (design §16.5): geometry, RNG and hashing must agree bit for bit on
//! every platform. Everything is compared against `fixtures/boards/golden.txt`.
//!
//! To rewrite the goldens after an intentional change, run `STRATA_BLESS=1 cargo test` and review
//! the diff to `golden.txt` before committing it.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::PathBuf;

use serde::Deserialize;
use strata_kernel::{CommandRng, Hex, Point, Point3, Seg3, StableHash, StableHasher, Sym, blocked};
use strata_model::board::{BoardDefs, compile};
use strata_model::*;

#[derive(Deserialize)]
struct BoardFile {
    tiles: BTreeMap<String, TileDef>,
    features: BTreeMap<String, FeatureDef>,
    map: MapDef,
}

struct Defs(BoardFile);

impl BoardDefs for Defs {
    fn tile(&self, name: &Sym) -> Option<&TileDef> {
        self.0.tiles.get(name.as_str())
    }
    fn feature(&self, name: &Sym) -> Option<&FeatureDef> {
        self.0.features.get(name.as_str())
    }
}

/// A line-of-sight query between hex centers at the given sub-level heights.
type Query = ((i32, i32, i64), (i32, i32, i64));

const BRIDGE: &str = include_str!("../../../fixtures/boards/bridge.json");
const WALL: &str = include_str!("../../../fixtures/boards/wall.json");

const BRIDGE_QUERIES: [Query; 6] = [
    ((0, 0, 40), (6, 0, 40)),
    ((0, 0, 18), (6, 0, 18)),
    ((2, 0, 30), (4, 0, 30)),
    ((2, 0, 30), (4, 0, 18)),
    ((6, 0, 30), (1, 0, 20)),
    ((0, 0, 30), (3, 0, 6)),
];

const WALL_QUERIES: [Query; 8] = [
    ((0, 0, 14), (4, 0, 14)),
    ((0, 0, 14), (4, 0, 40)),
    ((0, 0, 30), (4, 0, 14)),
    ((0, 1, 14), (4, 1, 14)),
    ((0, 0, 14), (3, 1, 14)),
    ((-2, 2, 14), (4, 0, 14)),
    ((4, 0, 14), (0, 0, 14)),
    ((0, 0, 14), (1, 0, 14)),
];

fn center(hex: (i32, i32, i64)) -> Point3 {
    Point3::new(Point::center(Hex::new(hex.0, hex.1)), hex.2)
}

/// Compiles one fixture and reports its column digest and query results.
fn board_report(out: &mut String, name: &str, json: &str, queries: &[Query]) {
    let defs = Defs(serde_json::from_str(json).expect("board fixture parses"));
    let board = compile(&defs.0.map, &defs).expect("board fixture compiles");
    let mut hasher = StableHasher::new();
    for q in -3..9 {
        for r in -1..4 {
            let hex = Hex::new(q, r);
            for surface in board.surfaces(hex) {
                hasher.write(&surface.z.to_le_bytes());
                hasher.write(&surface.terrain.0.to_le_bytes());
            }
            for blocker in board.blockers(hex) {
                hasher.write(&blocker.z0.to_le_bytes());
                hasher.write(&blocker.z1.to_le_bytes());
            }
        }
    }
    writeln!(out, "{name}.columns = {:016x}", hasher.finish()).unwrap();
    let bits: String = queries
        .iter()
        .map(|&(from, to)| {
            let ends = [Hex::new(from.0, from.1), Hex::new(to.0, to.1)];
            let seg = Seg3::new(center(from), center(to));
            if blocked(seg, |hex| board.blockers(hex), &ends) {
                '1'
            } else {
                '0'
            }
        })
        .collect();
    writeln!(out, "{name}.los_blocked = {bits}").unwrap();
}

fn state() -> GameState {
    GameState {
        players: vec![PlayerState {
            team: 0,
            hand: vec![strata_kernel::DefId(2)],
        }],
        units: vec![UnitState {
            figures: vec![FigureId(0)],
            ..Default::default()
        }],
        figures: vec![FigureState {
            location: Some(strata_kernel::Space::new(2, -1, 1)),
            ..Default::default()
        }],
        command_index: 42,
        ..Default::default()
    }
}

fn report() -> String {
    let mut out = String::new();
    board_report(&mut out, "bridge", BRIDGE, &BRIDGE_QUERIES);
    board_report(&mut out, "wall", WALL, &WALL_QUERIES);
    let mut rng = CommandRng::for_command(0x5EED, 7);
    let draws: Vec<u32> = (0..1000).map(|_| rng.next_u32()).collect();
    writeln!(out, "rng.first = {:?}", &draws[..4]).unwrap();
    writeln!(out, "rng.hash = {:016x}", draws.stable_hash_value()).unwrap();
    let mut rng = CommandRng::for_command(0x5EED, 8);
    let rolls: Vec<u32> = (0..1000).map(|_| rng.roll(6)).collect();
    writeln!(out, "rng.rolls = {:016x}", rolls.stable_hash_value()).unwrap();
    writeln!(out, "state.hash = {:016x}", state().hash()).unwrap();
    out
}

#[test]
fn matches_golden_values() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/boards/golden.txt");
    let actual = report();
    assert_eq!(
        actual,
        report(),
        "report is not repeatable within a process"
    );
    if std::env::var_os("STRATA_BLESS").is_some() {
        std::fs::write(&path, &actual).expect("write goldens");
        return;
    }
    let expected =
        std::fs::read_to_string(&path).expect("golden.txt exists; run with STRATA_BLESS=1");
    assert_eq!(
        actual,
        expected.replace("\r\n", "\n"),
        "golden values changed"
    );
}
