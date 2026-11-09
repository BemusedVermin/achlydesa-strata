use bevy::asset::AssetPlugin;
use strata_kernel::{Point, Point3, Seg3};

use super::*;
use crate::board::tests::board;

fn stop(q: i32, stops_here: bool, falls: bool, provokes_reprisal: bool) -> StopMark {
    StopMark {
        space: Space::new(q, 0, 1),
        stops_here,
        falls,
        provokes_reprisal,
    }
}

fn ray() -> Seg3 {
    Seg3::new(
        Point3::new(Point::new(0, 0), 22),
        Point3::new(Point::new(30, 0), 18),
    )
}

fn marks() -> PreviewMarks {
    PreviewMarks {
        stops: vec![
            stop(1, false, false, false),
            stop(2, true, true, true),
            stop(3, false, false, true),
        ],
        route: Some((Space::new(0, 0, 1), vec![Space::new(1, 0, 1)])),
        volume: Some(VolumeMark {
            cells: vec![Hex::new(2, 0)],
            bottom: 12,
            top: 24,
        }),
        witnesses: vec![ray(), ray()],
    }
}

#[test]
fn every_flag_rings_its_stop_and_plain_stops_get_none() {
    let lines = preview_lines(&marks(), Orientation::Pointy);
    let count = |look| lines.get(&look).map_or(0, Vec::len);
    assert_eq!(count(Look::Reprisal), 12, "two stops provoke");
    assert_eq!(count(Look::Falls), 6);
    assert_eq!(count(Look::StopsHere), 6);
    assert_eq!(count(Look::Route), 1);
    assert_eq!(count(Look::Volume), 18);
    assert_eq!(count(Look::Witness), 2);
}

#[test]
fn nothing_to_preview_draws_nothing() {
    let none = PreviewMarks {
        stops: vec![stop(1, false, false, false)],
        ..PreviewMarks::default()
    };
    assert!(preview_lines(&none, Orientation::Flat).is_empty());
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), PreviewPlugin))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .insert_resource(ClientBoard(board()));
    app.update();
    app
}

fn drawn(app: &mut App) -> Vec<Look> {
    let mut query = app.world_mut().query::<&PreviewMesh>();
    let mut looks: Vec<Look> = query.iter(app.world()).map(|m| m.0).collect();
    looks.sort();
    looks
}

#[test]
fn the_plugin_draws_one_mesh_per_look_and_clears_them() {
    let mut app = app();
    assert!(drawn(&mut app).is_empty());
    *app.world_mut().resource_mut::<PreviewMarks>() = marks();
    app.update();
    assert_eq!(drawn(&mut app), Look::ALL);
    *app.world_mut().resource_mut::<PreviewMarks>() = PreviewMarks::default();
    app.update();
    assert!(drawn(&mut app).is_empty());
}
