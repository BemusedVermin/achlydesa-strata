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
    ConfigSchema, Ctx, FigureVolume, Halt, LogicError, LosResult, ModuleCatalog, ModuleError,
    NativeAction, NativeEnv, Reach, Registrar, RuleModule, RulesRuntime, RulesetSource, VoteQuery,
    resolve,
};

use super::*;

struct Only(RulesetDef);

impl RulesetSource for Only {
    fn ruleset(&self, _: &str) -> Option<&RulesetDef> {
        Some(&self.0)
    }
}

/// An add-on with three actions besides the attack: `walk`, which targets every stop the
/// figure reaches, as a pack's move does with `ctx.movement.reachable`; `hop`, which targets
/// only the first of them; and `shove`, which targets figure 1.
struct Actions;

/// What one test action targets.
struct Targets(fn(&GameState, &dyn NativeEnv, FigureId) -> Vec<Value>);

impl NativeAction for Targets {
    fn targets(
        &self,
        state: &GameState,
        env: &dyn NativeEnv,
        figure: FigureId,
    ) -> Result<Vec<Value>, Halt> {
        Ok((self.0)(state, env, figure))
    }

    fn resolve(
        &self,
        _: &mut dyn Ctx,
        _: &dyn NativeEnv,
        _: FigureId,
        _: &Value,
    ) -> Result<(), Halt> {
        Ok(())
    }
}

/// Every unoccupied stop of `figure`, listed as `[q, r, z]`.
fn reach_targets(state: &GameState, env: &dyn NativeEnv, figure: FigureId) -> Vec<Value> {
    let reach = env.reachable(state, figure).unwrap_or_default();
    let stops = reach.iter().filter(|r| !r.occupied);
    stops.map(|r| space_arg(i64::from(r.space.q))).collect()
}

impl RuleModule for Actions {
    fn id(&self) -> ModuleId {
        ModuleId::new("test:actions")
    }

    fn slot(&self) -> Option<SlotId> {
        None
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        static EMPTY: std::sync::OnceLock<ConfigSchema> = std::sync::OnceLock::new();
        EMPTY.get_or_init(ConfigSchema::default)
    }

    fn register(&self, r: &mut Registrar, _: &Value) -> Result<(), ModuleError> {
        r.native_action(
            "walk",
            &["move"],
            "test:walk",
            Box::new(Targets(reach_targets)),
        );
        let hop = |state: &GameState, env: &dyn NativeEnv, figure| {
            reach_targets(state, env, figure)
                .into_iter()
                .take(1)
                .collect()
        };
        r.native_action("hop", &["move"], "test:hop", Box::new(Targets(hop)));
        let shove = |_: &GameState, _: &dyn NativeEnv, _| vec![Value::Int(1)];
        r.native_action("shove", &["attack"], "test:shove", Box::new(Targets(shove)));
        Ok(())
    }
}

/// A ruleset with `core:combat` in its slot, the default `combat` die and `cover` on, and the
/// [`Actions`] add-on.
fn runtime() -> RulesRuntime {
    let mut catalog = ModuleCatalog::default();
    for module in native_modules() {
        catalog.register(module).unwrap();
    }
    catalog.register(Box::new(Actions)).unwrap();
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
        addons: vec![ModuleRef {
            id: ModuleId::new("test:actions"),
            config: BTreeMap::new(),
        }],
        abilities: true,
        undo: UndoPolicy::default(),
        tags: BTreeSet::new(),
        meta: BTreeMap::new(),
        source: SourceLoc::default(),
    };
    resolve("rs", &Only(ruleset), &catalog).unwrap()
}

/// The ID of the action `id` in `rt`.
fn action(rt: &RulesRuntime, id: &str) -> DefId {
    let at = rt.actions().iter().position(|a| a.id.as_str() == id);
    DefId(at.unwrap_or_else(|| panic!("`{id}` is registered")) as u32)
}

/// The attack action's ID in `rt`.
fn attack_action(rt: &RulesRuntime) -> DefId {
    action(rt, "attack")
}

