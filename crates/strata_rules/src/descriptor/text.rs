//! Card-text phrases for descriptors (design §8.4).

use super::{Base, Cmp, Cond, Sel, Val};

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
    out
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
