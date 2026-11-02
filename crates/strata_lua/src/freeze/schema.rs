//! Which record fields reference other definitions, and which hold a bare `ScriptRef`.

/// Where a cross-reference sits.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Site {
    /// A string that names another definition.
    Value,
    /// A map key that names another definition.
    Key,
}

/// Reference fields by kind. A route is `/`-separated and `*` stands for any list index or map key.
const REFERENCES: &[(&str, &str, Site)] = &[
    ("unit", "faction", Site::Value),
    ("unit", "stats/*", Site::Key),
    ("unit", "figures/*/footprint", Site::Value),
    ("terrain", "abilities/*", Site::Value),
    ("tile", "terrain", Site::Value),
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
    let mut segments = pattern.split('/');
    let same = route
        .iter()
        .all(|part| segments.next().is_some_and(|s| s == "*" || s == part));
    same && segments.next().is_none()
}

/// The site of the reference at `route` in a `kind` record, if any.
pub(super) fn reference(kind: &str, route: &[String]) -> Option<Site> {
    REFERENCES
        .iter()
        .find(|(k, pattern, _)| *k == kind && matches(pattern, route))
        .map(|(.., site)| *site)
}

/// Whether a function at `route` is a bare `ScriptRef`.
pub(super) fn bare_script(kind: &str, route: &[String]) -> bool {
    BARE_SCRIPTS
        .iter()
        .any(|(k, pattern)| *k == kind && matches(pattern, route))
}
