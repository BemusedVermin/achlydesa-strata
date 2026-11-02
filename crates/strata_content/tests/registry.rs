use std::path::{Path, PathBuf};

use strata_content::{ContentRegistry, load};
use strata_kernel::{DefId, PackId};
use strata_model::{FactionDef, TerrainDef, TileDef, UnitDef};

fn write_pack(root: &Path, id: &str, dependencies: &str, data: &str) {
    let dir = root.join(id);
    std::fs::create_dir_all(&dir).unwrap();
    let manifest = format!(
        "return {{ id = '{id}', version = '1.0.0', name = '{id}', engine = '>=0.1, <0.2', \
         dependencies = {{ {dependencies} }} }}"
    );
    std::fs::write(dir.join("pack.lua"), manifest).unwrap();
    std::fs::write(dir.join("data.lua"), data).unwrap();
}

const BASE: &str = r#"
terrain { id = 'grass', name = 'Grass', standable = true }
terrain { id = 'rock', name = 'Rock', standable = false }
faction { id = 'brass', name = 'Brass', color = '#B5893A' }
stat { id = 'hp', name = 'HP', min = 0, max = 9 }
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
"#;

const GAME: &str = r#"
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'cliff', name = 'Cliff', terrain = 'base:rock', thickness = 2, shape = {{ q = 0, r = 0 }} }
unit {
    id = 'soldier', name = 'Soldier', kind = 'squad', rarity = 'common', faction = 'brass',
    stats = { hp = 5 },
    figures = {{
        id = 'f', count = 1, footprint = 'one', height = 1,
        los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
    }},
    abilities = {{ name = 'Brace', text = 'Holds.', scope = 'unit', triggers = {{ event = 'e', effect = function() end }} }},
}
"#;

fn fixture(base: &str, game: &str) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().unwrap();
    write_pack(dir.path(), "base", "", base);
    write_pack(dir.path(), "game", "base = '^1'", game);
    let roots = vec![dir.path().to_owned()];
    (dir, roots)
}

fn errors(base: &str, game: &str) -> Vec<String> {
    let (_dir, roots) = fixture(base, game);
    let errors = load(&roots).expect_err("expected the load to fail");
    errors.iter().map(ToString::to_string).collect()
}

#[test]
fn a_valid_fixture_pack_loads_and_resolves_every_reference() {
    let (_dir, roots) = fixture(BASE, GAME);
    let registry = load(&roots).unwrap();
    let packs = [PackId::new("base"), PackId::new("game")];
    assert_eq!(registry.load_order(), packs);

    // Terrain is interned in full-ID order: base:grass, base:rock.
    assert_eq!(
        registry.find::<TerrainDef>("base:grass").unwrap().0,
        DefId(0)
    );
    assert_eq!(
        registry.find::<TerrainDef>("base:rock").unwrap().0,
        DefId(1)
    );
    let tiles: Vec<_> = registry.iter::<TileDef>().collect();
    assert_eq!(tiles.len(), 2);
    let (cliff_id, cliff_name, cliff) = tiles[0];
    assert_eq!((cliff_id, cliff_name), (DefId(0), "game:cliff"));
    assert_eq!(cliff.id, cliff_id);
    // A full ID in another pack, and a bare ID found in a dependency.
    assert_eq!(cliff.terrain, DefId(1));
    assert_eq!(tiles[1].2.terrain, DefId(0));

    let (unit_id, unit) = registry.find::<UnitDef>("game:soldier").unwrap();
    assert_eq!(unit.id, unit_id);
    let (faction_id, _) = registry.find::<FactionDef>("base:brass").unwrap();
    assert_eq!(unit.faction, Some(faction_id));
    assert_eq!(unit.figures[0].footprint, DefId(0));
    assert_eq!(unit.stats.keys().copied().collect::<Vec<_>>(), [DefId(0)]);
    assert_eq!(registry.name::<UnitDef>(unit_id), Some("game:soldier"));
    assert_eq!(registry.get::<UnitDef>(unit_id), Some(unit));
    assert!(registry.get::<UnitDef>(DefId(9)).is_none());

    // The script handle points at the unit that holds the function.
    let script = match &unit.abilities[0].triggers[0].effect {
        strata_model::Logic::Script(script) => script,
        other => panic!("not a script: {other:?}"),
    };
    assert_eq!(script.def, unit_id);
}

