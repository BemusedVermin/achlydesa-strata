//! Previews: what the viewer's legal moves and attacks would do, computed on the simulation thread
//! where the rules can be asked (design §13.1, §13.6).
//!
//! The client only draws these. Movement previews come from the `movement` slot's reach, the same
//! result the move action and the AI read, and are attached only to the actions whose targets
//! are that reach; attack previews come from the `combat` slot's [`CombatPreview`] service, with
//! the line of sight it judged the attack on. Nothing here rolls dice or changes the state.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use strata_kernel::{DefId, Rotation, Space, Sym, Value};
use strata_model::{Command, FigureId, GameState};
use strata_rules::{
    CombatPreview, FigureVolume, LosResult, NativeEnv, PreviewPool, Reach, RulesRuntime,
    native_name,
};

/// One space a figure may stop on, with what getting there costs and causes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveStop {
    /// The anchor space.
    pub space: Space,
    /// The figure's rotation there.
    pub rotation: Rotation,
    /// Total movement cost.
    pub cost: i64,
    /// The anchor spaces walked, excluding the start and ending at `space`.
    pub path: Vec<Space>,
    /// Entering the space ends the move.
    pub stops_here: bool,
    /// The last step drops further than the fall limit.
    pub falls: bool,
    /// Some step leaves engagement with an enemy who gets a reprisal.
    pub provokes_reprisal: bool,
}

impl MoveStop {
    fn of(reach: &Reach) -> Self {
        Self {
            space: reach.space,
            rotation: reach.rotation,
            cost: reach.cost,
            path: reach.path.clone(),
            stops_here: reach.stops_here,
            falls: reach.falls,
            provokes_reprisal: reach.provokes_reprisal,
        }
    }
}

/// One side's dice in an attack preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PoolPreview {
    /// How many dice it rolls.
    pub dice: i64,
    /// The die's name.
    pub die: String,
    /// The face that counts: `hit` for the attack, `block` for the defense by default.
    pub face: Sym,
    /// How many of the die's faces are `face`, and how many faces it has; `None` when the die is
    /// unknown or a range die, which has no faces to count.
    pub odds: Option<(u32, u32)>,
}

impl PoolPreview {
    fn of(pool: PreviewPool, env: &dyn NativeEnv) -> Self {
        let odds = env
            .die(&pool.die)
            .and_then(|(_, kind)| kind.odds(pool.face.as_str()));
        Self {
            dice: pool.dice,
            die: pool.die,
            face: pool.face,
            odds,
        }
    }
}

/// What one legal attack would roll, and how the attacker sees the target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttackPreview {
    /// The attacking figure.
    pub attacker: FigureId,
    /// The target.
    pub target: FigureId,
    /// The attack action, as the legal command names it.
    pub action: DefId,
    /// The attacker's pool, counting hits.
    pub attack: PoolPreview,
    /// The target's pool, counting blocks.
    pub defense: PoolPreview,
    /// Whether the target is beyond adjacency, so line of sight was needed.
    pub ranged: bool,
    /// Whether the target adds a defense die for cover.
    pub cover: bool,
    /// The line-of-sight answer, with its clear fraction and witness rays, for a ranged attack
    /// the `los` slot could answer.
    pub sight: Option<LosResult>,
    /// The target's volume, to outline around its sprite.
    pub volume: Option<FigureVolume>,
}

/// Every preview for the viewer's legal commands.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Previews {
    /// The stops of each move, by figure and move action. An action counts as a move for a
    /// figure when its legal targets are exactly the stops the `movement` slot lists for that
    /// figure, as `ctx.movement.reachable` lists them; any other action that targets spaces gets
    /// no movement preview.
    pub moves: BTreeMap<(FigureId, DefId), Vec<MoveStop>>,
    /// One preview per legal attack command.
    pub attacks: Vec<AttackPreview>,
}

/// A space target as a move lists it: the anchor space, and the rotation for a figure whose
/// footprint is larger than one cell.
type Spot = (Space, Option<Rotation>);

