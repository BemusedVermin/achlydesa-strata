use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use bevy::asset::AssetPlugin;
use strata_content::read_pack_assets;
use strata_kernel::Value;
use strata_kernel::{DefId, Hex, PackId, Rotation};
use strata_lua::PackSource;
use strata_model::{ArmyEntry, ArmyList, Command, Text};
use strata_render::{FigurePlugin, FigureView};
use strata_scene::PixelScale;
use strata_session::{LaunchSpec, launch};

use super::*;
use crate::sim_link::{Sim, SimLinkPlugin};

fn fixture() -> PackSource {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/assets/sprites");
    PackSource {
        id: PackId::new("sprites"),
        root,
        dependencies: Vec::new(),
    }
}

fn look(visual: Option<&str>, color: Option<&str>) -> FigureLook {
    FigureLook {
        unit: DefId(1),
        figure: Sym::new("s"),
        pack: PackId::new("sprites"),
        visual: visual.map(Sym::new),
        hit_zone: (0, 18),
        footprint: vec![Hex::ORIGIN],
        color: color.map(str::to_owned),
    }
}

/// Builds the sprite for `look` from the fixture pack, returning it and the image it stored.
fn build(look: &FigureLook) -> (FigureSprite, Image) {
    // The simulation thread runs `assets.lua`; the client only types what it declared.
    let (catalog, errors) = SpriteCatalog::from_packs(&read_pack_assets(&[fixture()]));
    assert!(errors.is_empty(), "{errors:?}");
    build_from(&catalog, look)
}

fn build_from(catalog: &SpriteCatalog, look: &FigureLook) -> (FigureSprite, Image) {
    let mut stored = Vec::new();
    let mut add = |image: Image| {
        stored.push(image);
        Handle::default()
    };
    let sprite = figure_sprite(catalog, look, Orientation::Pointy, &mut add);
    assert_eq!(stored.len(), 1, "one sheet per figure kind");
    (sprite, stored.remove(0))
}

#[test]
fn a_declared_sprite_uses_its_sheet() {
    let (sprite, image) = build(&look(Some("sprite.test_soldier"), Some("#B5893A")));
    assert_eq!(sprite.def.frame, UVec2::new(4, 6));
    assert_eq!(sprite.sheet_size, UVec2::new(4, 36));
    assert_eq!(image.size(), UVec2::new(4, 36));
    assert_eq!(
        sprite.scale,
        PixelScale {
            pixels_per_level: 6,
            hex_width_px: 32
        }
    );
    assert_eq!((sprite.base, sprite.tint_by_seat), (None, false));
}

#[test]
fn a_missing_key_or_sheet_falls_back_to_a_generated_sprite() {
    for visual in [None, Some("sprite.nowhere"), Some("sprite.test_archer")] {
        let (sprite, image) = build(&look(visual, Some("#B5893A")));
        assert_eq!(sprite.base, Some([0xb5, 0x89, 0x3a]), "{visual:?}");
        assert!(!sprite.tint_by_seat);
        // Drawn at the pack's scale: 18 sub-levels at 6 px per level is 9 px.
        assert_eq!(sprite.def.frame.y, 9);
        assert_eq!(image.size(), sprite.sheet_size);
        let height = sprite.scale.px_to_sub_levels(sprite.def.anchor.y);
        assert_eq!(height, 18.0);
    }
}

#[test]
fn a_pack_with_a_broken_assets_file_gets_fallbacks_even_for_a_key_declared_elsewhere() {
    let mut packs = read_pack_assets(&[fixture()]);
    packs.push(PackAssets {
        pack: PackId::new("broken"),
        root: PathBuf::from("/nowhere"),
        declared: Err("syntax error".into()),
    });
    let (catalog, errors) = SpriteCatalog::from_packs(&packs);
    assert_eq!(errors.len(), 1);
    let mut broken = look(Some("sprite.test_soldier"), Some("#B5893A"));
    broken.pack = PackId::new("broken");
    let (sprite, _) = build_from(&catalog, &broken);
    assert!(
        sprite.base.is_some(),
        "generated, not the fixture's soldier"
    );
    let (sprite, _) = build_from(&catalog, &look(Some("sprite.test_soldier"), None));
    assert_eq!(
        sprite.base, None,
        "the fixture pack's own figures keep their art"
    );
}

#[test]
fn a_sprite_carries_the_footprint_it_faces_by() {
    let mut snake = look(Some("sprite.test_soldier"), None);
    snake.footprint = vec![Hex::ORIGIN, Hex::new(1, 0)];
    let (sprite, _) = build(&snake);
    assert_eq!(sprite.footprint, snake.footprint);
    snake.visual = None;
    let (sprite, _) = build(&snake);
    assert_eq!(sprite.footprint, snake.footprint);
}