#[test]
fn loading_the_same_packs_twice_gives_equal_registries() {
    let (_dir, roots) = fixture(BASE, GAME);
    let (first, second) = (load(&roots).unwrap(), load(&roots).unwrap());
    assert_eq!(first, second);
    let ids = |r: &ContentRegistry| -> Vec<_> {
        r.iter::<UnitDef>()
            .map(|(id, name, _)| (id, name.to_owned()))
            .collect()
    };
    assert_eq!(ids(&first), ids(&second));
    // A different pack set is a different registry.
    let (_other, roots) = fixture(BASE, "");
    assert_ne!(load(&roots).unwrap(), first);
}

#[test]
fn the_registry_is_shareable_across_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ContentRegistry>();
}

#[test]
fn a_dangling_reference_names_the_file_line_and_field() {
    let game = "tile { id = 't', name = 'T', terrain = 'lava', thickness = 1, shape = {{ q = 0, r = 0 }} }";
    assert_eq!(
        errors(BASE, game),
        ["game/data.lua:1: terrain: unknown terrain `lava`"]
    );
}

#[test]
fn a_wrong_kind_reference_says_what_it_found() {
    let game = "
unit { id = 'u', name = 'U', kind = 'squad', rarity = 'common', faction = 'grass', stats = {}, figures = {} }
tile { id = 't', name = 'T', terrain = 'brass', thickness = 1, shape = {{ q = 0, r = 0 }} }";
    assert_eq!(
        errors(BASE, game),
        [
            "game/data.lua:3: terrain: `brass` is a faction, but a terrain is required",
            "game/data.lua:2: faction: `grass` is a terrain, but a faction is required",
        ]
    );
}

#[test]
fn every_bad_reference_is_reported_not_just_the_first() {
    let game = "
tile { id = 'a', name = 'A', terrain = 'lava', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'b', name = 'B', terrain = 'ice', thickness = 1, shape = {{ q = 0, r = 0 }} }";
    assert_eq!(errors(BASE, game).len(), 2);
}

#[test]
fn a_bare_reference_prefers_its_own_pack_and_rejects_ambiguity() {
    // `game` defines its own `grass`, which wins over the one in `base`.
    let own = "
terrain { id = 'grass', name = 'Own', standable = true }
tile { id = 't', name = 'T', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }";
    let (_dir, roots) = fixture(BASE, own);
    let registry = load(&roots).unwrap();
    let (grass, _) = registry.find::<TerrainDef>("game:grass").unwrap();
    assert_eq!(registry.find::<TileDef>("game:t").unwrap().1.terrain, grass);

    // Two dependencies define `grass` and the referencing pack has none.
    let dir = tempfile::tempdir().unwrap();
    write_pack(
        dir.path(),
        "one",
        "",
        "terrain { id = 'grass', name = 'G', standable = true }",
    );
    write_pack(
        dir.path(),
        "two",
        "",
        "terrain { id = 'grass', name = 'G', standable = true }",
    );
    let tile = "tile { id = 't', name = 'T', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }";
    write_pack(dir.path(), "user", "one = '^1', two = '^1'", tile);
    let errors = load(&[dir.path().to_owned()]).unwrap_err();
    assert_eq!(
        errors[0].to_string(),
        "user/data.lua:1: terrain: `grass` is ambiguous: it could be one:grass or two:grass"
    );
}

#[test]
fn pack_errors_are_returned_as_load_errors() {
    let dir = tempfile::tempdir().unwrap();
    write_pack(dir.path(), "lonely", "missing = '^1'", "");
    let errors = load(&[dir.path().to_owned()]).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].to_string().contains("missing pack `missing`"));
}
