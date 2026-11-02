use std::collections::BTreeMap;
use std::sync::Arc;

use strata_kernel::{DefId, Sym, Value};
use strata_model::{
    ActionDef, Command, Descriptor, Event, FigureId, FigureState, FigureStatus, GameState, Logic,
    MatchSetup, PlayerId, PlayerState, UnitId, UnitState,
};

use super::*;
use crate::engine::{Ctx, Engine, Halt, HookRunner, Outcome};
use crate::registrar::{Anchor, CommandHandler, PhaseDef, Registrar};
use crate::runtime::RulesRuntime;

fn sym(s: &str) -> Sym {
    Sym::new(s)
}

fn phase(name: &str, at: Anchor) -> (PhaseDef, Anchor) {
    (PhaseDef { id: sym(name) }, at)
}

fn within(name: &str, parent: &str) -> (PhaseDef, Anchor) {
    phase(name, Anchor::Within(sym(parent)))
}

/// The base modules: rounds containing turns, then the end of the round.
fn base_phases() -> Vec<(PhaseDef, Anchor)> {
    vec![
        within("rounds", ROOT),
        within("turns", "rounds"),
        phase("round_end", Anchor::After(sym("turns"))),
    ]
}

fn names(tree: &PhaseTree) -> Vec<&str> {
    tree.phases().iter().map(|p| p.as_str()).collect()
}

#[test]
fn a_module_inserting_a_phase_needs_no_change_to_other_modules() {
    let base = base_phases();
    let mut with_morale = base.clone();
    with_morale.push(phase("morale", Anchor::After(sym("turns"))));
    let plain = PhaseTree::build(&base).unwrap();
    let extended = PhaseTree::build(&with_morale).unwrap();
    assert_eq!(names(&plain), ["rounds", "turns", "round_end"]);
    assert_eq!(names(&extended), ["rounds", "turns", "morale", "round_end"]);
    // The anchors of the base modules are untouched, and the order of registration is free.
    with_morale.rotate_right(1);
    assert_eq!(PhaseTree::build(&with_morale).unwrap(), extended);
}

#[test]
fn phases_insert_before_after_and_within() {
    let mut phases = base_phases();
    phases.push(phase("upkeep", Anchor::Before(sym("turns"))));
    phases.push(within("cleanup", "round_end"));
    phases.push(within("setup", ROOT));
    let tree = PhaseTree::build(&phases).unwrap();
    assert_eq!(
        names(&tree),
        ["rounds", "upkeep", "turns", "round_end", "cleanup", "setup"]
    );
    assert_eq!(tree.id(&sym("turns")), Some(DefId(2)));
    assert_eq!(tree.name(DefId(2)), Some(&sym("turns")));
}

#[test]
fn broken_phase_trees_are_reported() {
    let mut phases = base_phases();
    phases.push(phase("lost", Anchor::After(sym("nowhere"))));
    assert_eq!(
        PhaseTree::build(&phases),
        Err(FlowError::UnknownAnchor {
            phase: sym("lost"),
            anchor: sym("nowhere")
        })
    );
    let mut phases = base_phases();
    phases.push(within("turns", "rounds"));
    assert_eq!(
        PhaseTree::build(&phases),
        Err(FlowError::Duplicate(sym("turns")))
    );
}

fn logic() -> Logic {
    Logic::Native(Descriptor(Value::Nil))
}

fn action(id: &str, slots: &[&str], targeted: bool) -> ActionDef {
    ActionDef {
        id: sym(id),
        slots: slots.iter().map(|s| sym(s)).collect(),
        targets: targeted.then(logic),
        resolve: logic(),
    }
}

/// Targets are always 1 and 2; "dash" attacks and then grants another move.
struct Runner(ActivationRules);

impl ActionRunner for Runner {
    fn targets(&self, _: &ActionDef, _: FigureId, _: &GameState) -> Result<Vec<Value>, Halt> {
        Ok(vec![Value::Int(1), Value::Int(2)])
    }