impl Previews {
    /// Previews for `legal`, the viewer's legal commands in `state`. A figure whose reach or an
    /// attack whose pools the rules cannot answer is left out.
    pub fn of(
        rt: &RulesRuntime,
        env: &dyn NativeEnv,
        state: &GameState,
        legal: &[Command],
    ) -> Self {
        let combat = rt.service::<Arc<dyn CombatPreview>>();
        let attack_action = |action: DefId| {
            let def = rt.actions().get(action.0 as usize);
            let native = def.and_then(|d| native_name(&d.resolve));
            combat.is_some_and(|c| native == Some(c.action()))
        };
        let mut previews = Self::default();
        // The targets of each figure's non-attack actions; `None` once one is not a space.
        let mut spots: BTreeMap<(FigureId, DefId), Option<BTreeSet<Spot>>> = BTreeMap::new();
        for command in legal {
            let Command::Action {
                figure,
                action,
                args,
            } = command
            else {
                continue;
            };
            if let (Some(combat), true) = (combat, attack_action(*action)) {
                let target = args.as_int().ok().and_then(|n| u32::try_from(n).ok());
                let found = target
                    .and_then(|t| attack(&**combat, env, state, (*figure, FigureId(t)), *action));
                previews.attacks.extend(found);
                continue;
            }
            let entry = spots.entry((*figure, *action));
            let entry = entry.or_insert_with(|| Some(BTreeSet::new()));
            match (entry.as_mut(), spot(args)) {
                (Some(set), Some(at)) => {
                    set.insert(at);
                }
                _ => *entry = None,
            }
        }
        // Reach is per figure, so it is asked once whatever the number of its actions.
        let mut reach: BTreeMap<FigureId, Option<Vec<Reach>>> = BTreeMap::new();
        for ((figure, action), targets) in spots {
            let Some(targets) = targets else { continue };
            let stops = reach.entry(figure).or_insert_with(|| {
                let found = env.reachable(state, figure).ok();
                found.map(|r| r.into_iter().filter(|r| !r.occupied).collect())
            });
            let Some(stops) = stops.as_deref() else {
                continue;
            };
            if stops.iter().map(listed).collect::<BTreeSet<_>>() == targets {
                let stops = stops.iter().map(MoveStop::of).collect();
                previews.moves.insert((figure, action), stops);
            }
        }
        previews
    }

    /// The stops `figure` may move to with `action`, if it is a move with a preview.
    pub fn stops(&self, figure: FigureId, action: DefId) -> &[MoveStop] {
        let found = self.moves.get(&(figure, action));
        found.map_or(&[], Vec::as_slice)
    }

    /// The cheapest stop of `figure` moving with `action` on `space`, the one a move without a
    /// rotation takes.
    pub fn stop(&self, figure: FigureId, action: DefId, space: Space) -> Option<&MoveStop> {
        let on = self
            .stops(figure, action)
            .iter()
            .filter(|s| s.space == space);
        on.min_by_key(|s| (s.cost, s.rotation))
    }

    /// The preview of `attacker` attacking `target`.
    pub fn attack(&self, attacker: FigureId, target: FigureId) -> Option<&AttackPreview> {
        let mut all = self.attacks.iter();
        all.find(|a| a.attacker == attacker && a.target == target)
    }
}

/// The preview of one attack, or `None` when the combat slot says it cannot be made.
fn attack(
    combat: &dyn CombatPreview,
    env: &dyn NativeEnv,
    state: &GameState,
    (attacker, target): (FigureId, FigureId),
    action: DefId,
) -> Option<AttackPreview> {
    let pools = combat.pools(state, env, attacker, target).ok()??;
    Some(AttackPreview {
        attacker,
        target,
        action,
        attack: PoolPreview::of(pools.attack, env),
        defense: PoolPreview::of(pools.defense, env),
        ranged: pools.ranged,
        cover: pools.cover,
        sight: pools.sight,
        volume: env.volume(state, target),
    })
}

/// How `ctx.movement.reachable` lists `reach`: its rotation only for a footprint larger than one
/// cell.
fn listed(reach: &Reach) -> Spot {
    let turned = reach.cells.len() > 1;
    (reach.space, turned.then_some(reach.rotation))
}

/// The space an action argument names: `[q, r, z]`, `[q, r, z, o]`, `{ q, r, z }` or
/// `{ q, r, z, o }`.
fn spot(args: &Value) -> Option<Spot> {
    let fields: Vec<Option<&Value>> = match args {
        Value::List(xs) if (3..=4).contains(&xs.len()) => (0..4).map(|i| xs.get(i)).collect(),
        Value::Map(m) => ["q", "r", "z", "o"]
            .iter()
            .map(|k| m.get(&Sym::new(k)))
            .collect(),
        _ => return None,
    };
    let int = |i: usize| fields[i]?.as_int().ok();
    let coord = |i: usize| i32::try_from(int(i)?).ok();
    let space = Space::new(coord(0)?, coord(1)?, u32::try_from(int(2)?).ok()?);
    let rotation = match fields[3] {
        Some(_) => Some(u8::try_from(int(3)?).ok().and_then(Rotation::new)?),
        None => None,
    };
    Some((space, rotation))
}

#[cfg(test)]
mod tests;
