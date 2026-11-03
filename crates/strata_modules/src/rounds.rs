//! `core:rounds`: the round loop, initiative and round end (design §6.1, §6.4).
//!
//! The module owns the phases `round` > (`initiative`, `turns`, `round_end`). The `start_round`
//! command begins a round: every player rolls the initiative die, ties are broken, and the
//! players take turns in that order, `turns_per_round` times each. A turn ends when its
//! activation ends. Other modules slot phases in around these, such as token placement after
//! `initiative`.

use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use strata_kernel::{ModuleId, SlotId, Sym, Value};
use strata_model::{
    Command, ConfigField, Descriptor, Event, GameState, Logic, PlayerId, TriggerDef, VarKey,
};
use strata_rules::{
    Anchor, CommandHandler, ConfigSchema, Ctx, Halt, ModuleError, NativeEnv, NativeLogic, PhaseDef,
    PhaseTree, ROOT, Registrar, RuleModule, roll_pool,
};

/// The phase that contains a whole round.
pub const ROUND: &str = "round";
/// The phase in which players roll for initiative.
pub const INITIATIVE: &str = "initiative";
/// The phase in which players take turns.
pub const TURNS: &str = "turns";
/// The phase that closes the round.
pub const ROUND_END: &str = "round_end";
/// The custom event emitted when a round ends, carrying `{ round }`.
pub const ROUND_ENDED: &str = "round_ended";

/// The event the `start_round` command emits for the module's native logic to act on.
const REQUESTED: &str = "round_requested";
/// The name the driver is registered under.
const DRIVE: &str = "core:rounds.drive";
/// The module that owns the round's variables.
const OWNER: &str = "core:rounds";
/// How many times a tie is rerolled before the lower player number wins it.
const MAX_REROLLS: u32 = 8;

/// How players tied on the initiative roll are ordered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tie {
    /// The tied players roll again.
    Reroll,
    /// The player who went first last round goes first among them.
    Keep,
}

/// The turns slot.
pub struct Rounds;

impl RuleModule for Rounds {
    fn id(&self) -> ModuleId {
        ModuleId::new(OWNER)
    }

    fn slot(&self) -> Option<SlotId> {
        Some(SlotId::new("turns"))
    }

    fn depends_on(&self) -> &[ModuleId] {
        &[]
    }

    fn config_schema(&self) -> &ConfigSchema {
        static SCHEMA: OnceLock<ConfigSchema> = OnceLock::new();
        SCHEMA.get_or_init(|| {
            let field = |kind: &str, default: Value| ConfigField {
                kind: Sym::new(kind),
                default,
            };
            let text = |s: &str| Value::Str(Sym::new(s));
            ConfigSchema::new(BTreeMap::from([
                (Sym::new("turns_per_round"), field("int", Value::Int(1))),
                (Sym::new("initiative"), field("str", text("d20"))),
                (Sym::new("tie"), field("str", text("reroll"))),
            ]))
        })
    }

    fn register(&self, r: &mut Registrar, cfg: &Value) -> Result<(), ModuleError> {
        let bad = |e: strata_kernel::ValueError| ModuleError::Invalid(e.to_string());
        let per_round = cfg.get("turns_per_round").and_then(Value::as_int);
        let per_round = u32::try_from(per_round.map_err(bad)?)
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| ModuleError::Invalid("turns_per_round must be at least 1".into()))?;
        let die = cfg.get("initiative").and_then(Value::as_str).map_err(bad)?;
        let tie = match cfg.get("tie").and_then(Value::as_str).map_err(bad)? {
            "reroll" => Tie::Reroll,
            "keep" => Tie::Keep,
            other => {
                return Err(ModuleError::Invalid(format!(
                    "tie must be `reroll` or `keep`, not `{other}`"
                )));
            }
        };
        let mut phase = |id: &str, at: Anchor| r.phase(PhaseDef { id: Sym::new(id) }, at);
        let within = |parent: &str| Anchor::Within(Sym::new(parent));
        phase(ROUND, within(ROOT));
        phase(INITIATIVE, within(ROUND));
        phase(TURNS, within(ROUND));
        phase(ROUND_END, within(ROUND));
        r.command(StartRound);
        let drive = Drive {
            per_round,
            die: die.to_owned(),
            tie,
        };
        r.native(DRIVE, Box::new(drive));
        let call = [(Sym::new("native"), Value::Str(Sym::new(DRIVE)))];
        for event in [REQUESTED, "activation_ended"] {
            r.trigger(TriggerDef {
                event: Sym::new(event),
                filter: None,
                limit: None,
                effect: Logic::Native(Descriptor(Value::Map(call.clone().into()))),
            });
        }
        Ok(())
    }
}

/// Handles `start_round`, which asks the driver to begin the next round.
struct StartRound;

impl CommandHandler for StartRound {
    fn kind(&self) -> &str {
        "start_round"
    }

    fn resolve(&self, ctx: &mut dyn Ctx, _: PlayerId, _: &Command) -> Result<(), Halt> {
        ctx.emit(Event::Custom {
            id: Sym::new(REQUESTED),
            payload: Value::Nil,
        })
    }
}

fn key(name: &str) -> VarKey {
    VarKey {
        module: ModuleId::new(OWNER),
        name: Sym::new(name),
    }
}

