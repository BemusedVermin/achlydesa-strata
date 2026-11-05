//! Live validation of a map: the board, inventory and zone findings of design §14.1.

use std::collections::BTreeMap;

use strata_kernel::{DefId, Hex, Sym};
use strata_model::MapDef;
use strata_model::board::{BoardError, CompiledBoard};

use crate::defs::EditorDefs;
use crate::finding::{Finding, FindingKind, Severity};
use crate::standing::standable_spaces;

/// Zones whose names start with this are start zones, as in the design's `start_a`.
pub const START_ZONE_PREFIX: &str = "start_";

/// What validation is checked against, beyond the map itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ValidationOptions {
    /// Figures in the largest army that deploys in one start zone; `None` skips the size check.
    pub army_size: Option<u32>,
}

/// Everything validation found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validation {
    /// The findings: board errors first, then inventory, then zones.
    pub findings: Vec<Finding>,
}

impl Validation {
    /// How many findings have `severity`.
    pub fn count(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }
}

/// Validates `map`, given the result of compiling it.
///
/// Zone checks need a board, so a map that does not compile reports only its board errors and
/// inventory findings.
pub(crate) fn validate(
    map: &MapDef,
    board: Result<&CompiledBoard, &[BoardError]>,
    defs: &impl EditorDefs,
    options: &ValidationOptions,
) -> Validation {
    let mut findings = Vec::new();
    for error in board.err().unwrap_or_default() {
        let (kind, cell) = Finding::from_board(error);
        add_cell(&mut findings, kind, cell);
    }
    check_inventory(map, defs, &mut findings);
    if let Ok(board) = board {
        check_zones(map, board, defs, options, &mut findings);
    }
    Validation { findings }
}

/// Adds `cell` to the finding of `kind`, creating it if needed.
fn add_cell(findings: &mut Vec<Finding>, kind: FindingKind, cell: Hex) {
    match findings.iter_mut().find(|f| f.kind == kind) {
        Some(found) if !found.cells.contains(&cell) => found.cells.push(cell),
        Some(_) => {}
        None => findings.push(Finding::new(kind, vec![cell])),
    }
}

/// Reports a missing inventory and every tile or feature used more often than it allows.
fn check_inventory(map: &MapDef, defs: &impl EditorDefs, findings: &mut Vec<Finding>) {
    let Some(name) = &map.constraints.inventory else {
        return;
    };
    let Some(inventory) = defs.inventory(name) else {
        let kind = FindingKind::UnknownInventory { name: name.clone() };
        findings.push(Finding::new(kind, Vec::new()));
        return;
    };
    let pieces = map.pieces.iter().map(|p| (&p.piece, p.at));
    let tile = |name: &Sym| defs.tile_id(name);
    overruns(pieces, &inventory.tiles, tile, findings);
    let features = map.features.iter().map(|f| (&f.feature, f.at));
    let feature = |name: &Sym| defs.feature_id(name);
    overruns(features, &inventory.features, feature, findings);
}

/// What a name counts against: its definition, or the name itself when it does not resolve.
type Key<'a> = Result<DefId, &'a Sym>;

/// Reports each definition in `placed` used more often than `caps` allows, at the anchors using
/// it. Names are compared by what `resolve` makes of them, so `grass` and `pack:grass` are one
/// tile; the finding names it as the map first spells it.
fn overruns<'a>(
    placed: impl Iterator<Item = (&'a Sym, Hex)>,
    caps: &'a BTreeMap<Sym, u32>,
    resolve: impl Fn(&Sym) -> Option<DefId>,
    findings: &mut Vec<Finding>,
) {
    let key = |name: &'a Sym| -> Key<'a> { resolve(name).ok_or(name) };
    let mut available: BTreeMap<Key<'a>, u32> = BTreeMap::new();
    for (name, &cap) in caps {
        *available.entry(key(name)).or_default() += cap;
    }
    let mut uses: BTreeMap<Key<'a>, (&Sym, Vec<Hex>)> = BTreeMap::new();
    for (name, at) in placed {
        uses.entry(key(name))
            .or_insert((name, Vec::new()))
            .1
            .push(at);
    }
    for (key, (name, cells)) in uses {
        let available = available.get(&key).copied().unwrap_or(0);
        let used = u32::try_from(cells.len()).unwrap_or(u32::MAX);
        if used > available {
            let name = name.clone();
            let kind = FindingKind::InventoryOverrun {
                name,
                used,
                available,
            };
            findings.push(Finding::new(kind, cells));
        }
    }
}

/// Reports unstandable zone cells, small start zones and a map with no start zone.
fn check_zones(
    map: &MapDef,
    board: &CompiledBoard,
    defs: &impl EditorDefs,
    options: &ValidationOptions,
    findings: &mut Vec<Finding>,
) {
    let mut starts = 0;
    for (zone, cells) in &map.zones {
        let unstandable = |&&hex: &&Hex| standable_spaces(board, defs, &[hex]).is_empty();
        let bad: Vec<Hex> = cells.iter().filter(unstandable).copied().collect();
        if !bad.is_empty() {
            let kind = FindingKind::UnstandableZone { zone: zone.clone() };
            findings.push(Finding::new(kind, bad));
        }
        if !zone.as_str().starts_with(START_ZONE_PREFIX) {
            continue;
        }
        starts += 1;
        let spaces = u32::try_from(standable_spaces(board, defs, cells).len()).unwrap_or(u32::MAX);
        if let Some(army) = options.army_size
            && spaces < army
        {
            let zone = zone.clone();
            let kind = FindingKind::StartZoneTooSmall { zone, spaces, army };
            findings.push(Finding::new(kind, cells.clone()));
        }
    }
    if starts == 0 {
        findings.push(Finding::new(FindingKind::NoStartZones, Vec::new()));
    }
}
