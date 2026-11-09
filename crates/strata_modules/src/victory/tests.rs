//! `core:victory` against a hand-built state: setup checks, settling, team attribution, the
//! tiebreaks, runtime failures, progress estimates and result reading. Whole matches are covered
//! by pack tests.

use std::collections::BTreeMap;

use strata_kernel::{CommandRng, DefId, Frac, Hex, PackId, Space, Sym, Value, WindowId};
use strata_model::{
    Answer, Decision, Descriptor, DieKind, Event, FigureId, FigureState, FigureStatus, GameState,
    Logic, MapDef, PlayerId, PlayerRange, PlayerState, ScenarioDef, ScriptRef, Text, UnitId,
    UnitState,
};
use strata_rules::{Ctx, Halt, LogicError, MatchResult, NativeEnv, Victory};

use super::{CONDITION_FAILED, Service, key};
use crate::rounds::ROUND_ENDED;

/// The zone every test that holds one uses: the single column (0, 0).
const ZONE: &str = "z";

/// Knows the scenario, one zone, the units' printed points and what any Lua condition returns.
struct Env {
    scenario: ScenarioDef,
    /// Printed points by unit definition; `None` for packs without a `points` stat.
    points: Option<BTreeMap<DefId, i64>>,
    script: Result<Value, Halt>,
}

impl Env {
    fn new(victory: Vec<Logic>) -> Self {
        Env {
            scenario: scenario(victory),
            points: Some(BTreeMap::new()),
            script: Ok(Value::Nil),
        }
    }
}

impl NativeEnv for Env {
    fn stat(&self, _: &GameState, _: FigureId, stat: &str) -> Result<i64, LogicError> {
        Err(LogicError(format!("no stat `{stat}`")))
    }
    fn die(&self, _: &str) -> Option<(DefId, DieKind)> {
        None
    }
    fn points(&self, unit: DefId) -> Option<i64> {
        let points = self.points.as_ref()?;
        Some(points.get(&unit).copied().unwrap_or(0))
    }
    fn scenario(&self, _: &GameState) -> Option<&ScenarioDef> {
        Some(&self.scenario)
    }
    fn zone(&self, _: &GameState, name: &str) -> Option<Vec<Hex>> {
        (name == ZONE).then(|| vec![Hex::new(0, 0)])
    }
    fn call_logic(&self, _: &mut dyn Ctx, _: &Logic, _: &[Value]) -> Result<Value, Halt> {
        self.script.clone()
    }
}

/// A `Ctx` that applies what is emitted and keeps it.
struct Bench {
    state: GameState,
    rng: CommandRng,
    events: Vec<Event>,
}

impl Bench {
    fn new(state: GameState) -> Self {
        Bench {
            state,
            rng: CommandRng::for_command(1, 0),
            events: Vec::new(),
        }
    }

    /// Settles as if the last command ended `rounds`.
    fn settle(&mut self, env: &Env, rounds: &[i64]) {
        let ended = rounds.iter().map(|round| Event::Custom {
            id: Sym::new(ROUND_ENDED),
            payload: Value::Map([(Sym::new("round"), Value::Int(*round))].into()),
        });
        let ended: Vec<Event> = ended.collect();
        Service.settle(self, env, &ended).unwrap();
    }

    fn result(&self) -> Option<MatchResult> {
        Service.result(&self.state)
    }

    /// The figure standing in the zone, or nobody.
    fn stand(&mut self, figure: Option<u32>) {
        let space = Space::at(Hex::new(0, 0), 1);
        let occupancy = &mut self.state.board_changes.occupancy;
        occupancy.clear();
        if let Some(figure) = figure {
            occupancy.insert(space, FigureId(figure));
        }
    }

    fn kinds(&self) -> Vec<&str> {
        let custom = self.events.iter().filter_map(|e| match e {
            Event::Custom { id, .. } => Some(id.as_str()),
            _ => None,
        });
        custom.collect()
    }
}

impl Ctx for Bench {
    fn state(&self) -> &GameState {
        &self.state
    }

    fn rng(&mut self) -> &mut CommandRng {
        &mut self.rng
    }

    fn emit(&mut self, e: Event) -> Result<(), Halt> {
        self.state.apply(&e)?;
        self.events.push(e);
        Ok(())
    }

    fn window(&mut self, _: &WindowId) -> Result<(), Halt> {
        Ok(())
    }

    fn choose(&mut self, _: PlayerId, _: Decision) -> Result<Answer, Halt> {
        Err(Halt::reject("the bench asks nothing"))
    }
}

