//! `PreviewPlugin`: draws what the client says a legal move or attack would do (design §13.6).
//!
//! The client fills [`PreviewMarks`] from the previews the simulation thread computed; this
//! plugin only turns them into line meshes with the geometry in [`strata_scene`]. Cost labels and
//! the odds table are text, so the client's UI draws them.

use std::collections::BTreeMap;

use bevy::asset::RenderAssetUsages;
use bevy::mesh::PrimitiveTopology;
use bevy::prelude::*;
use strata_kernel::{Hex, Seg3, Space};
use strata_scene::{Line, Orientation, ring, route, segment, volume_outline};

use crate::camera::ClientBoard;

/// A space a move may stop on, with what stopping there causes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopMark {
    /// The space.
    pub space: Space,
    /// Entering it ends the move.
    pub stops_here: bool,
    /// Moving there ends in a fall.
    pub falls: bool,
    /// Moving there gives an enemy a reprisal.
    pub provokes_reprisal: bool,
}

/// A figure's line-of-sight volume: vertical prisms over its cells, in sub-level heights.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeMark {
    /// The footprint cells.
    pub cells: Vec<Hex>,
    /// Bottom of the hit zone.
    pub bottom: i64,
    /// Top of the hit zone.
    pub top: i64,
}

/// What the previews mark on the board now.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct PreviewMarks {
    /// The stops of the move being chosen; only flagged ones get a ring.
    pub stops: Vec<StopMark>,
    /// The route to the hovered stop: where the figure stands, then each space it steps on.
    pub route: Option<(Space, Vec<Space>)>,
    /// The hovered target's line-of-sight volume.
    pub volume: Option<VolumeMark>,
    /// The clear sample segments from the attacker to that volume.
    pub witnesses: Vec<Seg3>,
}

/// What a preview line shows; each look is drawn in its own color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Look {
    /// A stop that ends the move.
    StopsHere,
    /// A stop reached by falling.
    Falls,
    /// A stop that gives an enemy a reprisal.
    Reprisal,
    /// The route to the hovered stop.
    Route,
    /// The target's line-of-sight volume.
    Volume,
    /// A witness ray.
    Witness,
}

impl Look {
    const ALL: [Look; 6] = [
        Look::StopsHere,
        Look::Falls,
        Look::Reprisal,
        Look::Route,
        Look::Volume,
        Look::Witness,
    ];

    fn color(self) -> Color {
        match self {
            Look::StopsHere => Color::srgb(0.3, 0.6, 1.0),
            Look::Falls => Color::srgb(1.0, 0.55, 0.1),
            Look::Reprisal => Color::srgb(1.0, 0.2, 0.2),
            Look::Route => Color::srgb(1.0, 1.0, 1.0),
            Look::Volume => Color::srgba(0.9, 0.9, 1.0, 0.45),
            Look::Witness => Color::srgb(0.3, 1.0, 0.4),
        }
    }
}

/// The lines to draw for `marks`, by look. Each flag of a stop gets its own ring size, so a
/// space with several flags shows each.
pub fn preview_lines(marks: &PreviewMarks, orientation: Orientation) -> BTreeMap<Look, Vec<Line>> {
    let mut out: BTreeMap<Look, Vec<Line>> = BTreeMap::new();
    for stop in &marks.stops {
        let flags = [
            (stop.provokes_reprisal, Look::Reprisal, 0.75),
            (stop.falls, Look::Falls, 0.6),
            (stop.stops_here, Look::StopsHere, 0.45),
        ];
        for (_, look, scale) in flags.into_iter().filter(|(on, ..)| *on) {
            let lines = ring(orientation, stop.space, scale);
            out.entry(look).or_default().extend(lines);
        }
    }
    if let Some((from, path)) = &marks.route {
        out.entry(Look::Route)
            .or_default()
            .extend(route(orientation, *from, path));
    }
    if let Some(v) = &marks.volume {
        let lines = volume_outline(orientation, &v.cells, v.bottom, v.top);
        out.entry(Look::Volume).or_default().extend(lines);
    }
    let rays = marks.witnesses.iter().map(|s| segment(orientation, *s));
    out.entry(Look::Witness).or_default().extend(rays);
    out.retain(|_, lines| !lines.is_empty());
    out
}

/// Marks a spawned preview mesh.
#[derive(Component, Debug)]
pub struct PreviewMesh(pub Look);

/// One unlit material per look.
#[derive(Resource)]
struct PreviewLooks(BTreeMap<Look, Handle<StandardMaterial>>);

/// Draws [`PreviewMarks`] as line meshes, rebuilt whenever they or the board change.
pub struct PreviewPlugin;

impl Plugin for PreviewPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PreviewMarks>()
            .add_systems(Startup, make_looks)
            .add_systems(Update, draw_previews);
    }
}

fn make_looks(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    let looks = Look::ALL.map(|look| {
        let material = StandardMaterial {
            base_color: look.color(),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        };
        (look, materials.add(material))
    });
    commands.insert_resource(PreviewLooks(looks.into()));
}

fn line_mesh(lines: &[Line]) -> Mesh {
    let positions: Vec<Vec3> = lines.iter().flatten().copied().collect();
    Mesh::new(PrimitiveTopology::LineList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
}

fn draw_previews(
    mut commands: Commands,
    marks: Res<PreviewMarks>,
    looks: Option<Res<PreviewLooks>>,
    board: Option<Res<ClientBoard>>,
    drawn: Query<Entity, With<PreviewMesh>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(looks) = looks else { return };
    let board_changed = board.as_ref().is_some_and(Res::is_changed);
    if !marks.is_changed() && !board_changed {
        return;
    }
    for entity in &drawn {
        commands.entity(entity).despawn();
    }
    let Some(board) = board else { return };
    for (look, lines) in preview_lines(&marks, board.0.orientation) {
        commands.spawn((
            PreviewMesh(look),
            Mesh3d(meshes.add(line_mesh(&lines))),
            MeshMaterial3d(looks.0[&look].clone()),
            Transform::default(),
        ));
    }
}

#[cfg(test)]
mod tests;
