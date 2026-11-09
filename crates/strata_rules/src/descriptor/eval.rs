//! Evaluating compiled descriptors against a view of the game.

use std::collections::BTreeSet;

use strata_kernel::{Hex, Space, Sym, Value};
use strata_model::{Descriptor, FigureId, PlayerId};

use super::{AttackKind, Base, Cmp, Cond, Node, Sel, Val, compile};
use crate::query::{EvalCtx, LogicError, LogicEvaluator};

/// What descriptors may ask about the game. Implemented over the state and the content registry.
pub trait World: Send + Sync {
    /// Figures standing on the board, in any order.
    fn figures(&self) -> Vec<FigureId>;
    /// The anchor space of a figure, or `None` off the board.
    fn location(&self, figure: FigureId) -> Option<Space>;
    /// The player in control of a figure.
    fn controller(&self, figure: FigureId) -> PlayerId;
    /// The team of a player.
    fn team(&self, player: PlayerId) -> u8;
    /// Whether any trait of the figure's unit has this value.
    fn has_trait(&self, figure: FigureId, value: &str) -> bool;
    /// The kind of the figure's unit, such as `squad`.
    fn kind(&self, figure: FigureId) -> Sym;
    /// The current value of a stat for a figure.
    fn stat(&self, figure: FigureId, stat: &str) -> Result<i64, LogicError>;
    /// The terrain a figure on `space` stands on.
    fn terrain_at(&self, space: Space) -> Option<Sym>;
    /// The figure standing on `space`.
    fn occupant(&self, space: Space) -> Option<FigureId>;
    /// Whether `from` has line of sight to `to`. Worlds without a line-of-sight service fail.
    fn in_los(&self, _from: FigureId, _to: FigureId) -> Result<bool, LogicError> {
        Err(LogicError("no line-of-sight service is running".into()))
    }
    /// The name of the innermost active phase.
    fn phase(&self) -> Sym;
    /// Whether a figure on the board is engaged. Worlds without an engagement service fail.
    fn engaged(&self, _figure: FigureId) -> Result<bool, LogicError> {
        Err(LogicError("no engagement service is running".into()))
    }
    /// The attack in progress, if any.
    fn attack(&self) -> Option<AttackView> {
        None
    }
    /// The levels of the standable surfaces at `hex`, bottom to top; empty off the board.
    fn levels(&self, _hex: Hex) -> Vec<u32> {
        Vec::new()
    }
}

/// The attack in progress, as descriptors see it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttackView {
    /// The attacking figure.
    pub attacker: FigureId,
    /// The figure attacked.
    pub target: FigureId,
    /// Whether the target is beyond adjacency.
    pub ranged: bool,
}

/// Evaluates `Logic::Native` descriptors against a [`World`].
///
/// Selectors give a list of figure IDs sorted ascending, conditions a boolean and values an
/// integer. The subject is a figure ID (`Value::Int`) or a space (`{q, r, z}`), the latter for
/// terrain abilities where only `occupant` is meaningful.
pub struct NativeEvaluator<'a> {
    /// The game being asked about.
    pub world: &'a dyn World,
}

enum Subject {
    Figure(FigureId),
    Space(Space),
    /// No subject, as for a hook nothing owns: descriptors that need `self` fail, while `target`
    /// and those that ask only the board still work.
    None,
}

fn fail<T>(msg: impl Into<String>) -> Result<T, LogicError> {
    Err(LogicError(msg.into()))
}

impl LogicEvaluator for NativeEvaluator<'_> {
    fn eval(&self, descriptor: &Descriptor, cx: &EvalCtx<'_>) -> Result<Value, LogicError> {
        let node = compile(descriptor).map_err(|e| LogicError(e.to_string()))?;
        let run = self.run(cx)?;
        Ok(match &node {
            Node::Sel(s) => {
                let ids = run.select(s)?;
                Value::List(
                    ids.into_iter()
                        .map(|f| Value::Int(i64::from(f.0)))
                        .collect(),
                )
            }
            Node::Cond(c) => Value::Bool(run.cond(c)?),
            Node::Val(v) => Value::Int(run.val(v)?),
            Node::Eff(_) => return fail("an effect has no value; it runs through `run_effect`"),
        })
    }
}

