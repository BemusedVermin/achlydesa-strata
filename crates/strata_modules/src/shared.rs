//! Small helpers that more than one module of this crate needs.

use strata_kernel::{Sym, Value, ValueError};
use strata_model::{DieKind, FigureId, FigureState};
use strata_rules::{Halt, NativeEnv};

/// The names under `key` of a descriptor, written as one string or a list of them. A missing
/// key, nil or an empty table lists none.
pub(crate) fn names(descriptor: &Value, key: &str) -> Result<Vec<Sym>, ValueError> {
    match descriptor.get(key) {
        Err(_) | Ok(Value::Nil) => Ok(Vec::new()),
        Ok(Value::Map(map)) if map.is_empty() => Ok(Vec::new()),
        Ok(Value::Str(one)) => Ok(vec![one.clone()]),
        Ok(list) => {
            let items = list.as_list()?.iter();
            items.map(|v| v.as_str().map(Sym::new)).collect()
        }
    }
}

/// The figure's ID as query subjects, script values and action arguments carry it.
pub(crate) fn id_value(figure: FigureId) -> Value {
    Value::Int(i64::from(figure.0))
}

/// How many levels `figure` spans, at least one.
pub(crate) fn height(env: &dyn NativeEnv, figure: &FigureState) -> u32 {
    let def = env.figure_def(&figure.def);
    def.map_or(1, |d| u32::from(d.height).max(1))
}

/// Whether the level ranges `[a, a + a_height)` and `[b, b + b_height)` share a level.
pub(crate) fn overlaps(a: u32, a_height: u32, b: u32, b_height: u32) -> bool {
    a < b.saturating_add(b_height) && b < a.saturating_add(a_height)
}

/// Checks that the die `die` exists and has every face in `faces`, each paired with the config
/// key that names it. The error names `module` and the config key at fault.
pub(crate) fn check_faces(
    env: &dyn NativeEnv,
    module: &str,
    die: &str,
    faces: &[(&str, &Sym)],
) -> Result<(), Halt> {
    let bad = |key: &str, e: String| Halt::reject(format!("{module} `{key}`: {e}"));
    let Some((_, kind)) = env.die(die) else {
        return Err(bad("die", format!("unknown die `{die}`")));
    };
    for &(key, face) in faces {
        if !matches!(&kind, DieKind::Faces(found) if found.contains(face)) {
            return Err(bad(key, format!("`{face}` is not a face of die `{die}`")));
        }
    }
    Ok(())
}