/// Every legal command of figure 0 taking the action `id`, from the action's own targets.
fn legal_for(rt: &RulesRuntime, id: &str) -> Vec<Command> {
    let targets = match id {
        "walk" => reach_targets(&state(), &Env, FigureId(0)),
        "hop" => reach_targets(&state(), &Env, FigureId(0))[..1].to_vec(),
        "shove" => vec![Value::Int(1)],
        other => panic!("no test action `{other}`"),
    };
    let found = action(rt, id);
    targets.into_iter().map(|t| act(found, t)).collect()
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
fn a_move_action_gets_the_figures_unoccupied_stops() {
    let rt = runtime();
    let walk = action(&rt, "walk");
    let legal = legal_for(&rt, "walk");
    assert_eq!(legal.len(), 2, "the occupied stop is no target");
    let previews = Previews::of(&rt, &Env, &state(), &legal);
    let stops = previews.stops(FigureId(0), walk);
    let spaces: Vec<Space> = stops.iter().map(|s| s.space).collect();
    assert_eq!(spaces, [Space::new(1, 0, 1), Space::new(2, 0, 1)]);
    let first = previews
        .stop(FigureId(0), walk, Space::new(1, 0, 1))
        .unwrap();
    assert_eq!(
        (first.cost, first.provokes_reprisal, first.stops_here),
        (1, true, false)
    );
    let second = previews
        .stop(FigureId(0), walk, Space::new(2, 0, 1))
        .unwrap();
    assert_eq!((second.cost, second.stops_here), (2, true));
    assert_eq!(second.path.len(), 2);
    assert!(previews.attacks.is_empty());
}

#[test]
fn a_space_action_that_is_not_the_move_gets_no_movement_preview() {
    // `hop` targets a space the figure reaches, but not every one: it is not the move.
    let rt = runtime();
    let (walk, hop) = (action(&rt, "walk"), action(&rt, "hop"));
    let mut legal = legal_for(&rt, "hop");
    let only_hop = Previews::of(&rt, &Env, &state(), &legal);
    assert_eq!(only_hop, Previews::default());
    legal.extend(legal_for(&rt, "walk"));
    let both = Previews::of(&rt, &Env, &state(), &legal);
    assert_eq!(both.stops(FigureId(0), walk).len(), 2);
    assert!(both.stops(FigureId(0), hop).is_empty());
    assert_eq!(both.stop(FigureId(0), hop, Space::new(1, 0, 1)), None);
    assert_eq!(
        both.moves.keys().collect::<Vec<_>>(),
        [&(FigureId(0), walk)]
    );
}

#[test]
fn a_space_given_as_a_table_counts_too() {
    let map = |q| {
        let fields = [("q", q), ("r", 0), ("z", 1)];
        let fields = fields.map(|(k, n)| (Sym::new(k), Value::Int(n)));
        Value::Map(fields.into_iter().collect())
    };
    let rt = runtime();
    let walk = action(&rt, "walk");
    let legal = [act(walk, map(1)), act(walk, map(2))];
    let previews = Previews::of(&rt, &Env, &state(), &legal);
    assert_eq!(previews.stops(FigureId(0), walk).len(), 2);
    let not_space = [act(walk, map(1)), act(walk, Value::Str(Sym::new("x")))];
    assert!(
        Previews::of(&rt, &Env, &state(), &not_space)
            .moves
            .is_empty()
    );
}

#[test]
fn a_figure_that_cannot_answer_reach_gets_no_movement_preview() {
    let rt = runtime();
    let walk = action(&rt, "walk");
    let legal = [Command::Action {
        figure: FigureId(1),
        action: walk,
        args: space_arg(1),
    }];
    assert!(Previews::of(&rt, &Env, &state(), &legal).moves.is_empty());
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
    // `shove` targets figure 1, which `attack` could attack too; only the attack is previewed.
    let rt = runtime();
    let legal = legal_for(&rt, "shove");
    assert_eq!(
        Previews::of(&rt, &Env, &state(), &legal),
        Previews::default()
    );
    let mut both = legal.clone();
    both.push(act(attack_action(&rt), Value::Int(1)));
    let previews = Previews::of(&rt, &Env, &state(), &both);
    let actions: Vec<DefId> = previews.attacks.iter().map(|a| a.action).collect();
    assert_eq!(actions, [attack_action(&rt)]);
    assert!(previews.moves.is_empty());
}

#[test]
fn a_misconfigured_die_gives_no_attack_preview() {
    // The attack would be rejected: the die has no `block` face.
    struct NoBlock;
    impl NativeEnv for NoBlock {
        fn stat(&self, s: &GameState, f: FigureId, stat: &str) -> Result<i64, LogicError> {
            Env.stat(s, f, stat)
        }
        fn die(&self, _: &str) -> Option<(DefId, DieKind)> {
            let faces = ["hit", "hit", "blank"].map(Sym::new);
            Some((DefId(0), DieKind::Faces(faces.to_vec())))
        }
        fn vote(&self, s: &GameState, q: &VoteQuery) -> Result<bool, LogicError> {
            Env.vote(s, q)
        }
    }
    let rt = runtime();
    let legal = [act(attack_action(&rt), Value::Int(1))];
    assert!(
        Previews::of(&rt, &NoBlock, &state(), &legal)
            .attacks
            .is_empty()
    );
    assert_eq!(Previews::of(&rt, &Env, &state(), &legal).attacks.len(), 1);
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

#[test]
fn a_ranged_attack_preview_casts_line_of_sight_once() {
    // The combat slot hands back the cast it judged the attack on; the preview reuses it.
    struct Counting(std::sync::atomic::AtomicU32);
    impl NativeEnv for Counting {
        fn stat(&self, s: &GameState, f: FigureId, stat: &str) -> Result<i64, LogicError> {
            Env.stat(s, f, stat)
        }
        fn die(&self, name: &str) -> Option<(DefId, DieKind)> {
            Env.die(name)
        }
        fn vote(&self, s: &GameState, q: &VoteQuery) -> Result<bool, LogicError> {
            Env.vote(s, q)
        }
        fn los(&self, s: &GameState, a: FigureId, b: FigureId) -> Result<LosResult, LogicError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Env.los(s, a, b)
        }
    }
    let env = Counting(0.into());
    let rt = runtime();
    let legal = [act(attack_action(&rt), Value::Int(2))];
    let previews = Previews::of(&rt, &env, &state(), &legal);
    let found = previews.attack(FigureId(0), FigureId(2)).unwrap();
    assert_eq!(found.sight.as_ref().map(|s| s.witnesses.len()), Some(1));
    assert_eq!(env.0.into_inner(), 1);
}