impl NativeEvaluator<'_> {
    /// The figures `sel` selects, sorted by ID.
    pub fn select(&self, sel: &Sel, cx: &EvalCtx<'_>) -> Result<Vec<FigureId>, LogicError> {
        Ok(self.run(cx)?.select(sel)?.into_iter().collect())
    }

    /// The value of `val`.
    pub fn value(&self, val: &Val, cx: &EvalCtx<'_>) -> Result<i64, LogicError> {
        self.run(cx)?.val(val)
    }

    fn run(&self, cx: &EvalCtx<'_>) -> Result<Run<'_>, LogicError> {
        let target = match cx.context {
            Value::Int(n) => u32::try_from(*n).ok().map(FigureId),
            _ => None,
        };
        Ok(Run {
            world: self.world,
            subject: self.subject(cx.subject)?,
            target,
        })
    }

    fn subject(&self, value: &Value) -> Result<Subject, LogicError> {
        let coord = |key| value.get(key).and_then(Value::as_int).ok();
        match value {
            Value::Nil => Ok(Subject::None),
            Value::Int(n) => match u32::try_from(*n) {
                Ok(id) => Ok(Subject::Figure(FigureId(id))),
                Err(_) => fail("subject figure ID out of range"),
            },
            Value::Map(_) => match (coord("q"), coord("r"), coord("z")) {
                (Some(q), Some(r), Some(z)) => {
                    match (i32::try_from(q), i32::try_from(r), u32::try_from(z)) {
                        (Ok(q), Ok(r), Ok(z)) => Ok(Subject::Space(Space::new(q, r, z))),
                        _ => fail("subject space out of range"),
                    }
                }
                _ => fail("a space subject needs integer q, r and z"),
            },
            other => fail(format!("a {} cannot be a subject", other.type_name())),
        }
    }
}

struct Run<'a> {
    world: &'a dyn World,
    subject: Subject,
    target: Option<FigureId>,
}

