//! The fairness report: path lengths from each start zone to each objective (design §14.1).

use std::fmt;

use strata_kernel::{Hex, Sym};
use strata_model::MapDef;
use strata_model::board::CompiledBoard;

use crate::defs::EditorDefs;
use crate::reach::{distances, standable_spaces};

/// The shortest walk from each start zone to one relic spot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Objective {
    /// The relic spot's name.
    pub spot: Sym,
    /// The relic spot's cell.
    pub at: Hex,
    /// Steps from each start zone, in [`Fairness::start_zones`] order; `None` if unreachable.
    pub steps: Vec<Option<i64>>,
}

impl Objective {
    /// The gap between the nearest and farthest start zone, or `None` if any cannot reach it.
    pub fn spread(&self) -> Option<i64> {
        let steps: Option<Vec<i64>> = self.steps.iter().copied().collect();
        let steps = steps?;
        Some(steps.iter().max()? - steps.iter().min()?)
    }
}

/// How evenly a map treats its start zones, measured with a reference one-space figure that
/// walks one step per hex, climbs at most one level, and ignores move-ending terrain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fairness {
    /// The start zones compared, in name order.
    pub start_zones: Vec<Sym>,
    /// One entry per relic spot, in name order.
    pub objectives: Vec<Objective>,
    /// `between[i][j]` is the steps from start zone `i` to the nearest cell of zone `j`.
    pub between: Vec<Vec<Option<i64>>>,
}

impl Fairness {
    /// Measures `map` on its compiled `board` for the named start zones.
    pub(crate) fn measure(
        map: &MapDef,
        board: &CompiledBoard,
        defs: &impl EditorDefs,
        start_zones: Vec<Sym>,
    ) -> Self {
        let cells = |zone: &Sym| map.zones.get(zone).map_or(&[][..], Vec::as_slice);
        let reach: Vec<_> = start_zones
            .iter()
            .map(|zone| distances(board, defs, &standable_spaces(board, defs, cells(zone))))
            .collect();
        let objectives = map.relic_spots.iter().map(|(spot, &at)| Objective {
            spot: spot.clone(),
            at,
            steps: reach.iter().map(|d| d.get(&at).copied()).collect(),
        });
        let between = reach.iter().map(|d| {
            let zone_steps = |to: &Sym| cells(to).iter().filter_map(|h| d.get(h)).min().copied();
            start_zones.iter().map(zone_steps).collect()
        });
        Self {
            objectives: objectives.collect(),
            between: between.collect(),
            start_zones,
        }
    }
}

/// Writes `steps`, or `-` for unreachable.
struct Steps(Option<i64>);

impl fmt::Display for Steps {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(n) => write!(f, "{n}"),
            None => f.write_str("-"),
        }
    }
}

impl fmt::Display for Fairness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "fairness: steps for a one-space reference figure (- = unreachable)"
        )?;
        let zones = || self.start_zones.iter();
        for objective in &self.objectives {
            let Hex { q, r } = objective.at;
            write!(f, "  objective `{}` at ({q}, {r}):", objective.spot)?;
            for (i, (zone, &steps)) in zones().zip(&objective.steps).enumerate() {
                let sep = if i == 0 { " " } else { ", " };
                write!(f, "{sep}{zone} {}", Steps(steps))?;
            }
            writeln!(f, "; spread {}", Steps(objective.spread()))?;
        }
        for (from, row) in zones().zip(&self.between) {
            write!(f, "  from {from}:")?;
            let others = zones().zip(row).filter(|(to, _)| *to != from);
            for (i, (to, &steps)) in others.enumerate() {
                let sep = if i == 0 { " " } else { ", " };
                write!(f, "{sep}{to} {}", Steps(steps))?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
