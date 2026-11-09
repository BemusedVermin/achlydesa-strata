//! `AssetsPlugin`: resolves each figure's visual key through the packs' `assets.lua`, and
//! generates a fallback sprite where there is none (design §13.3, §13.4).
//!
//! It also copies where the figures stand from the latest view into [`FigurePoses`], which the
//! render crate's `FigurePlugin` draws.

use std::collections::BTreeMap;
use std::path::Path;

use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use strata_kernel::{Space, Sym};
use strata_lua::PackSource;
use strata_model::{FigureId, FigureStatus, GameState};
use strata_render::{FigurePose, FigurePoses, FigureSprite, FigureSprites};
use strata_scene::{Orientation, SpriteCatalog, fallback_sprite, parse_color};
use strata_session::FigureLook;

use crate::sim_link::LatestView;

/// The color of a fallback for a unit without a faction, before its seat's tint.
const NEUTRAL: [u8; 3] = [200, 200, 200];

/// What the simulation said about the packs and figure kinds, when the board arrived.
#[derive(Resource, Debug)]
pub struct BoardLooks {
    /// How the board's hexes are laid out.
    pub orientation: Orientation,
    /// The packs in load order.
    pub packs: Vec<PackSource>,
    /// How each kind of figure is drawn.
    pub looks: Vec<FigureLook>,
}

/// Loads sprites when the board arrives and keeps [`FigurePoses`] current.
pub struct AssetsPlugin;

impl Plugin for AssetsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FigureSprites>()
            .init_resource::<FigurePoses>()
            .add_systems(Update, (load_sprites, pose_figures));
    }
}

/// Builds every figure kind's sprite from the packs' `assets.lua` once the board arrives.
fn load_sprites(
    looks: Option<Res<BoardLooks>>,
    mut images: ResMut<Assets<Image>>,
    mut sprites: ResMut<FigureSprites>,
) {
    let Some(looks) = looks.filter(|l| l.is_changed()) else {
        return;
    };
    let (catalog, errors) = SpriteCatalog::load(&looks.packs);
    for error in errors {
        warn!("{error}; its figures use generated sprites");
    }
    let mut add = |image| images.add(image);
    sprites.0 = looks
        .looks
        .iter()
        .map(|look| {
            let key = (look.unit, look.figure.clone());
            (
                key,
                figure_sprite(&catalog, look, looks.orientation, &mut add),
            )
        })
        .collect();
}

/// The sprite for `look`: its declared sheet when that loads and is big enough, a generated
/// fallback otherwise. `add` stores an image and hands back its handle.
pub fn figure_sprite(
    catalog: &SpriteCatalog,
    look: &FigureLook,
    orientation: Orientation,
    add: &mut impl FnMut(Image) -> Handle<Image>,
) -> FigureSprite {
    if let Some(key) = &look.visual {
        match declared(catalog, key.as_str()) {
            Ok((sprite, image)) => {
                return FigureSprite {
                    sheet: add(image),
                    ..sprite
                };
            }
            Err(why) => warn!("`{key}`: {why}; using a generated sprite"),
        }
    }
    let color = look.color.as_deref().and_then(parse_color);
    let key = look.visual.clone().unwrap_or_else(|| Sym::new("fallback"));
    let scale = catalog.scale(&look.pack);
    let rgb = color.unwrap_or(NEUTRAL);
    let art = fallback_sprite(key, look.hit_zone, &look.footprint, orientation, rgb, scale);
    let size = Extent3d {
        width: art.size.x,
        height: art.size.y,
        depth_or_array_layers: 1,
    };
    let mut image = Image::new(
        size,
        TextureDimension::D2,
        art.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::nearest();
    FigureSprite {
        def: art.def,
        scale,
        sheet: add(image),
        sheet_size: art.size,
        base: Some(rgb),
        tint_by_seat: color.is_none(),
    }
}

/// The declared sprite `key` with its sheet decoded, or why it cannot be used.
fn declared(catalog: &SpriteCatalog, key: &str) -> Result<(FigureSprite, Image), String> {
    let entry = catalog.sprite(key).ok_or("no pack declares it")?;
    let image = load_png(&entry.sheet)?;
    let size = image.size();
    let needed = entry.def.sheet_size();
    if size.x < needed.x || size.y < needed.y {
        return Err(format!(
            "{} is {}x{}, smaller than the {}x{} its frames need",
            entry.sheet.display(),
            size.x,
            size.y,
            needed.x,
            needed.y
        ));
    }
    let sprite = FigureSprite {
        def: entry.def.clone(),
        scale: catalog.scale(&entry.pack),
        sheet: Handle::default(),
        sheet_size: size,
        base: None,
        tint_by_seat: false,
    };
    Ok((sprite, image))
}

/// Decodes a PNG sheet for pixel art: sRGB, nearest-neighbor filtering.
fn load_png(path: &Path) -> Result<Image, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Image::from_buffer(
        &bytes,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        true,
        ImageSampler::nearest(),
        RenderAssetUsages::default(),
    )
    .map_err(|e| format!("{}: {e}", path.display()))
}

/// Copies the figures on the board from the latest view whenever it changes.
fn pose_figures(view: Res<LatestView>, mut poses: ResMut<FigurePoses>) {
    if !view.is_changed() {
        return;
    }
    let Some(view) = &view.0 else { return };
    let next = figure_poses(&view.state);
    if poses.0 != next {
        poses.0 = next;
    }
}

/// Every figure standing on the board, with the spaces its footprint covers.
pub fn figure_poses(state: &GameState) -> Vec<FigurePose> {
    let mut cells: BTreeMap<FigureId, Vec<Space>> = BTreeMap::new();
    for (space, figure) in &state.board_changes.occupancy {
        cells.entry(*figure).or_default().push(*space);
    }
    let on_board = state.figures.iter().enumerate();
    let on_board = on_board.filter(|(_, f)| f.status == FigureStatus::OnBoard);
    on_board
        .filter_map(|(index, figure)| {
            let anchor = figure.location?;
            let id = FigureId(index as u32);
            Some(FigurePose {
                id,
                look: (figure.def.unit, figure.def.figure.clone()),
                anchor,
                cells: cells.remove(&id).unwrap_or_else(|| vec![anchor]),
                rotation: figure.rotation,
                seat: figure.controller.0,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