fn win(fields: &[(&str, Value)]) -> Logic {
    let map = fields.iter().map(|(k, v)| (Sym::new(k), v.clone()));
    Logic::Native(Descriptor(Value::Map(map.collect())))
}

fn text(s: &str) -> Value {
    Value::Str(Sym::new(s))
}

fn eliminate_all() -> Logic {
    win(&[("win", text("eliminate_all"))])
}

fn hold_zone(zone: &str, rounds: i64) -> Logic {
    win(&[
        ("win", text("hold_zone")),
        ("zone", text(zone)),
        ("rounds", Value::Int(rounds)),
    ])
}

fn round_limit(rounds: i64, tiebreak: &[&str]) -> Logic {
    let tiebreak = tiebreak.iter().map(|t| text(t)).collect();
    win(&[
        ("win", text("round_limit")),
        ("rounds", Value::Int(rounds)),
        ("tiebreak", Value::List(tiebreak)),
    ])
}

fn script() -> Logic {
    Logic::Script(ScriptRef {
        pack: PackId::new("test"),
        def: DefId(0),
        path: Sym::new("victory[1]"),
    })
}

fn scenario(victory: Vec<Logic>) -> ScenarioDef {
    ScenarioDef {
        id: DefId(0),
        name: Text("s".into()),
        map: Sym::new("m"),
        players: PlayerRange { min: 2, max: 2 },
        slots: Vec::new(),
        army: Vec::new(),
        relics: Vec::new(),
        victory,
        addons: Vec::new(),
        after: BTreeMap::new(),
        tags: Default::default(),
        meta: BTreeMap::new(),
        source: Default::default(),
    }
}

fn map() -> MapDef {
    MapDef {
        id: DefId(0),
        name: Text("m".into()),
        authors: Vec::new(),
        orientation: Sym::new("pointy"),
        pieces: Vec::new(),
        features: Vec::new(),
        zones: BTreeMap::from([(Sym::new(ZONE), vec![Hex::new(0, 0)])]),
        relic_spots: BTreeMap::new(),
        constraints: Default::default(),
        tags: Default::default(),
        meta: BTreeMap::new(),
        source: Default::default(),
    }
}

/// Two players, each a team of their own, and one unit per entry of `units`: its definition,
/// owner, and its figures' statuses. Every figure is controlled by its unit's owner.
fn match_of(units: &[(u32, u8, &[FigureStatus])]) -> GameState {
    let mut state = GameState::default();
    for team in 0..2 {
        state.players.push(PlayerState {
            team,
            hand: Vec::new(),
        });
    }
    for (index, (def, owner, statuses)) in units.iter().enumerate() {
        let mut figures = Vec::new();
        for status in *statuses {
            figures.push(FigureId(state.figures.len() as u32));
            state.figures.push(FigureState {
                unit: UnitId(index as u32),
                controller: PlayerId(*owner),
                status: *status,
                ..FigureState::default()
            });
        }
        state.units.push(UnitState {
            def: DefId(*def),
            owner: PlayerId(*owner),
            figures,
            ..UnitState::default()
        });
    }
    state
}

use FigureStatus::{Destroyed, OnBoard};

/// Player 0's figure 0 and player 1's figure 1, both on the board.
fn duel() -> GameState {
    match_of(&[(0, 0, &[OnBoard]), (1, 1, &[OnBoard])])
}

fn ended(bench: &Bench) -> Option<(String, usize, Vec<u8>)> {
    let result = bench.result()?;
    let winners = result.winners.iter().map(|p| p.0).collect();
    Some((result.condition.as_str().to_owned(), result.index, winners))
}

fn won(condition: &str, index: usize, winners: &[u8]) -> Option<(String, usize, Vec<u8>)> {
    Some((condition.to_owned(), index, winners.to_vec()))
}

#[test]
fn setup_accepts_well_formed_conditions() {
    let victory = vec![
        eliminate_all(),
        hold_zone(ZONE, 2),
        round_limit(
            12,
            &["points_remaining", "points_destroyed", "figures_remaining"],
        ),
        win(&[
            ("win", text("round_limit")),
            ("rounds", Value::Int(3)),
            ("tiebreak", text("points_destroyed")),
        ]),
        script(),
    ];
    assert_eq!(Service.check(&scenario(victory), &map()), Ok(()));
}

