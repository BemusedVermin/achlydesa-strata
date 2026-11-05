//! Which record fields reference other definitions, and which hold a bare `ScriptRef`.

/// Where a cross-reference sits.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Site {
    /// A string that names another definition.
    Value,
    /// A map key that names another definition.
    Key,
}

/// Reference fields: the record kind, the route, where the reference sits and the kind it must
/// name. A route is `/`-separated and `*` stands for any list index or map key.
const REFERENCES: &[(&str, &str, Site, &str)] = &[
    ("unit", "faction", Site::Value, "faction"),
    ("unit", "stats/*", Site::Key, "stat"),
    ("unit", "figures/*/footprint", Site::Value, "footprint"),
    ("terrain", "abilities/*", Site::Value, "ability"),
    ("tile", "terrain", Site::Value, "terrain"),
];

/// Fields typed as a bare `ScriptRef`. A function anywhere else is a `Logic::Script`.
const BARE_SCRIPTS: &[(&str, &str)] = &[("rule_module", "register"), ("scenario", "after/*")];

/// Kinds whose typed struct has no `id` field.
const WITHOUT_ID: [&str; 3] = ["ability", "army", "test"];
/// Kinds whose typed struct has no `source` field.
const WITHOUT_SOURCE: [&str; 2] = ["ability", "army"];

pub(super) fn has_id(kind: &str) -> bool {
    !WITHOUT_ID.contains(&kind)
}

pub(super) fn has_source(kind: &str) -> bool {
    !WITHOUT_SOURCE.contains(&kind)
}

/// Whether `route` matches `pattern`, segment by segment.
fn matches(pattern: &str, route: &[String]) -> bool {
    let segments: Vec<&str> = pattern.split('/').collect();
    matches_parts(&segments, route)
}

/// Whether `route` matches the segments of `pattern`, where `*` matches any one segment.
fn matches_parts(pattern: &[&str], route: &[String]) -> bool {
    pattern.len() == route.len() && pattern.iter().zip(route).all(|(p, r)| *p == "*" || p == r)
}

/// The site and target kind of the reference at `route` in a `kind` record, if any.
pub(super) fn reference(kind: &str, route: &[String]) -> Option<(Site, &'static str)> {
    REFERENCES
        .iter()
        .find(|(k, pattern, ..)| *k == kind && matches(pattern, route))
        .map(|(_, _, site, target)| (*site, *target))
}

/// Fields typed as a `Value`, which Lua authors write as plain data.
const VALUES: &[(&str, &str)] = &[
    ("rule_module", "config/*/default"),
    ("test", "steps/*/Custom/payload"),
    ("test", "steps/*/Action/args"),
];

pub(super) fn is_value(kind: &str, route: &[String]) -> bool {
    VALUES
        .iter()
        .any(|(k, pattern)| *k == kind && matches(pattern, route))
}

/// Where abilities sit in each record kind: an `ability` record is one, and units and relics
/// hold lists of them.
const ABILITIES: &[(&str, &[&str])] = &[
    ("ability", &[]),
    ("unit", &["abilities", "*"]),
    ("relic", &["grants", "abilities", "*"]),
];
/// The lists of an ability that hold hooks, such as `modifiers`.
const HOOK_LISTS: [&str; 5] = [
    "modifiers",
    "actions",
    "triggers",
    "replacements",
    "reactions",
];
/// Where a module's registrations sit: each is a hook, as in an ability's lists.
const REGISTERED: [&str; 3] = ["~registered", "*", "def"];
/// The fields of a hook typed as `Logic`. A hook without one of these rejects it as unknown,
/// so one list serves every kind of hook.
const LOGIC_FIELDS: [&str; 8] = [
    "select", "when", "value", "targets", "resolve", "filter", "effect", "replace",
];

/// Whether `route` holds a hook: an entry of an ability's hook list, or a module's registration.
fn is_hook(kind: &str, route: &[String]) -> bool {
    if kind == "rule_module" {
        return matches_parts(&REGISTERED, route);
    }
    let Some((ability, [list, _])) = route.split_last_chunk::<2>() else {
        return false;
    };
    let site = |(k, at): &(&str, &[&str])| *k == kind && matches_parts(at, ability);
    HOOK_LISTS.contains(&list.as_str()) && ABILITIES.iter().any(site)
}

/// Whether `route` is a field typed as `Logic`, where plain data is a native descriptor: a
/// hook's `select`, `when`, `value` and so on, or an `on_claim` relic's `effect`.
pub(super) fn is_logic(kind: &str, route: &[String]) -> bool {
    let Some((field, hook)) = route.split_last() else {
        return false;
    };
    let relic_effect = kind == "relic" && hook.is_empty() && field == "effect";
    relic_effect || (LOGIC_FIELDS.contains(&field.as_str()) && is_hook(kind, hook))
}

/// Whether a function at `route` is a bare `ScriptRef`.
pub(super) fn bare_script(kind: &str, route: &[String]) -> bool {
    BARE_SCRIPTS
        .iter()
        .any(|(k, pattern)| *k == kind && matches(pattern, route))
}