/// The seating order of the current round, first to last.
fn order(state: &GameState) -> Vec<PlayerId> {
    let list = state.vars.get(&key("order")).and_then(|v| v.as_list().ok());
    let ints = list.unwrap_or_default().iter();
    ints.filter_map(|v| v.as_int().ok())
        .map(|p| PlayerId(p as u8))
        .collect()
}

/// Moves the round through its phases and turns.
struct Drive {
    per_round: u32,
    die: String,
    tie: Tie,
}

impl NativeLogic for Drive {
    fn run(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, event: &Event) -> Result<(), Halt> {
        let tree = env
            .phases()
            .ok_or_else(|| Halt::reject("core:rounds needs the phase tree"))?;
        match event {
            Event::Custom { .. } => self.begin(ctx, env, tree),
            Event::ActivationEnded => self.next_turn(ctx, tree),
            _ => Ok(()),
        }
    }
}

impl Drive {
    /// Starts the next round and rolls initiative, leaving the game in the phase after it.
    fn begin(&self, ctx: &mut dyn Ctx, env: &dyn NativeEnv, tree: &PhaseTree) -> Result<(), Halt> {
        let state = ctx.state();
        if !state.flow.phases.is_empty() {
            return Err(Halt::reject("a round is already under way"));
        }
        let seats = state.players.len();
        if seats == 0 {
            return Err(Halt::reject("there are no players"));
        }
        let first = order(state).first().copied();
        let round = state.flow.round + 1;
        ctx.emit(Event::RoundStarted(round))?;
        tree.start(ctx)?;
        tree.advance(ctx)?;
        let players = (0..seats as u8).map(PlayerId).collect();
        let seating = self.rank(ctx, env, players, first, 0)?;
        let ints = seating.iter().map(|p| Value::Int(i64::from(p.0)));
        let value = Value::List(ints.collect());
        ctx.emit(Event::VarSet {
            key: key("order"),
            value,
        })?;
        advance(ctx, tree)
    }

    /// Every player in `players` rolls the initiative die; the highest goes first.
    fn rank(
        &self,
        ctx: &mut dyn Ctx,
        env: &dyn NativeEnv,
        players: Vec<PlayerId>,
        previous: Option<PlayerId>,
        rerolls: u32,
    ) -> Result<Vec<PlayerId>, Halt> {
        let mut groups: BTreeMap<Reverse<u32>, Vec<PlayerId>> = BTreeMap::new();
        for player in players {
            let rolled = roll_pool(ctx, env, &self.die, 1)?.results[0];
            groups.entry(Reverse(rolled)).or_default().push(player);
        }
        let mut seating = Vec::new();
        for (_, mut tied) in groups {
            if tied.len() > 1 && self.tie == Tie::Reroll && rerolls < MAX_REROLLS {
                tied = self.rank(ctx, env, tied, previous, rerolls + 1)?;
            } else if let Some(at) = tied.iter().position(|p| Some(*p) == previous) {
                // Without a reroll, last round's first player wins the tie.
                let kept = tied.remove(at);
                tied.insert(0, kept);
            }
            seating.extend(tied);
        }
        Ok(seating)
    }

    /// Hands the turn to the next player, or ends the turns phase after the last turn.
    fn next_turn(&self, ctx: &mut dyn Ctx, tree: &PhaseTree) -> Result<(), Halt> {
        let state = ctx.state();
        let top = state.flow.phases.last().and_then(|f| tree.name(f.phase));
        if top.map(Sym::as_str) != Some(TURNS) {
            return Ok(());
        }
        let turn = state.flow.turn;
        if turn < self.per_round * order(state).len() as u32 {
            start_turn(ctx, turn + 1)
        } else {
            advance(ctx, tree)
        }
    }
}

/// Starts turn `turn` (counted from 1): the seats take it in order, over and over.
fn start_turn(ctx: &mut dyn Ctx, turn: u32) -> Result<(), Halt> {
    let seating = order(ctx.state());
    let seat = (turn as usize - 1) % seating.len().max(1);
    let active = *seating
        .get(seat)
        .ok_or_else(|| Halt::reject("the round has no seating order"))?;
    ctx.emit(Event::TurnStarted { turn, active })
}

/// Announces the end of the round, then leaves its phases.
fn end_round(ctx: &mut dyn Ctx, tree: &PhaseTree) -> Result<(), Halt> {
    let round = Value::Int(i64::from(ctx.state().flow.round));
    ctx.emit(Event::Custom {
        id: Sym::new(ROUND_ENDED),
        payload: Value::Map([(Sym::new("round"), round)].into()),
    })?;
    tree.advance(ctx)
}

/// Moves to the phase after the current one and does what that phase calls for: the first turn
/// starts when `turns` opens, and the round ends when `round_end` does.
fn advance(ctx: &mut dyn Ctx, tree: &PhaseTree) -> Result<(), Halt> {
    tree.advance(ctx)?;
    let top = ctx.state().flow.phases.last();
    let top = top
        .and_then(|f| tree.name(f.phase))
        .map(|p| p.as_str().to_owned());
    match top.as_deref() {
        Some(TURNS) => start_turn(ctx, 1),
        Some(ROUND_END) => end_round(ctx, tree),
        _ => Ok(()),
    }
}