#[test]
fn setup_refuses_malformed_conditions() {
    let refused = |condition: Logic| {
        let found = Service.check(&scenario(vec![eliminate_all(), condition]), &map());
        found.unwrap_err().0
    };
    assert_eq!(
        refused(hold_zone("nowhere", 1)),
        "victory condition 2: win.hold_zone: the map has no zone `nowhere`"
    );
    assert_eq!(
        refused(round_limit(12, &["initiative_roll"])),
        "victory condition 2: win.round_limit: unknown tiebreak `initiative_roll`"
    );
    assert_eq!(
        refused(win(&[("win", text("king_of_the_hill"))])),
        "victory condition 2: core:victory has no condition `king_of_the_hill`"
    );
    assert!(refused(hold_zone(ZONE, 0)).contains("rounds must be at least 1"));
    assert!(refused(win(&[("win", text("round_limit"))])).contains("win.round_limit"));
    assert!(refused(win(&[("zone", text(ZONE))])).contains("needs `win`"));
}

#[test]
fn holding_the_zone_counts_round_ends_in_a_row_and_losing_it_starts_again() {
    let env = Env::new(vec![hold_zone(ZONE, 2)]);
    let mut bench = Bench::new(duel());
    bench.stand(Some(0));
    bench.settle(&env, &[1]);
    assert_eq!(ended(&bench), None);
    // Lost at the second round end: the count is gone.
    bench.stand(None);
    bench.settle(&env, &[2]);
    assert_eq!(ended(&bench), None);
    // Held again: it needs both round ends once more.
    bench.stand(Some(0));
    bench.settle(&env, &[3]);
    assert_eq!(ended(&bench), None);
    bench.settle(&env, &[4]);
    assert_eq!(ended(&bench), won("hold_zone", 0, &[0]));
}

#[test]
fn a_new_holder_starts_its_count_at_one() {
    let env = Env::new(vec![hold_zone(ZONE, 2)]);
    let mut bench = Bench::new(duel());
    bench.stand(Some(0));
    bench.settle(&env, &[1]);
    bench.stand(Some(1));
    bench.settle(&env, &[2]);
    assert_eq!(ended(&bench), None);
    bench.settle(&env, &[3]);
    assert_eq!(ended(&bench), won("hold_zone", 0, &[1]));
}

#[test]
fn a_zone_is_held_by_whoever_controls_the_figure_in_it() {
    let env = Env::new(vec![hold_zone(ZONE, 1)]);
    let mut bench = Bench::new(duel());
    bench.stand(Some(1));
    let taken = Event::ControlChanged {
        figure: FigureId(1),
        controller: PlayerId(0),
    };
    bench.emit(taken).unwrap();
    bench.settle(&env, &[1]);
    assert_eq!(ended(&bench), won("hold_zone", 0, &[0]));
}

#[test]
fn elimination_counts_a_figure_for_its_owner_whoever_controls_it() {
    // Player 0's own figure is gone; it controls player 1's, which still counts for player 1.
    let env = Env::new(vec![eliminate_all()]);
    let state = match_of(&[(0, 0, &[Destroyed]), (1, 1, &[OnBoard])]);
    let mut bench = Bench::new(state);
    let taken = Event::ControlChanged {
        figure: FigureId(1),
        controller: PlayerId(0),
    };
    bench.emit(taken).unwrap();
    bench.settle(&env, &[]);
    assert_eq!(ended(&bench), won("eliminate_all", 0, &[1]));
}

/// Player 0 keeps a 30-point unit and lost a 60-point one; player 1 lost its only unit, worth
/// 50 points.
fn spent() -> GameState {
    match_of(&[
        (0, 0, &[OnBoard]),
        (2, 0, &[Destroyed]),
        (1, 1, &[Destroyed, Destroyed]),
    ])
}

fn priced(victory: Vec<Logic>) -> Env {
    let mut env = Env::new(victory);
    env.points = Some(BTreeMap::from([
        (DefId(0), 30),
        (DefId(1), 50),
        (DefId(2), 60),
    ]));
    env
}

#[test]
fn points_destroyed_ranks_teams_by_the_printed_points_they_destroyed() {
    // Player 1 destroyed 60 points, player 0 only 50.
    let env = priced(vec![round_limit(1, &["points_destroyed"])]);
    let mut bench = Bench::new(spent());
    bench.settle(&env, &[1]);
    assert_eq!(ended(&bench), won("round_limit", 0, &[1]));

    let env = priced(vec![round_limit(1, &["points_remaining"])]);
    let mut bench = Bench::new(spent());
    bench.settle(&env, &[1]);
    assert_eq!(ended(&bench), won("round_limit", 0, &[0]));
}

