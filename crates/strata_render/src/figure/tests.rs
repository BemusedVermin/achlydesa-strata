use std::collections::BTreeMap;

use bevy::asset::AssetPlugin;
use strata_kernel::Hex;
use strata_scene::{IsoView, fallback_sprite};

use super::*;
use crate::board::tests::board;

const SOLDIER: DefId = DefId(3);

fn look() -> LookKey {
    (SOLDIER, Sym::new("s"))
}

fn pose(id: u32, hex: Hex, rotation: u8) -> FigurePose {
    let anchor = Space::at(hex, 1);
    FigurePose {
        id: FigureId(id),
        look: look(),
        anchor,
        cells: vec![anchor],
        rotation: Rotation::from_steps(i32::from(rotation)),
        seat: 0,
    }
}

/// An app with the fallback for a one-level figure, at camera step 0.
fn app(poses: Vec<FigurePose>) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), FigurePlugin))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .init_resource::<IsoCamera>()
        .insert_resource(ClientBoard(board()));
    let key = Sym::new("sprite.test_soldier");
    let scale = PixelScale::default();
    let art = fallback_sprite(
        key,
        (0, 12),
        &[Hex::ORIGIN],
        Orientation::Pointy,
        [9, 9, 9],
        scale,
    );
    let sheet = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(Image::default());
    let sprite = FigureSprite {
        def: art.def,
        scale,
        sheet,
        sheet_size: art.size,
        base: Some([9, 9, 9]),
        tint_by_seat: false,
    };
    app.insert_resource(FigureSprites(BTreeMap::from([(look(), sprite)])))
        .insert_resource(FigurePoses(poses));
    app.update();
    app
}

fn figures(app: &mut App) -> BTreeMap<FigureId, (Entity, Transform)> {
    let mut query = app.world_mut().query::<(Entity, &FigureView, &Transform)>();
    let all = query.iter(app.world());
    all.map(|(e, v, t)| (v.id, (e, *t))).collect()
}

fn bases(app: &mut App) -> usize {
    let mut query = app.world_mut().query::<&FigureBase>();
    query.iter(app.world()).count()
}

/// The texture rectangle's top-left corner the figure's material shows.
fn uv_origin(app: &mut App, id: FigureId) -> Vec2 {
    let mut query = app.world_mut().query::<(&FigureView, &Placed)>();
    let (_, placed) = query.iter(app.world()).find(|(v, _)| v.id == id).unwrap();
    let handle = placed.material.clone();
    let materials = app.world().resource::<Assets<StandardMaterial>>();
    materials.get(&handle).unwrap().uv_transform.translation
}

#[test]
fn each_figure_gets_a_billboard_standing_on_its_space() {
    let mut app = app(vec![pose(0, Hex::ORIGIN, 0), pose(1, Hex::new(20, 0), 2)]);
    let drawn = figures(&mut app);
    assert_eq!(drawn.len(), 2);
    let (_, transform) = drawn[&FigureId(1)];
    let expected = surface_point(Orientation::Pointy, Hex::new(20, 0), 1);
    assert!((transform.translation - expected).length() < 1e-5);
    assert_eq!(transform.rotation, billboard_rotation(0.0));
    assert_eq!(bases(&mut app), 2, "a fallback stands on a base per cell");
}

#[test]
fn the_frame_follows_the_facing_and_the_camera() {
    let mut app = app(vec![pose(0, Hex::ORIGIN, 0), pose(1, Hex::new(20, 0), 2)]);
    let rows = 6.0;
    assert_eq!(uv_origin(&mut app, FigureId(0)), Vec2::ZERO);
    assert!((uv_origin(&mut app, FigureId(1)).y - 2.0 / rows).abs() < 1e-6);
    let mut view = IsoView::default();
    view.turn(1);
    view.yaw = std::f32::consts::FRAC_PI_3;
    app.insert_resource(IsoCamera(view));
    app.update();
    assert!((uv_origin(&mut app, FigureId(0)).y - 1.0 / rows).abs() < 1e-6);
    assert!((uv_origin(&mut app, FigureId(1)).y - 3.0 / rows).abs() < 1e-6);
    let (_, transform) = figures(&mut app)[&FigureId(0)];
    assert!((transform.rotation * Vec3::Z - Vec3::new(0.866, 0.0, 0.5)).length() < 1e-3);
}

#[test]
fn a_moved_figure_keeps_its_entity_and_a_removed_one_goes() {
    let mut app = app(vec![pose(0, Hex::ORIGIN, 0), pose(1, Hex::new(20, 0), 2)]);
    let before = figures(&mut app);
    app.insert_resource(FigurePoses(vec![pose(0, Hex::new(1, 0), 3)]));
    app.update();
    let after = figures(&mut app);
    assert_eq!(after.len(), 1);
    assert_eq!(after[&FigureId(0)].0, before[&FigureId(0)].0, "same entity");
    let expected = surface_point(Orientation::Pointy, Hex::new(1, 0), 1);
    assert!((after[&FigureId(0)].1.translation - expected).length() < 1e-5);
    assert!(
        (uv_origin(&mut app, FigureId(0)).y - 3.0 / 6.0).abs() < 1e-6,
        "turned to 3"
    );
    assert_eq!(bases(&mut app), 1);
}

#[test]
fn a_finished_animation_returns_to_idle() {
    let mut app = app(vec![pose(0, Hex::ORIGIN, 0)]);
    let attack = Sym::new("attack");
    let mut sprites = app.world_mut().resource_mut::<FigureSprites>();
    let sprite = sprites.0.get_mut(&look()).unwrap();
    let one_frame = strata_scene::Animation { frames: 1, fps: 1 };
    sprite.def.animations.insert(attack.clone(), one_frame);
    app.update();
    let (entity, _) = figures(&mut app)[&FigureId(0)];
    let playing = |elapsed| FigureAnimation {
        name: attack.clone(),
        elapsed,
    };
    app.world_mut().entity_mut(entity).insert(playing(0.0));
    app.update();
    assert_eq!(
        app.world().get::<FigureAnimation>(entity).unwrap().name,
        attack
    );
    app.world_mut().entity_mut(entity).insert(playing(5.0));
    app.update();
    let now = app.world().get::<FigureAnimation>(entity).unwrap();
    assert_eq!(now.name.as_str(), IDLE);
}

#[test]
fn figures_without_a_sprite_or_a_board_are_not_drawn() {
    let mut stranger = pose(5, Hex::ORIGIN, 0);
    stranger.look = (DefId(99), Sym::new("x"));
    let mut app = app(vec![stranger]);
    assert!(figures(&mut app).is_empty());
    app.world_mut().remove_resource::<ClientBoard>();
    app.insert_resource(FigurePoses(vec![pose(0, Hex::ORIGIN, 0)]));
    app.update();
    assert!(figures(&mut app).is_empty());
}
