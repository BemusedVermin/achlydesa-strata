//! Card-text phrases for descriptors (design §8.4).

use super::{AttackKind, Base, Cmp, Cond, Eff, Sel, Val};

fn join<T>(items: &[T], word: &str, f: fn(&T) -> String) -> String {
    items.iter().map(f).collect::<Vec<_>>().join(word)
}

/// Phrases a selector, such as `adjacent friendly soldier figures`.
pub(super) fn sel(s: &Sel) -> String {
    let qualifiers = [&s.trait_value, &s.kind]
        .into_iter()
        .flatten()
        .map(|q| format!("{} ", q.as_str()))
        .collect::<String>();
    let noun = match s.base {
        Base::Own => "this figure".to_owned(),
        Base::Occupant => "the occupant".to_owned(),
        Base::Target => "the target".to_owned(),
        Base::Allies => format!("friendly {qualifiers}figures"),
        Base::Enemies => format!("enemy {qualifiers}figures"),
        Base::MyUnits => format!("your {qualifiers}figures"),
    };
    let mut out = noun;
    match s.adjacent_to.as_deref() {
        Some(to) if to.base == Base::Own => out = format!("adjacent {out}"),
        Some(to) => out = format!("{out} adjacent to {}", sel(to)),
        None => {}
    }
    if let Some(n) = s.within {
        let of = s.of.as_deref().map_or("this figure".to_owned(), sel);
        out = format!("{out} within {n} of {of}");
    }
    match s.in_los.as_deref() {
        Some(from) if from.base == Base::Own => out = format!("{out} in line of sight"),
        Some(from) => out = format!("{out} in line of sight of {}", sel(from)),
        None => {}
    }
    match (s.engaged, singular(s)) {
        (Some(true), true) => out = format!("{out} while engaged"),
        (Some(false), true) => out = format!("{out} while not engaged"),
        (Some(true), false) => out = format!("{out} that are engaged"),
        (Some(false), false) => out = format!("{out} that are not engaged"),
        (None, _) => {}
    }
    out
}

/// Whether a selector names one figure, for verb agreement.
fn singular(s: &Sel) -> bool {
    matches!(s.base, Base::Own | Base::Occupant | Base::Target)
}

/// `is` or `are`, agreeing with the selector.
fn be(s: &Sel) -> &'static str {
    if singular(s) { "is" } else { "are" }
}

/// `n` of `noun`, as `1 wound` or `2 wounds`; a computed amount reads `wounds equal to ...`.
fn count(n: &Val, noun: &str) -> String {
    match n {
        Val::Lit(1) => format!("1 {noun}"),
        Val::Lit(n) => format!("{n} {noun}s"),
        other => format!("{noun}s equal to {}", val(other)),
    }
}

/// Phrases a condition, such as `at least 2 adjacent enemy figures`.
pub(super) fn cond(c: &Cond) -> String {
    match c {
        Cond::Const(true) => "always".to_owned(),
        Cond::Const(false) => "never".to_owned(),
        Cond::All(cs) => join(cs, " and ", cond),
        Cond::Any(cs) => join(cs, " or ", cond),
        Cond::None(cs) => format!("none of: {}", join(cs, ", ", cond)),
        Cond::Not(c) => format!("not ({})", cond(c)),
        Cond::Compare(op, a, b) => {
            let word = match op {
                Cmp::Eq => "equals",
                Cmp::Ne => "differs from",
                Cmp::Lt => "is less than",
                Cmp::Lte => "is at most",
                Cmp::Gt => "is more than",
                Cmp::Gte => "is at least",
            };
            format!("{} {word} {}", val(a), val(b))
        }
        Cond::Count { of, min, max: None } => format!("at least {min} of {}", sel(of)),
        Cond::Count {
            of,
            min,
            max: Some(max),
        } => {
            format!("between {min} and {max} of {}", sel(of))
        }
        Cond::Phase(p) => format!("during the {} phase", p.as_str()),
        Cond::OnTerrain { terrain, who } => format!("{} on {}", sel(who), terrain.as_str()),
        Cond::HigherThan { than, by: 1 } => format!("higher than {}", sel(than)),
        Cond::HigherThan { than, by } => format!("{by} levels higher than {}", sel(than)),
        Cond::Engaged(who) => format!("{} {} engaged", sel(who), be(who)),
        Cond::InLos { from, to } => {
            let has = if singular(from) { "has" } else { "have" };
            format!("{} {has} line of sight to {}", sel(from), sel(to))
        }
        Cond::AttackKind(AttackKind::Melee) => "during a melee attack".to_owned(),
        Cond::AttackKind(AttackKind::Ranged) => "during a ranged attack".to_owned(),
    }
}

/// Phrases an effect as an instruction, such as `place 1 wound on the target`.
pub(super) fn eff(e: &Eff) -> String {
    match e {
        Eff::Wound { who, amount } => format!("place {} on {}", count(amount, "wound"), sel(who)),
        Eff::Heal { who, amount } => {
            format!("remove up to {} from {}", count(amount, "wound"), sel(who))
        }
        Eff::Place { who, within, of } => format!(
            "place {} on an empty space within {} of {}",
            sel(who),
            val(within),
            sel(of)
        ),
        Eff::Remove { who } => format!("remove {} from the game", sel(who)),
        Eff::Move { who, up_to } => format!("move {} up to {}", sel(who), count(up_to, "space")),
        Eff::GrantSlot {
            slot,
            count: n,
            who,
        } => {
            let slot = format!("extra {} action", slot.as_str());
            format!("{} may take {}", sel(who), count(n, &slot))
        }
        Eff::AddMarker {
            marker,
            count: n,
            rounds,
            who,
        } => {
            let marker = format!("{} marker", marker.as_str());
            let lasting = match rounds {
                None => String::new(),
                Some(1) => " until the end of the round".to_owned(),
                Some(r) => format!(" for {r} rounds"),
            };
            format!("place {} on {}{lasting}", count(n, &marker), sel(who))
        }
        Eff::TakeControl { who } => format!("take control of {}", sel(who)),
        Eff::CancelWounds => "the attack deals no wounds".to_owned(),
        Eff::ExcessBlocksWoundAttacker => {
            "the attacker takes 1 wound for each block beyond the hits".to_owned()
        }
    }
}

/// Phrases a value, such as `the defense of this figure`.
pub(super) fn val(v: &Val) -> String {
    match v {
        Val::Lit(n) => n.to_string(),
        Val::Stat { stat, of } => format!("the {} of {}", stat.as_str(), sel(of)),
        Val::Count(of) => format!("the number of {}", sel(of)),
        Val::Distance(to) => format!("the distance to {}", sel(to)),
        Val::LevelDifference(to) => format!("the level difference to {}", sel(to)),
        Val::Sum(vs) => format!("the sum of {}", join(vs, ", ", val)),
        Val::Min(vs) => format!("the lowest of {}", join(vs, ", ", val)),
        Val::Max(vs) => format!("the highest of {}", join(vs, ", ", val)),
    }
}
