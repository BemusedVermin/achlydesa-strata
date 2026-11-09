//! The fairness report: walking distances from each start zone to each objective and to each
//! other start zone (design §14.1).

use std::collections::BTreeMap;
use std::fmt;

use strata_kernel::{Hex, Space, Sym};
use strata_model::MapDef;
use strata_model::board::CompiledBoard;

use crate::defs::EditorDefs;
use crate::reach::distances;

/// A start zone and the spaces in it a figure can stand on.
pub(crate) struct StartZone {
    /// The zone's name.
    pub(crate) name: Sym,
    /// Its standable spaces, where walks from it begin.
    pub(crate) spaces: Vec<Space>,
}

/// The shortest walk from each start zone to one relic spot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Objective {
    /// The relic spot's name.
    pub spot: Sym,
    /// The relic spot's cell.
    pub at: Hex,
    /// Steps from each start zone, in [`Fairness::start_zones`] order; `None` if unreachable.
    pub steps: Vec<Option<u32>>,
}

impl Objective {
    /// The gap between the nearest and the farthest start zone, or `None` if some start zone
    /// cannot reach the spot or there are no start zones.
    pub fn spread(&self) -> Option<u32> {
        let steps = self.steps.iter().copied().collect::<Option<Vec<u32>>>()?;
        Some(steps.iter().max()? - steps.iter().min()?)
    }
}

/// How evenly a map treats its start zones, measured in steps of a reference one-space figure.
///
/// The figure is one level tall, pays one per step, climbs at most one level, may drop any
/// distance, and ignores terrain that would end its move. Steps are counted from the nearest
/// standable space of a start zone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fairness {
    /// The start zones compared, in name order.
    pub start_zones: Vec<Sym>,
    /// One entry per relic spot, in name order.
    pub objectives: Vec<Objective>,
    /// `between[i][j]` is the steps from start zone `i` to the nearest cell of start zone `j`,
    /// or `None` if no cell of it is reachable.
    pub between: Vec<Vec<Option<u32>>>,
}

impl Fairness {
    /// Measures `map` on its compiled `board` for `starts`, walking from their standable spaces.
    pub(crate) fn measure(
        map: &MapDef,
        board: &CompiledBoard,
        defs: &impl EditorDefs,
        starts: &[StartZone],
    ) -> Self {
        let cells = |zone: &Sym| map.zones.get(zone).map_or(&[][..], Vec::as_slice);
        let reach: Vec<BTreeMap<Hex, u32>> = starts
            .iter()
            .map(|zone| distances(board, defs, &zone.spaces))
            .collect();
        let start_zones: Vec<Sym> = starts.iter().map(|zone| zone.name.clone()).collect();
        let objectives = map.relic_spots.iter().map(|(spot, &at)| Objective {
            spot: spot.clone(),
            at,
            steps: reach.iter().map(|d| d.get(&at).copied()).collect(),
        });
        let between = reach.iter().map(|d| {
            let nearest = |to: &Sym| cells(to).iter().filter_map(|h| d.get(h)).min().copied();
            start_zones.iter().map(nearest).collect()
        });
        Self {
            objectives: objectives.collect(),
            between: between.collect(),
            start_zones,
        }
    }
}

/// Writes a step count, or `-` for unreachable.
struct Steps(Option<u32>);

impl fmt::Display for Steps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(n) => write!(f, "{n}"),
            None => f.write_str("-"),
        }
    }
}

/// Writes `zone steps` pairs separated by commas.
fn pairs<'a>(
    f: &mut fmt::Formatter<'_>,
    pairs: impl Iterator<Item = (&'a Sym, Option<u32>)>,
) -> fmt::Result {
    for (i, (zone, steps)) in pairs.enumerate() {
        let sep = if i == 0 { " " } else { ", " };
        write!(f, "{sep}{zone} {}", Steps(steps))?;
    }
    Ok(())
}

impl fmt::Display for Fairness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start_zones.is_empty() {
            return writeln!(f, "fairness: no start zones to compare");
        }
        writeln!(
            f,
            "fairness: steps for a one-space reference figure (- = unreachable)"
        )?;
        let zones = || self.start_zones.iter();
        for objective in &self.objectives {
            let Hex { q, r } = objective.at;
            write!(f, "  objective `{}` at ({q}, {r}):", objective.spot)?;
            pairs(f, zones().zip(objective.steps.iter().copied()))?;
            writeln!(f, "; spread {}", Steps(objective.spread()))?;
        }
        // With one start zone there is nothing to walk between.
        let between = if self.start_zones.len() > 1 {
            &self.between[..]
        } else {
            &[]
        };
        for (from, row) in zones().zip(between) {
            let others = zones()
                .zip(row.iter().copied())
                .filter(|(to, _)| *to != from);
            write!(f, "  from {from}:")?;
            pairs(f, others)?;
            writeln!(f)?;
        }
        Ok(())
    }
}
