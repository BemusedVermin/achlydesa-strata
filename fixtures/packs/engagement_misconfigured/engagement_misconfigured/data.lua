-- Pack tests for a `core:engagement` misconfiguration. Each test fails on purpose: the reprisal it
-- provokes names a die or face that does not exist, and the error must name the config key.
-- Figure 0 for player 0 on (0,0) leaves an enemy on (1,0) for (-1,0) with the `toy:driver` command
-- `move_to`.
stat { id = 'life', name = 'Life', min = 1, max = 9 }
marker { id = 'wound', name = 'Wound', stacking = 'count', visibility = 'public' }
die { id = 'combat', name = 'Combat', faces = { 'hit', 'blank' } }
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
local function at(q, r) return { piece = 'meadow', at = { q = q, r = r }, rot = 0, level = 0 } end
map { id = 'field', name = 'Field', orientation = 'pointy', pieces = { at(-1, 0), at(0, 0), at(1, 0) } }
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'test_soldier', kind = 'squad', rarity = 'common', stats = { life = 2 },
  figures = {{
    id = 'f', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}

local function rules(id, config)
  ruleset {
    id = id, name = id, addons = {{ id = 'toy:driver' }},
    slots = { engagement = { id = 'core:engagement', config = config }, units = { id = 'core:units' } },
  }
end
rules('rs_bad_die', { die = { Str = 'combt' } })
rules('rs_bad_face', { wound_on = { Str = 'hti' } })

local function leave(id, ruleset)
  test {
    id = id, name = id, ruleset = ruleset, map = 'field',
    place = {
      { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
      { unit = 'test_soldier', player = 1, at = {{ q = 1, r = 0, z = 1 }} },
    },
    steps = { act.command(0, 'move_to', { figure = 0, q = -1, r = 0, z = 1 }) },
  }
end
leave('misspelled_die', 'rs_bad_die')
leave('misspelled_face', 'rs_bad_face')
