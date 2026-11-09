//! What the client needs to draw each kind of figure, read from the content on the simulation
//! thread (design §13.4, §13.8).
//!
//! This is presentation data: the client resolves `visual` through the packs' `assets.lua`, and
//! falls back to a generated sprite sized from `hit_zone` and `footprint`.

use strata_content::ContentRegistry;
use strata_kernel::{DefId, Hex, PackId, Rotation, Sym};
use strata_model::{FactionDef, FootprintDef, UnitDef};

/// How one figure entry of one unit definition is drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FigureLook {
    /// The unit definition.
    pub unit: DefId,
    /// The figure's name within the unit.
    pub figure: Sym,
    /// The pack that defines the unit; its pixel scale sizes a generated sprite.
    pub pack: PackId,
    /// The sprite key, such as `sprite.test_soldier`.
    pub visual: Option<Sym>,
    /// `(bottom, top)` of the figure's hit zone, in sub-levels.
    pub hit_zone: (u16, u16),
    /// The footprint's cells around the anchor at rotation 0, anchor first.
    pub footprint: Vec<Hex>,
    /// The unit's faction color, such as `#B5893A`.
    pub color: Option<String>,
}

/// A look for every figure entry of every unit in `registry`, in unit then figure order.
pub fn figure_looks(registry: &ContentRegistry) -> Vec<FigureLook> {
    let mut looks = Vec::new();
    for (unit_id, full, unit) in registry.iter::<UnitDef>() {
        let pack = PackId::new(full.split_once(':').map_or("", |(p, _)| p));
        let faction = unit.faction.and_then(|f| registry.get::<FactionDef>(f));
        for figure in &unit.figures {
            let footprint = registry.get::<FootprintDef>(figure.footprint);
            let cells = footprint.map_or_else(
                || vec![Hex::ORIGIN],
                |f| f.cells_at(Hex::ORIGIN, Rotation::default()),
            );
            looks.push(FigureLook {
                unit: unit_id,
                figure: figure.id.clone(),
                pack: pack.clone(),
                visual: figure.visual.clone(),
                hit_zone: figure.los.hit_zone,
                footprint: cells,
                color: faction.map(|f| f.color.clone()),
            });
        }
    }
    looks
}