#[test]
fn a_fallback_without_a_faction_is_tinted_by_seat() {
    let (sprite, _) = build(&look(None, None));
    assert!(sprite.tint_by_seat);
    let (sprite, _) = build(&look(None, Some("not a color")));
    assert!(sprite.tint_by_seat);
}

fn spec() -> LaunchSpec {
    let root = env!("CARGO_MANIFEST_DIR");
    let army = |unit: &str| {
        Some(ArmyList {
            name: Text("test".into()),
            format: Sym::new("none"),
            packs: BTreeMap::new(),
            units: vec![ArmyEntry {
                unit: Sym::new(unit),
                copies: 2,
            }],
        })
    };
    LaunchSpec {
        roots: vec![PathBuf::from(root).join("../../fixtures/packs/toy")],
        scenario: "duel".into(),
        ruleset: Some("toy_rust".into()),
        seed: 7,
        armies: vec![army("test_soldier"), army("test_archer")],
        commands: 1,
    }
}

/// A match in the fixture pack whose ruleset lets a seat place a unit, one soldier per side.
fn placing_spec() -> LaunchSpec {
    let root = env!("CARGO_MANIFEST_DIR");
    let mut army = spec().armies.remove(0);
    if let Some(list) = &mut army {
        list.units[0].copies = 1;
    }
    LaunchSpec {
        roots: vec![PathBuf::from(root).join("../../fixtures/packs/command")],
        scenario: "duel".into(),
        ruleset: Some("rs".into()),
        seed: 5,
        armies: vec![army, None],
        commands: 6,
    }
}

#[test]
fn a_launched_match_gets_a_sprite_for_every_figure_kind() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Image>()
        .init_resource::<strata_render::IsoCamera>()
        .add_plugins((
            SimLinkPlugin::new(launch(placing_spec())),
            AssetsPlugin,
            FigurePlugin,
        ));
    let mut placed = false;
    for _ in 0..2000 {
        app.update();
        let world = app.world();
        let sprites = &world.resource::<FigureSprites>().0;
        let Some(view) = world.resource::<LatestView>().0.as_ref() else {
            std::thread::sleep(Duration::from_millis(5));
            continue;
        };
        if !placed {
            // Put the soldier on the board, as the session's launch tests do.
            let at = Value::List(vec![Value::List([0, 0, 1].map(Value::Int).to_vec())]);
            let payload = [(Sym::new("unit"), Value::Int(0)), (Sym::new("at"), at)];
            let command = Command::Custom {
                id: Sym::new("place_unit"),
                payload: Value::Map(payload.into()),
            };
            world.resource::<Sim>().0.send_command(command);
            placed = true;
        }
        let poses = &world.resource::<FigurePoses>().0;
        if !sprites.is_empty() && !poses.is_empty() {
            let looks = &world.resource::<BoardLooks>().looks;
            assert!(!looks.is_empty());
            assert_eq!(sprites.len(), looks.len(), "one per figure kind");
            assert!(
                sprites.values().all(|s| s.base.is_some()),
                "the fixture pack has no art"
            );
            assert_eq!(poses, &figure_poses(&view.state));
            assert!(poses.iter().all(|p| sprites.contains_key(&p.look)));
            app.update();
            let mut drawn = app.world_mut().query::<&FigureView>();
            assert!(
                drawn.iter(app.world()).count() >= 1,
                "at least one figure is drawn on the board"
            );
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the sprites never loaded or no figure reached the board");
}

#[test]
fn poses_list_each_figure_on_the_board_with_its_cells() {
    let view = {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(SimLinkPlugin::new(launch(spec())));
        for _ in 0..2000 {
            app.update();
            if app.world().resource::<LatestView>().0.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        app.world_mut()
            .resource_mut::<LatestView>()
            .0
            .take()
            .unwrap()
    };
    let mut state = view.state;
    assert!(state.figures.len() >= 2);
    let anchor = Space::at(Hex::ORIGIN, 1);
    let first = &mut state.figures[0];
    (first.location, first.status) = (Some(anchor), FigureStatus::OnBoard);
    first.rotation = Rotation::from_steps(4);
    // On the board but without a location is not drawn; neither is a figure in reserve.
    state.figures[1].status = FigureStatus::OnBoard;
    state.figures[1].location = None;
    let poses = figure_poses(&state);
    assert_eq!(poses.len(), 1);
    assert_eq!(poses[0].id, FigureId(0));
    assert_eq!(
        poses[0].look,
        (
            state.figures[0].def.unit,
            state.figures[0].def.figure.clone()
        )
    );
    assert_eq!(
        poses[0].cells,
        [anchor],
        "a figure missing from occupancy covers its anchor"
    );
    assert_eq!(poses[0].rotation, Rotation::from_steps(4));
    let extra = Space::at(Hex::new(1, 0), 1);
    for space in [anchor, extra] {
        state.board_changes.occupancy.insert(space, FigureId(0));
    }
    assert_eq!(figure_poses(&state)[0].cells, [anchor, extra]);
}
