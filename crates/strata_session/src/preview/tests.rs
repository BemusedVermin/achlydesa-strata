//! Previews built from a ruleset with `core:combat` and an environment that answers reach, line
//! of sight and stats directly.

use std::collections::{BTreeMap, BTreeSet};

use strata_kernel::{
    DefId, Frac, Hex, ModuleId, Point, Point3, Rotation, Seg3, SlotId, Space, Sym, Value,
};
use strata_model::{
    Command, DieKind, FigureId, FigureState, FigureStatus, GameState, ModuleRef, PlayerId,
    PlayerState, RulesetDef, SourceLoc, Text, UndoPolicy,
};
use strata_modules::native_modules;
use strata_rules::{
    FigureVolume, LogicError, LosResult, ModuleCatalog, NativeEnv, Reach, RulesRuntime,
    RulesetSource, VoteQuery, resolve,
};

use super::*;

struct Only(RulesetDef);

impl RulesetSource for Only {
    fn ruleset(&self, _: &str) -> Option<&RulesetDef> {
        Some(&self.0)
    }
}

/// A ruleset with `core:combat` in its slot, the default `combat` die and `cover` on.
fn runtime() -> RulesRuntime {
    let mut catalog = ModuleCatalog::default();
    for module in native_modules() {
        catalog.register(module).unwrap();
    }
    let slot = |id: &str, config: BTreeMap<Sym, Value>| {
        let module = ModuleRef {
            id: ModuleId::new(id),
            config,
        };
        (SlotId::new(id.trim_start_matches("core:")), Some(module))
    };
    let cover = BTreeMap::from([(Sym::new("cover"), Value::Bool(true))]);
    let ruleset = RulesetDef {
        id: DefId(0),
        name: Text::default(),
        extends: None,
        stats: Vec::new(),
        dice: Vec::new(),
        slots: BTreeMap::from([
            slot("core:combat", cover),
            slot("core:units", BTreeMap::new()),
        ]),
        addons: Vec::new(),
        abilities: true,
        undo: UndoPolicy::default(),
        tags: BTreeSet::new(),
        meta: BTreeMap::new(),
        source: SourceLoc::default(),
    };
    resolve("rs", &Only(ruleset), &catalog).unwrap()
}

/// The attack action's ID in `rt`.
fn attack_action(rt: &RulesRuntime) -> DefId {
    let at = rt.actions().iter().position(|a| a.id.as_str() == "attack");
    DefId(at.expect("core:combat registers `attack`") as u32)
}

/// Attack 3, defense 2 and range 3 for everyone; line of sight with 7 of 21 samples clear and
/// one witness; figure 0 reaches (1, 0, 1) for 1, (2, 0, 1) through (1, 0, 1) for 2, and the
/// occupied (3, 0, 1).
struct Env;

fn witness() -> Seg3 {
    Seg3::new(
        Point3::new(Point::new(0, 0), 22),
        Point3::new(Point::new(45, 0), 18),
    )
}

fn reach(q: i32, cost: i64, path: Vec<Space>, occupied: bool) -> Reach {
    let space = Space::new(q, 0, 1);
    Reach {
        space,
        rotation: Rotation::default(),
        cells: vec![space],
        cost,
        path,
        walk: Vec::new(),
        stops_here: q == 2,
        falls: false,
        provokes_reprisal: q == 1,
        occupied,
    }
}

impl NativeEnv for Env {
    fn stat(&self, _: &GameState, _: FigureId, stat: &str) -> Result<i64, LogicError> {
        match stat {
            "attack" => Ok(3),
            "defense" => Ok(2),
            "range" => Ok(3),
            other => Err(LogicError(format!("no stat `{other}`"))),
        }
    }

    fn die(&self, name: &str) -> Option<(DefId, DieKind)> {
        let faces = ["hit", "hit", "hit", "block", "block", "blank"].map(Sym::new);
        (name == "combat").then(|| (DefId(0), DieKind::Faces(faces.to_vec())))
    }

    fn vote(&self, _: &GameState, query: &VoteQuery) -> Result<bool, LogicError> {
        Ok(query.default)
    }

    fn reachable(&self, _: &GameState, figure: FigureId) -> Result<Vec<Reach>, LogicError> {
        if figure != FigureId(0) {
            return Err(LogicError("only figure 0 moves".into()));
        }
        let one = Space::new(1, 0, 1);
        Ok(vec![
            reach(1, 1, vec![one], false),
            reach(2, 2, vec![one, Space::new(2, 0, 1)], false),
            reach(3, 3, Vec::new(), true),
        ])
    }

    fn los(&self, _: &GameState, _: FigureId, _: FigureId) -> Result<LosResult, LogicError> {
        Ok(LosResult {
            visible: true,
            clear_fraction: Frac::new(7, 21),
            witnesses: vec![witness()],
        })
    }

    fn volume(&self, state: &GameState, figure: FigureId) -> Option<FigureVolume> {
        let at = state.standing(figure)?.hex();
        Some(FigureVolume {
            anchor: at,
            cells: vec![at],
            target: 22,
            bottom: 12,
            top: 24,
        })
    }
}