    fn resolve(
        &self,
        def: &ActionDef,
        figure: FigureId,
        args: &Value,
        ctx: &mut dyn Ctx,
    ) -> Result<(), Halt> {
        ctx.emit(Event::Custom {
            id: def.id.clone(),
            payload: args.clone(),
        })?;
        if def.id.as_str() == "dash" {
            self.0.grant(ctx, figure, &sym("move"), 1)?;
        }
        Ok(())
    }
}

struct Handler {
    kind: &'static str,
    rules: ActivationRules,
    actions: Vec<ActionDef>,
    tree: PhaseTree,
}

impl CommandHandler for Handler {
    fn kind(&self) -> &str {
        self.kind
    }

    fn resolve(&self, ctx: &mut dyn Ctx, player: PlayerId, command: &Command) -> Result<(), Halt> {
        match command {
            Command::ActivateUnit(unit) => self.rules.begin(ctx, player, *unit),
            Command::Action { .. } => {
                let runner = Runner(self.rules.clone());
                take_action(&self.actions, &self.rules, &runner, ctx, player, command)
            }
            Command::EndActivation => ctx.emit(Event::ActivationEnded),
            _ if self.kind == "start" => self.tree.start(ctx),
            _ => self.tree.advance(ctx),
        }
    }
}

struct NoHooks;

impl HookRunner for NoHooks {
    fn replaces(
        &self,
        _: &strata_model::ReplacementDef,
        _: &Event,
        _: &GameState,
    ) -> Result<bool, Halt> {
        Ok(false)
    }

    fn replace(
        &self,
        _: &strata_model::ReplacementDef,
        e: &Event,
        _: &mut dyn Ctx,
    ) -> Result<Vec<Event>, Halt> {
        Ok(vec![e.clone()])
    }

    fn fires(&self, _: &strata_model::TriggerDef, _: &Event, _: &GameState) -> Result<bool, Halt> {
        Ok(false)
    }

    fn fire(&self, _: &strata_model::TriggerDef, _: &Event, _: &mut dyn Ctx) -> Result<(), Halt> {
        Ok(())
    }
}

fn rules(ordering: OrderingPolicy) -> ActivationRules {
    ActivationRules {
        steps: vec![sym("move"), sym("attack")],
        budgets: BTreeMap::new(),
        ordering,
    }
}

fn actions() -> Vec<ActionDef> {
    vec![
        action("walk", &["move"], false),
        action("shoot", &["attack"], true),
        action("dash", &["attack"], true),
    ]
}

fn runtime(rules: &ActivationRules) -> RulesRuntime {
    let mut phases = base_phases();
    phases.push(phase("morale", Anchor::After(sym("turns"))));
    let tree = PhaseTree::build(&phases).unwrap();
    let mut reg = Registrar::default();
    for (def, at) in phases {
        reg.phase(def, at);
    }
    actions().into_iter().for_each(|a| reg.action(a));
    for kind in [
        "start",
        "advance",
        "activate_unit",
        "action",
        "end_activation",
    ] {
        reg.command(Handler {
            kind,
            rules: rules.clone(),
            actions: actions(),
            tree: tree.clone(),
        });
    }
    RulesRuntime {
        slots: BTreeMap::new(),
        addons: Vec::new(),
        reg,
    }
}

/// Player 0 has unit 0 with figures 0 and 1; player 1 has unit 1 with figure 2.
fn state() -> GameState {
    let figure = |unit, controller| FigureState {
        unit: UnitId(unit),
        controller: PlayerId(controller),
        status: FigureStatus::OnBoard,
        ..Default::default()
    };
    let unit = |owner, figures: &[u32]| UnitState {
        owner: PlayerId(owner),
        figures: figures.iter().map(|f| FigureId(*f)).collect(),
        ..Default::default()
    };
    GameState {
        setup: Arc::new(MatchSetup::default()),
        players: vec![PlayerState::default(); 2],
        units: vec![unit(0, &[0, 1]), unit(1, &[2])],
        figures: vec![figure(0, 0), figure(0, 0), figure(1, 1)],
        ..Default::default()
    }
}