impl Run<'_> {
    fn figure(&self) -> Result<FigureId, LogicError> {
        match self.subject {
            Subject::Figure(f) => Ok(f),
            Subject::Space(_) | Subject::None => {
                fail("this descriptor needs a figure as its subject")
            }
        }
    }

    fn space(&self) -> Result<Space, LogicError> {
        match self.subject {
            Subject::Space(s) => Ok(s),
            Subject::Figure(f) => self
                .world
                .location(f)
                .map_or_else(|| fail(format!("figure {} is not on the board", f.0)), Ok),
            Subject::None => fail("this descriptor needs a subject"),
        }
    }

    fn select(&self, s: &Sel) -> Result<BTreeSet<FigureId>, LogicError> {
        let w = self.world;
        let mut out: BTreeSet<FigureId> = match s.base {
            Base::Own => BTreeSet::from([self.figure()?]),
            Base::Occupant => w.occupant(self.space()?).into_iter().collect(),
            Base::Target => self.target.into_iter().collect(),
            base => {
                let me = self.figure()?;
                let mine = w.controller(me);
                let team = w.team(mine);
                let keep = |f: FigureId| match base {
                    Base::Allies => f != me && w.team(w.controller(f)) == team,
                    Base::Enemies => w.team(w.controller(f)) != team,
                    _ => w.controller(f) == mine,
                };
                w.figures().into_iter().filter(|f| keep(*f)).collect()
            }
        };
        if let Some(t) = &s.trait_value {
            out.retain(|f| w.has_trait(*f, t.as_str()));
        }
        if let Some(k) = &s.kind {
            out.retain(|f| w.kind(*f) == *k);
        }
        if let Some(to) = &s.adjacent_to {
            self.near(&mut out, to, 1, 1)?;
        }
        if let Some(n) = s.within {
            let of = s.of.as_deref().cloned().unwrap_or_else(Sel::own);
            self.near(&mut out, &of, 0, n)?;
        }
        if let Some(from) = &s.in_los {
            self.seen(&mut out, from)?;
        }
        if let Some(want) = s.engaged {
            let mut kept = BTreeSet::new();
            for f in out {
                if w.engaged(f)? == want {
                    kept.insert(f);
                }
            }
            out = kept;
        }
        Ok(out)
    }

    /// Keeps the figures that some figure of `from` has line of sight to.
    fn seen(&self, out: &mut BTreeSet<FigureId>, from: &Sel) -> Result<(), LogicError> {
        let seers = self.select(from)?;
        let mut kept = BTreeSet::new();
        for &target in out.iter() {
            for &seer in seers.iter().filter(|s| **s != target) {
                if self.world.in_los(seer, target)? {
                    kept.insert(target);
                    break;
                }
            }
        }
        *out = kept;
        Ok(())
    }

    /// Keeps the figures whose distance to some figure of `to` lies in `[lo, hi]`.
    fn near(
        &self,
        out: &mut BTreeSet<FigureId>,
        to: &Sel,
        lo: u32,
        hi: u32,
    ) -> Result<(), LogicError> {
        let anchors: Vec<_> = self
            .select(to)?
            .into_iter()
            .filter_map(|f| self.world.location(f))
            .collect();
        out.retain(|f| {
            let Some(at) = self.world.location(*f) else {
                return false;
            };
            anchors
                .iter()
                .any(|a| (lo..=hi).contains(&at.hex().distance(a.hex())))
        });
        Ok(())
    }

    fn first(&self, s: &Sel) -> Result<FigureId, LogicError> {
        match self.select(s)?.into_iter().next() {
            Some(f) => Ok(f),
            None => fail("the selector matched no figure"),
        }
    }

    fn level(&self, f: FigureId) -> Result<i64, LogicError> {
        match self.world.location(f) {
            Some(at) => Ok(i64::from(at.z)),
            None => fail(format!("figure {} is not on the board", f.0)),
        }
    }

    fn cond(&self, c: &Cond) -> Result<bool, LogicError> {
        let all = |cs: &[Cond]| cs.iter().try_fold(true, |acc, c| Ok(acc && self.cond(c)?));
        let any = |cs: &[Cond]| cs.iter().try_fold(false, |acc, c| Ok(acc || self.cond(c)?));
        Ok(match c {
            Cond::Const(b) => *b,
            Cond::All(cs) => all(cs)?,
            Cond::Any(cs) => any(cs)?,
            Cond::None(cs) => !any(cs)?,
            Cond::Not(c) => !self.cond(c)?,
            Cond::Compare(op, a, b) => {
                let (a, b) = (self.val(a)?, self.val(b)?);
                match op {
                    Cmp::Eq => a == b,
                    Cmp::Ne => a != b,
                    Cmp::Lt => a < b,
                    Cmp::Lte => a <= b,
                    Cmp::Gt => a > b,
                    Cmp::Gte => a >= b,
                }
            }
            Cond::Count { of, min, max } => {
                let n = i64::try_from(self.select(of)?.len()).unwrap_or(i64::MAX);
                n >= *min && max.is_none_or(|m| n <= m)
            }
            Cond::Phase(p) => self.world.phase() == *p,
            Cond::OnTerrain { terrain, who } => {
                let who = self.select(who)?;
                let on = |f: &FigureId| {
                    let at = self.world.location(*f);
                    at.and_then(|s| self.world.terrain_at(s)).as_ref() == Some(terrain)
                };
                !who.is_empty() && who.iter().all(on)
            }
            Cond::HigherThan { than, by } => {
                let me = self.level(self.figure()?)?;
                let others = self.select(than)?;
                let mut diffs = others.iter().map(|f| Ok(me - self.level(*f)?));
                !others.is_empty()
                    && diffs
                        .try_fold(true, |acc, d: Result<i64, LogicError>| Ok(acc && d? >= *by))?
            }
            Cond::Engaged(who) => {
                let who = self.select(who)?;
                let mut all = !who.is_empty();
                for f in &who {
                    all = all && self.world.engaged(*f)?;
                }
                all
            }
            Cond::InLos { from, to } => {
                let mut seen = self.select(to)?;
                self.seen(&mut seen, from)?;
                !seen.is_empty()
            }
            Cond::AttackKind(kind) => self
                .world
                .attack()
                .is_some_and(|a| a.ranged == (*kind == AttackKind::Ranged)),
        })
    }

    fn val(&self, v: &Val) -> Result<i64, LogicError> {
        let all = |vs: &[Val]| {
            vs.iter()
                .map(|v| self.val(v))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(match v {
            Val::Lit(n) => *n,
            Val::Stat { stat, of } => self.world.stat(self.first(of)?, stat.as_str())?,
            Val::Count(of) => i64::try_from(self.select(of)?.len()).unwrap_or(i64::MAX),
            Val::Distance(to) => {
                let there = self.world.location(self.first(to)?);
                match there {
                    Some(at) => i64::from(self.space()?.hex().distance(at.hex())),
                    None => return fail("the target is not on the board"),
                }
            }
            Val::LevelDifference(to) => i64::from(self.space()?.z) - self.level(self.first(to)?)?,
            Val::Sum(vs) => all(vs)?.iter().sum(),
            Val::Min(vs) => all(vs)?.into_iter().min().unwrap_or(0),
            Val::Max(vs) => all(vs)?.into_iter().max().unwrap_or(0),
        })
    }
}