/// Figure 0 of player 0 at (0, 0, 1); figures 1 and 2 of player 1 at (1, 0, 1) and (3, 0, 1).
fn state() -> GameState {
    let figure = |controller: u8, q: i32| FigureState {
        location: Some(Space::new(q, 0, 1)),
        status: FigureStatus::OnBoard,
        controller: PlayerId(controller),
        ..FigureState::default()
    };
    let team = |team| PlayerState {
        team,
        ..PlayerState::default()
    };
    GameState {
        players: vec![team(0), team(1)],
        figures: vec![figure(0, 0), figure(1, 1), figure(1, 3)],
        ..GameState::default()
    }
}

fn act(action: DefId, args: Value) -> Command {
    Command::Action {
        figure: FigureId(0),
        action,
        args,
    }
}

fn space_arg(q: i64) -> Value {
    Value::List(vec![Value::Int(q), Value::Int(0), Value::Int(1)])
}

#[test]
fn a_figure_with_a_space_target_gets_its_unoccupied_stops() {
    let rt = runtime();
    let walk = DefId(99);
    let legal = [act(walk, space_arg(1)), act(walk, space_arg(2))];
    let previews = Previews::of(&rt, &Env, &state(), &legal);
    let stops = previews.stops(FigureId(0));
    let spaces: Vec<Space> = stops.iter().map(|s| s.space).collect();
    assert_eq!(spaces, [Space::new(1, 0, 1), Space::new(2, 0, 1)]);
    let first = previews.stop(FigureId(0), Space::new(1, 0, 1)).unwrap();
    assert_eq!(
        (first.cost, first.provokes_reprisal, first.stops_here),
        (1, true, false)
    );
    let second = previews.stop(FigureId(0), Space::new(2, 0, 1)).unwrap();
    assert_eq!((second.cost, second.stops_here), (2, true));
    assert_eq!(second.path.len(), 2);
    assert!(previews.attacks.is_empty());
}

#[test]
fn a_space_given_as_a_table_counts_too() {
    let map = |q| {
        let fields = [("q", q), ("r", 0), ("z", 1)];
        let fields = fields.map(|(k, n)| (Sym::new(k), Value::Int(n)));
        Value::Map(fields.into_iter().collect())
    };
    let previews = Previews::of(&runtime(), &Env, &state(), &[act(DefId(99), map(1))]);
    assert_eq!(previews.stops(FigureId(0)).len(), 2);
    let not_space = [act(DefId(99), Value::Str(Sym::new("x")))];
    assert!(
        Previews::of(&runtime(), &Env, &state(), &not_space)
            .moves
            .is_empty()
    );
}

#[test]
fn an_adjacent_attack_previews_its_pools_without_line_of_sight() {
    let rt = runtime();
    let attack = attack_action(&rt);
    let legal = [act(attack, Value::Int(1))];
    let previews = Previews::of(&rt, &Env, &state(), &legal);
    let found = previews.attack(FigureId(0), FigureId(1)).unwrap();
    assert_eq!(found.action, attack);
    assert_eq!((found.attack.dice, found.defense.dice), (3, 2));
    assert_eq!(found.attack.face.as_str(), "hit");
    assert_eq!(found.attack.odds, Some((3, 6)));
    assert_eq!(found.defense.odds, Some((2, 6)));
    assert!(!found.ranged);
    assert_eq!(found.sight, None);
    assert_eq!(
        found.volume.as_ref().map(|v| v.cells.clone()),
        Some(vec![Hex::new(1, 0)])
    );
    assert!(previews.moves.is_empty());
}

#[test]
fn a_ranged_attack_carries_its_witnesses_and_cover() {
    let rt = runtime();
    let legal = [act(attack_action(&rt), Value::Int(2))];
    let previews = Previews::of(&rt, &Env, &state(), &legal);
    let found = previews.attack(FigureId(0), FigureId(2)).unwrap();
    assert!(found.ranged && found.cover, "7 of 21 clear is cover");
    assert_eq!(found.defense.dice, 3, "cover adds a defense die");
    let sight = found.sight.as_ref().unwrap();
    assert_eq!(sight.clear_fraction, Frac::new(1, 3));
    assert_eq!(sight.witnesses, [witness()]);
}

#[test]
fn other_actions_on_figures_are_not_attacks() {
    let legal = [act(DefId(99), Value::Int(1))];
    let previews = Previews::of(&runtime(), &Env, &state(), &legal);
    assert_eq!(previews, Previews::default());
}

#[test]
fn a_range_die_has_no_odds() {
    struct Ranged;
    impl NativeEnv for Ranged {
        fn stat(&self, s: &GameState, f: FigureId, stat: &str) -> Result<i64, LogicError> {
            Env.stat(s, f, stat)
        }
        fn die(&self, _: &str) -> Option<(DefId, DieKind)> {
            Some((DefId(0), DieKind::Range(1, 6)))
        }
        fn vote(&self, s: &GameState, q: &VoteQuery) -> Result<bool, LogicError> {
            Env.vote(s, q)
        }
    }
    let pool = PreviewPool {
        dice: 2,
        die: "combat".into(),
        face: Sym::new("hit"),
    };
    assert_eq!(PoolPreview::of(pool.clone(), &Ranged).odds, None);
    assert_eq!(PoolPreview::of(pool, &Env).odds, Some((3, 6)));
}