fn walk(f: u32) -> Command {
    Command::Action {
        figure: FigureId(f),
        action: DefId(0),
        args: Value::Nil,
    }
}

fn hit(f: u32, action: u32, target: i64) -> Command {
    Command::Action {
        figure: FigureId(f),
        action: DefId(action),
        args: Value::Int(target),
    }
}

fn custom(id: &str) -> Command {
    Command::Custom {
        id: sym(id),
        payload: Value::Nil,
    }
}

fn committed(outcome: Outcome) {
    assert!(matches!(outcome, Outcome::Committed(_)), "{outcome:?}");
}

fn rejected(outcome: Outcome) {
    assert!(matches!(outcome, Outcome::Rejected(_)), "{outcome:?}");
}

fn flow<'a>(rules: &ActivationRules, runner: &'a Runner) -> Flow<'a> {
    let mut phases = base_phases();
    phases.push(phase("morale", Anchor::After(sym("turns"))));
    Flow {
        tree: PhaseTree::build(&phases).unwrap(),
        rules: rules.clone(),
        runner,
        turn_phase: sym("turns"),
    }
}

/// Drives the engine into the turn phase of round one.
fn into_turns<'a>(rt: &'a RulesRuntime, runner: &'a Runner, rules: &ActivationRules) -> Engine<'a> {
    let mut engine = Engine::new(rt, &NoHooks, state()).with_flow(flow(rules, runner));
    committed(engine.submit(PlayerId(0), custom("start")));
    committed(engine.submit(PlayerId(0), custom("advance")));
    engine
}

#[test]
fn walking_the_tree_enters_and_exits_phases_in_order() {
    let rules = rules(OrderingPolicy::AllFiguresPerStep);
    let rt = runtime(&rules);
    let mut engine = Engine::new(&rt, &NoHooks, state());
    let mut seen = Vec::new();
    committed(engine.submit(PlayerId(0), custom("start")));
    for _ in 0..8 {
        let Outcome::Committed(events) = engine.submit(PlayerId(0), custom("advance")) else {
            break;
        };
        seen.extend(events);
    }
    let tree = flow(&rules, &Runner(rules.clone())).tree;
    let label = |e: &Event| match e {
        Event::PhaseEntered(id) => format!("+{}", tree.name(*id).unwrap()),
        Event::PhaseExited(id) => format!("-{}", tree.name(*id).unwrap()),
        other => panic!("{other:?}"),
    };
    let labels: Vec<_> = seen.iter().map(label).collect();
    assert_eq!(
        labels,
        [
            "+turns",
            "-turns",
            "+morale",
            "-morale",
            "+round_end",
            "-round_end",
            "-rounds"
        ]
    );
    assert!(engine.state().flow.phases.is_empty());
}

#[test]
fn an_activation_of_move_then_attack() {
    let rules = rules(OrderingPolicy::AllFiguresPerStep);
    let rt = runtime(&rules);
    let runner = Runner(rules.clone());
    let mut engine = into_turns(&rt, &runner, &rules);
    assert_eq!(
        engine.legal_commands(PlayerId(0)),
        [Command::ActivateUnit(UnitId(0))]
    );
    assert!(engine.legal_commands(PlayerId(1)).is_empty());

    committed(engine.submit(PlayerId(0), Command::ActivateUnit(UnitId(0))));
    // Only the current step's slots are open.
    assert_eq!(
        engine.legal_commands(PlayerId(0)),
        [walk(0), walk(1), Command::EndActivation]
    );
    rejected(engine.submit(PlayerId(0), hit(0, 1, 1)));

    committed(engine.submit(PlayerId(0), walk(0)));
    rejected(engine.submit(PlayerId(0), walk(0)));
    assert_eq!(
        engine.state().flow.activation.as_ref().unwrap().step,
        sym("move")
    );
    committed(engine.submit(PlayerId(0), walk(1)));
    // Every figure has moved, so the attack step opens.
    assert_eq!(
        engine.state().flow.activation.as_ref().unwrap().step,
        sym("attack")
    );
    let legal = engine.legal_commands(PlayerId(0));
    assert!(legal.contains(&hit(1, 1, 2)) && !legal.contains(&walk(0)));

    rejected(engine.submit(PlayerId(0), hit(0, 1, 3)));
    committed(engine.submit(PlayerId(0), hit(0, 1, 2)));
    committed(engine.submit(PlayerId(0), hit(1, 1, 1)));
    assert_eq!(engine.legal_commands(PlayerId(0)), [Command::EndActivation]);
    committed(engine.submit(PlayerId(0), Command::EndActivation));
    assert!(engine.state().flow.activation.is_none());
}