#[test]
fn without_a_points_stat_every_unit_is_worth_nothing() {
    let mut env = priced(vec![round_limit(
        1,
        &["points_destroyed", "points_remaining"],
    )]);
    env.points = None;
    let mut bench = Bench::new(spent());
    bench.settle(&env, &[1]);
    assert_eq!(ended(&bench), won("round_limit", 0, &[]));
}

#[test]
fn a_command_that_ends_a_round_is_checked_as_a_command_first() {
    // The round limit is listed first, but only applies at round end; elimination is met by the
    // command itself, which is checked before the round's end.
    let env = Env::new(vec![round_limit(1, &[]), eliminate_all()]);
    let mut bench = Bench::new(match_of(&[(0, 0, &[OnBoard]), (1, 1, &[Destroyed])]));
    bench.settle(&env, &[1]);
    assert_eq!(ended(&bench), won("eliminate_all", 1, &[0]));
}

#[test]
fn a_condition_that_fails_is_skipped_and_reported() {
    let failures = [
        Ok(Value::Int(300)),
        Ok(text("everyone")),
        Err(Halt::reject("boom")),
    ];
    for failure in failures {
        let mut env = Env::new(vec![script(), eliminate_all()]);
        env.script = failure.clone();
        let mut bench = Bench::new(duel());
        bench.settle(&env, &[]);
        assert_eq!(ended(&bench), None, "{failure:?}");
        assert_eq!(bench.kinds(), [CONDITION_FAILED], "{failure:?}");
        let Event::Custom { payload, .. } = &bench.events[0] else {
            unreachable!();
        };
        assert_eq!(payload.get("index"), Ok(&Value::Int(0)));
        assert_eq!(payload.get("condition"), Ok(&text("script")));

        // The conditions after it are still checked.
        bench.state.figures[1].status = Destroyed;
        bench.settle(&env, &[]);
        assert_eq!(ended(&bench), won("eliminate_all", 1, &[0]), "{failure:?}");
    }
}

#[test]
fn a_decision_a_condition_asks_for_still_pauses_the_command() {
    let mut env = Env::new(vec![script()]);
    let pause = Halt::NeedsDecision {
        decision: Decision {
            prompt: Sym::new("pick"),
            kind: strata_model::DecisionKind::YesNo,
        },
        chooser: PlayerId(0),
    };
    env.script = Err(pause.clone());
    let mut bench = Bench::new(duel());
    assert_eq!(Service.settle(&mut bench, &env, &[]), Err(pause));
}

#[test]
fn progress_estimates_elimination_and_holding_but_not_the_round_limit() {
    let env = Env::new(vec![
        eliminate_all(),
        hold_zone(ZONE, 2),
        win(&[("win", text("round_limit")), ("rounds", Value::Int(3))]),
    ]);
    // Player 1 fields two figures, one destroyed; player 0 has held the zone at one round end.
    let mut state = match_of(&[(0, 0, &[OnBoard]), (1, 1, &[OnBoard, Destroyed])]);
    let held = [
        (Sym::new("team"), Value::Int(0)),
        (Sym::new("rounds"), Value::Int(1)),
    ];
    state.vars.insert(key("held.1"), Value::Map(held.into()));
    let found = Service.progress(&state, &env, PlayerId(0)).unwrap();
    let got: Vec<(&str, usize, Frac)> = found
        .iter()
        .map(|p| (p.condition.as_str(), p.index, p.value))
        .collect();
    let half = Frac::new(1, 2);
    assert_eq!(got, [("eliminate_all", 0, half), ("hold_zone", 1, half)]);

    let found = Service.progress(&state, &env, PlayerId(1)).unwrap();
    let got: Vec<Frac> = found.iter().map(|p| p.value).collect();
    assert_eq!(got, [Frac::ZERO, Frac::ZERO]);
}

#[test]
fn no_result_until_one_is_recorded() {
    let mut state = duel();
    assert_eq!(Service.result(&state), None);
    let result = [
        (Sym::new("condition"), text("hold_zone")),
        (Sym::new("index"), Value::Int(1)),
        (Sym::new("winners"), Value::List(vec![Value::Int(0)])),
    ];
    state.vars.insert(key("result"), Value::Map(result.into()));
    let found = Service.result(&state).unwrap();
    assert_eq!((found.condition.as_str(), found.index), ("hold_zone", 1));
    assert_eq!(found.winners, [PlayerId(0)]);
}
