-- Native logic written as plain data in a module's registrations: a modifier that applies only on
-- rock, so the harness answers `on_terrain` from the map.
terrain { id = 'grass', name = 'Grass', standable = true }
terrain { id = 'rock', name = 'Rock', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'crag', name = 'Crag', terrain = 'rock', thickness = 1, shape = {{ q = 0, r = 0 }} }
map {
  id = 'mixed', name = 'Mixed', orientation = 'pointy',
  pieces = {
    { piece = 'meadow', at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = 'crag', at = { q = 1, r = 0 }, rot = 0, level = 0 },
  },
}
stat { id = 'defense', name = 'Defense', min = 0, max = 9 }
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = { defense = 3 },
  figures = {{
    id = 's', count = 2, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}
rule_module {
  id = 'rules',
  register = function(r)
    r.modifier {
      query = 'defense', layer = 'add',
      when = { cond = 'on_terrain', terrain = 'rock' },
      value = 2,
    }
  end,
}
ruleset { id = 'rs', name = 'Native', addons = {{ id = 't:rules' }} }

test {
  id = 'on_terrain', name = 'a registered native modifier applies only on its terrain',
  ruleset = 'rs', map = 'mixed',
  place = {{ unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }, { q = 1, r = 0, z = 1 }} }},
  steps = {
    expect.stat('p0.test_soldier.s#1', 'defense', 3),
    expect.stat('p0.test_soldier.s#2', 'defense', 5),
  },
}