#[test]
fn only_the_active_players_units_and_figures_may_act() {
    let rules = rules(OrderingPolicy::AllFiguresPerStep);
    let rt = runtime(&rules);
    let runner = Runner(rules.clone());
    let mut engine = into_turns(&rt, &runner, &rules);
    rejected(engine.submit(PlayerId(1), Command::ActivateUnit(UnitId(0))));
    committed(engine.submit(PlayerId(0), Command::ActivateUnit(UnitId(0))));
    rejected(engine.submit(PlayerId(0), Command::ActivateUnit(UnitId(0))));
    rejected(engine.submit(PlayerId(0), walk(2)));
    rejected(engine.submit(PlayerId(1), walk(0)));
}

#[test]
fn granting_a_slot_reopens_an_earlier_step() {
    let rules = rules(OrderingPolicy::AllFiguresPerStep);
    let rt = runtime(&rules);
    let runner = Runner(rules.clone());
    let mut engine = into_turns(&rt, &runner, &rules);
    committed(engine.submit(PlayerId(0), Command::ActivateUnit(UnitId(0))));
    committed(engine.submit(PlayerId(0), walk(0)));
    committed(engine.submit(PlayerId(0), walk(1)));
    // "Dash" attacks and grants an extra move.
    committed(engine.submit(PlayerId(0), hit(0, 2, 1)));
    let act = engine.state().flow.activation.clone().unwrap();
    assert_eq!(act.step, sym("move"));
    assert_eq!(act.slots[&FigureId(0)][&sym("move")], 1);
    committed(engine.submit(PlayerId(0), walk(0)));
}

#[test]
fn per_figure_ordering_finishes_one_figure_before_the_next() {
    let rules = rules(OrderingPolicy::PerFigure);
    let rt = runtime(&rules);
    let runner = Runner(rules.clone());
    let mut engine = into_turns(&rt, &runner, &rules);
    committed(engine.submit(PlayerId(0), Command::ActivateUnit(UnitId(0))));
    // Either figure may start, and a figure moves before it attacks.
    let legal = engine.legal_commands(PlayerId(0));
    assert!(legal.contains(&walk(0)) && legal.contains(&walk(1)));
    assert!(!legal.contains(&hit(0, 1, 1)));

    committed(engine.submit(PlayerId(0), walk(0)));
    let legal = engine.legal_commands(PlayerId(0));
    assert!(legal.contains(&hit(0, 1, 1)));
    assert!(!legal.contains(&walk(1)) && !legal.contains(&hit(1, 1, 1)));
    rejected(engine.submit(PlayerId(0), walk(1)));

    committed(engine.submit(PlayerId(0), hit(0, 1, 1)));
    assert!(engine.legal_commands(PlayerId(0)).contains(&walk(1)));
}

#[test]
fn the_same_commands_twice_give_identical_hashes() {
    let play = || {
        let rules = rules(OrderingPolicy::AllFiguresPerStep);
        let rt = runtime(&rules);
        let runner = Runner(rules.clone());
        let mut engine = into_turns(&rt, &runner, &rules);
        for command in [
            Command::ActivateUnit(UnitId(0)),
            walk(0),
            walk(1),
            hit(0, 2, 1),
            walk(0),
            Command::EndActivation,
        ] {
            committed(engine.submit(PlayerId(0), command));
        }
        engine
            .log()
            .iter()
            .map(|r| r.state_hash)
            .collect::<Vec<_>>()
    };
    assert_eq!(play(), play());
}
