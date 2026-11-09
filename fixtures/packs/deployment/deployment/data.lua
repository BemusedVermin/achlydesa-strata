-- Pack tests for `core:deployment`. Units start in reserve; `place_unit` carries
-- { unit = <unit id>, at = { { q, r, z [, o] }, ... } }. Units are numbered in the order the tests
-- list them, so player 0's first unit is unit 0.
terrain { id = 'grass', name = 'Grass', standable = true }
terrain { id = 'water', name = 'Water', standable = false }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'pond', name = 'Pond', terrain = 'water', thickness = 1, shape = {{ q = 0, r = 0 }} }
map {
  id = 'row', name = 'Row', orientation = 'pointy',
  pieces = {
    { piece = 'meadow', at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = 'pond',   at = { q = 1, r = 0 }, rot = 0, level = 0 },
    { piece = 'meadow', at = { q = 2, r = 0 }, rot = 0, level = 0 },
    { piece = 'meadow', at = { q = 3, r = 0 }, rot = 0, level = 0 },
    { piece = 'meadow', at = { q = 4, r = 0 }, rot = 0, level = 0 },
    { piece = 'meadow', at = { q = 5, r = 0 }, rot = 0, level = 0 },
    { piece = 'meadow', at = { q = 6, r = 0 }, rot = 0, level = 0 },
    { piece = 'meadow', at = { q = 7, r = 0 }, rot = 0, level = 0 },
    { piece = 'meadow', at = { q = 8, r = 0 }, rot = 0, level = 0 },
    { piece = 'meadow', at = { q = 9, r = 0 }, rot = 0, level = 0 },
  },
  zones = {
    west = {{ q = 0, r = 0 }, { q = 1, r = 0 }, { q = 2, r = 0 }, { q = 3, r = 0 }, { q = 4, r = 0 }},
    east = {{ q = 5, r = 0 }, { q = 6, r = 0 }, { q = 7, r = 0 }, { q = 8, r = 0 }, { q = 9, r = 0 }},
  },
}
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
footprint { id = 'two', name = 'Two', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = {}, figures = {{
    id = 's', count = 2, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}
-- One two-space figure: placed facing the default way, its rear is one column east.
unit {
  id = 'test_cart', name = 'Test Cart', kind = 'squad', rarity = 'common',
  stats = {}, figures = {{
    id = 'c', count = 1, footprint = 'two', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}
scenario {
  id = 'duel', name = 'Duel', map = 'row', players = { min = 2, max = 2 },
  slots = {
    { id = 'a', start = 'west', deploy_order = 1 },
    { id = 'b', start = 'east', deploy_order = 2 },
  },
}
ruleset { id = 'rs', name = 'Deployment', slots = { deployment = { id = 'core:deployment' } } }

local function reserve(p0, p1)
  local out = {}
  for _ = 1, p0 do out[#out + 1] = { unit = 'test_soldier', player = 0, at = {} } end
  for _ = 1, p1 do out[#out + 1] = { unit = 'test_soldier', player = 1, at = {} } end
  return out
end
local function put(unit, a, b)
  return { unit = unit, at = {{ a, 0, 1 }, { b, 0, 1 }} }
end

test {
  id = 'place', name = 'a unit is placed on the surfaces of its start zone',
  ruleset = 'rs', scenario = 'duel', place = reserve(1, 1),
  steps = {
    act.command(0, 'place_unit', put(0, 0, 2)),
    expect.event('figure_moved'),
    expect.figure('p0.test_soldier#1', 'on_board'),
    expect.figure('p1.test_soldier#1', 'reserve'),
  },
}
test {
  id = 'alternate', name = 'players place in slot order and the last placement ends deployment',
  ruleset = 'rs', scenario = 'duel', place = reserve(1, 1),
  steps = {
    act.command(0, 'place_unit', put(0, 0, 2)),
    act.command(1, 'place_unit', put(1, 5, 6)),
    expect.event('deployment_done'),
  },
}
test {
  id = 'skip', name = 'a player with nothing left to place is skipped',
  ruleset = 'rs', scenario = 'duel', place = reserve(1, 2),
  steps = {
    act.command(0, 'place_unit', put(0, 0, 2)),
    act.command(1, 'place_unit', put(1, 5, 6)),
    act.command(1, 'place_unit', put(2, 7, 8)),
    expect.event('deployment_done'),
  },
}
test {
  id = 'out_of_turn', name = 'the second slot cannot place first',
  ruleset = 'rs', scenario = 'duel', place = reserve(1, 1),
  steps = { act.command(1, 'place_unit', put(1, 5, 6)) },
}
test {
  id = 'wrong_zone', name = 'placing outside the start zone is refused',
  ruleset = 'rs', scenario = 'duel', place = reserve(1, 1),
  steps = { act.command(0, 'place_unit', put(0, 0, 5)) },
}
test {
  id = 'water', name = 'placing on a surface nobody can stand on is refused',
  ruleset = 'rs', scenario = 'duel', place = reserve(1, 1),
  steps = { act.command(0, 'place_unit', put(0, 0, 1)) },
}
test {
  id = 'stacked', name = 'two figures cannot share a space',
  ruleset = 'rs', scenario = 'duel', place = reserve(1, 1),
  steps = { act.command(0, 'place_unit', put(0, 2, 2)) },
}
test {
  id = 'too_few', name = 'every figure of the unit needs a space',
  ruleset = 'rs', scenario = 'duel', place = reserve(1, 1),
  steps = { act.command(0, 'place_unit', { unit = 0, at = {{ 0, 0, 1 }} }) },
}
test {
  id = 'enemy_unit', name = 'a player cannot place the enemy unit',
  ruleset = 'rs', scenario = 'duel', place = reserve(1, 1),
  steps = { act.command(0, 'place_unit', put(1, 0, 2)) },
}
test {
  id = 'taken', name = 'a space already holding a figure is refused',
  ruleset = 'rs', scenario = 'duel', place = reserve(2, 1),
  steps = {
    act.command(0, 'place_unit', put(0, 0, 2)),
    act.command(1, 'place_unit', put(2, 5, 6)),
    act.command(0, 'place_unit', put(1, 0, 2)),
  },
}

-- Two-space figures: every cell must be in the zone, standable and free.
local function cart(unit, q)
  return { unit = unit, at = {{ q, 0, 1 }} }
end
local carts = {
  { unit = 'test_cart', player = 0, at = {} },
  { unit = 'test_soldier', player = 0, at = {} },
  { unit = 'test_soldier', player = 1, at = {} },
}
test {
  id = 'cart', name = 'a two-space figure covers both its cells',
  ruleset = 'rs', scenario = 'duel', place = carts,
  steps = {
    act.command(0, 'place_unit', cart(0, 2)),
    expect.event('figure_moved'),
    expect.figure('p0.test_cart#1', 'on_board'),
  },
}
test {
  id = 'cart_onto_soldier', name = 'a rear cell cannot be placed on a figure',
  ruleset = 'rs', scenario = 'duel', place = carts,
  steps = {
    act.command(0, 'place_unit', put(1, 0, 3)),
    act.command(1, 'place_unit', put(2, 5, 6)),
    act.command(0, 'place_unit', cart(0, 2)),
  },
}
test {
  id = 'soldier_onto_cart', name = 'a figure cannot be placed on a rear cell',
  ruleset = 'rs', scenario = 'duel', place = carts,
  steps = {
    act.command(0, 'place_unit', cart(0, 2)),
    act.command(1, 'place_unit', put(2, 5, 6)),
    act.command(0, 'place_unit', put(1, 0, 3)),
  },
}
test {
  id = 'cart_water', name = 'a rear cell cannot stand on water',
  ruleset = 'rs', scenario = 'duel', place = carts,
  steps = { act.command(0, 'place_unit', cart(0, 0)) },
}
test {
  id = 'cart_outside', name = 'a rear cell cannot leave the start zone',
  ruleset = 'rs', scenario = 'duel', place = carts,
  steps = { act.command(0, 'place_unit', cart(0, 4)) },
}

-- Turned placements, on a separate board. `act.deploy` builds the same `place_unit` payload,
-- with a rotation o per figure: o puts a cart's rear in direction 0 E, 1 SE, 2 SW, 3 W, 4 NW,
-- 5 NE of its anchor. Ground covers q = 0..3, r = -1..1 at level 1, except a hill at (3, -1)
-- standing at level 3. The west zone is q = 0..2, r = -1..1, plus the hill and the empty column
-- (-1, 0); (3, 0) is ground outside it.
local function ground(q, r, level)
  return { piece = 'meadow', at = { q = q, r = r }, rot = 0, level = level }
end
local field = {}
local west = {{ q = 3, r = -1 }, { q = -1, r = 0 }}
for q = 0, 3 do
  for r = -1, 1 do
    field[#field + 1] = ground(q, r, 0)
    if q < 3 then west[#west + 1] = { q = q, r = r } end
  end
end
field[#field + 1] = ground(3, -1, 1)
field[#field + 1] = ground(3, -1, 2)
for q = 6, 8 do field[#field + 1] = ground(q, 0, 0) end
map {
  id = 'field', name = 'Field', orientation = 'pointy', pieces = field,
  zones = { west = west, east = {{ q = 6, r = 0 }, { q = 7, r = 0 }, { q = 8, r = 0 }} },
}
scenario {
  id = 'field_duel', name = 'Field Duel', map = 'field', players = { min = 2, max = 2 },
  slots = {
    { id = 'a', start = 'west', deploy_order = 1 },
    { id = 'b', start = 'east', deploy_order = 2 },
  },
}
local function turned(o, steps)
  return {
    id = 'turn_' .. o, name = 'a cart deploys turned ' .. o,
    ruleset = 'rs', scenario = 'field_duel', place = carts, steps = steps,
  }
end
for o = 0, 5 do
  test(turned(o, {
    act.deploy(0, 0, {{ 1, 0, 1, o }}),
    expect.event('figure_moved'),
    expect.figure('p0.test_cart#1', 'on_board'),
  }))
end
test {
  id = 'turned_outside', name = 'a turned rear cell cannot leave the start zone',
  ruleset = 'rs', scenario = 'field_duel', place = carts,
  steps = { act.deploy(0, 0, {{ 2, 0, 1, 0 }}) },
}
test {
  id = 'turned_empty', name = 'a turned rear cell cannot hang over empty space',
  ruleset = 'rs', scenario = 'field_duel', place = carts,
  steps = { act.deploy(0, 0, {{ 0, 0, 1, 3 }}) },
}
test {
  id = 'turned_spread', name = 'a turned rear cell cannot spread past the footprint',
  ruleset = 'rs', scenario = 'field_duel', place = carts,
  steps = { act.deploy(0, 0, {{ 2, 0, 1, 5 }}) },
}
test {
  id = 'turned_onto_soldier', name = 'a turned rear cell cannot be placed on a figure',
  ruleset = 'rs', scenario = 'field_duel', place = carts,
  steps = {
    act.deploy(0, 1, {{ 1, 1, 1 }, { 0, 1, 1 }}),
    act.deploy(1, 2, {{ 6, 0, 1 }, { 7, 0, 1 }}),
    act.deploy(0, 0, {{ 1, 0, 1, 1 }}),
  },
}
test {
  id = 'turned_too_far', name = 'a rotation past 5 is refused',
  ruleset = 'rs', scenario = 'field_duel', place = carts,
  steps = { act.deploy(0, 0, {{ 1, 0, 1, 6 }}) },
}
