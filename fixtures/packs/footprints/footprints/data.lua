-- Pack tests for multi-space footprints and falling in `core:movement`: snake turns, rigid
-- translation, turning in place, the level-spread limit, cells over empty space, and falling
-- damage.
--
-- Two-space figures start with their anchor on (0, 0) and rotation 0, so their rear cell is on
-- (1, 0), east of the anchor. Rotation o puts the rear in direction o: 0 E, 1 SE, 2 SW, 3 W,
-- 4 NW, 5 NE. A two-space figure's targets are { q, r, z, o }; a one-space figure's { q, r, z }.
stat { id = 'move', name = 'Move', min = 0, max = 9 }
stat { id = 'life', name = 'Life', min = 1, max = 9 }
marker { id = 'wound', name = 'Wound', stacking = 'count', visibility = 'public' }
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
local function at(q, r, level) return { piece = 'meadow', at = { q = q, r = r }, rot = 0, level = level } end

-- Flat ground over q = -1..3, r = 0..1, all standing at level 1.
local field = {}
for q = -1, 3 do
  for r = 0, 1 do field[#field + 1] = at(q, r, 0) end
end
map { id = 'field', name = 'Field', orientation = 'pointy', pieces = field }

-- A one-level step along r = 0: q = 0..1 stand at level 1, q = 2..3 at level 2.
map {
  id = 'ledge', name = 'Ledge', orientation = 'pointy',
  pieces = { at(0, 0, 0), at(1, 0, 0), at(2, 0, 0), at(2, 0, 1), at(3, 0, 0), at(3, 0, 1) },
}

-- A tower at q = 0..1 standing at level 4 (a figure stands at z = 4), with ground at level 1 beyond it.
local cliff = {}
for q = 0, 1 do
  for level = 0, 3 do cliff[#cliff + 1] = at(q, 0, level) end
end
cliff[#cliff + 1] = at(2, 0, 0)
cliff[#cliff + 1] = at(3, 0, 0)
map { id = 'cliff', name = 'Cliff', orientation = 'pointy', pieces = cliff }

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
footprint { id = 'snake', name = 'Snake', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }}, move = 'snake', max_spread = 1 }
footprint { id = 'rigid', name = 'Rigid', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }}, move = 'rigid', max_spread = 1 }
footprint { id = 'flat', name = 'Flat', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }}, move = 'rigid', max_spread = 0 }

local function unit_of(id, footprint, move, life)
  unit {
    id = id, name = id, kind = 'squad', rarity = 'common', stats = { move = move, life = life },
    figures = {{
      id = 'f', count = 1, footprint = footprint, height = 1,
      los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
    }},
  }
end
unit_of('test_soldier', 'one', 3, 2)
unit_of('test_scout', 'one', 3, 1)
unit_of('test_wyrm', 'snake', 1, 2)
unit_of('test_cart', 'rigid', 1, 2)
unit_of('test_sled', 'flat', 1, 2)

rule_module {
  id = 'rules',
  register = function(r)
    r.action {
      id = 'walk', slots = { 'move' },
      targets = function(ctx, figure) return ctx.movement.reachable(figure) end,
      resolve = function(ctx, figure, q, r, z, o) ctx.movement.move(figure, q, r, z, o) end,
    }
    -- No target list: `move` picks the rotation itself.
    r.action {
      id = 'stride', slots = { 'move' },
      resolve = function(ctx, figure, q, r, z) ctx.movement.move(figure, q, r, z) end,
    }
  end,
}

local function setup(id, movement, engagement)
  ruleset {
    id = id, name = id,
    slots = {
      activation = { id = 'core:activation' },
      movement = { id = 'core:movement', config = movement },
      units = { id = 'core:units' },
      engagement = engagement and { id = 'core:engagement' } or nil,
    },
    addons = {{ id = 'footprints:rules' }},
  }
end
setup('rs', {})
setup('hard_falls', { fall_wounds = { Int = 1 } })
setup('engaged', {}, true)

-- Where each map keeps player 1's figure, out of the way.
local enemy = { field = { q = 3, r = 1, z = 1 }, ledge = { q = 3, r = 0, z = 2 }, cliff = { q = 3, r = 0, z = 1 } }
local function place(map, unit, q, z)
  return {
    { unit = unit, player = 0, at = {{ q = q or 0, r = 0, z = z or 1 }} },
    { unit = 'test_soldier', player = 1, at = { enemy[map] } },
  }
end
local function walk(id, name, ruleset, map, unit, to, expected, action, q, z)
  local who = 'p0.' .. unit .. '#1'
  local steps = { act.activate(0, who), act.action(who, action or 'walk', to) }
  for _, e in ipairs(expected or {}) do steps[#steps + 1] = e end
  test { id = id, name = name, ruleset = ruleset, map = map, place = place(map, unit, q, z), steps = steps }
end
local moved = { expect.event('figure_moved') }

-- Snake: the rear follows the leader, so a step south-east swings the rear to the north-west.
walk('snake_turn', 'a snake turns as its rear follows the leader', 'rs', 'field', 'test_wyrm', { 0, 1, 1, 4 }, moved)
walk('snake_reverse', 'a snake reverses by stepping onto its rear', 'rs', 'field', 'test_wyrm', { 1, 0, 1, 3 }, moved)

-- Rigid: a step translates the shape and keeps its rotation.
walk('rigid_slide', 'a rigid shape translates', 'rs', 'field', 'test_cart', { 0, 1, 1, 0 }, moved)

-- Turning in place (#187): a two-space figure pivots about its anchor at no cost, one 60-degree
-- turn at a time, and every rotation the rear sweeps through must be a legal placement.
walk('rigid_pivot', 'a rigid two-space figure turns in place at no cost', 'rs', 'field', 'test_cart', { 0, 0, 1, 2 }, moved)
-- Facing west takes three free turns; the cart's one point of movement then pays for the step.
walk('rigid_pivot_move', 'a rigid two-space figure turns in place and then moves', 'rs', 'field', 'test_cart', { 0, 1, 1, 3 }, moved)
walk('snake_pivot', 'a snake turns in place at no cost', 'rs', 'field', 'test_wyrm', { 0, 0, 1, 1 }, moved)
-- After its step south-east, the snake turns its rear from (0, 0) round to (1, 1).
walk('snake_step_pivot', 'a snake steps and then turns in place', 'rs', 'field', 'test_wyrm', { 0, 1, 1, 0 }, moved)
-- North-east of the anchor, (1, -1), is off the map.
walk('pivot_no_surface', 'a turn cannot put the rear cell where there is no surface', 'rs', 'field', 'test_cart', { 0, 0, 1, 5 })

-- No cell may end over empty space: from (2, 0) a step east would leave the rear on (4, 0).
walk('edge', 'a rear cell cannot end over empty space', 'rs', 'field', 'test_cart', { 3, 0, 1, 0 }, nil, nil, 2)
walk('inside', 'a footprint can walk up to the edge', 'rs', 'field', 'test_cart', { 1, 0, 1, 0 }, moved)

-- Level spread: straddling the one-level step needs `max_spread = 1`.
walk('spread_ok', 'a footprint may straddle a step within its spread', 'rs', 'ledge', 'test_cart', { 1, 0, 1, 0 }, moved)
walk('spread_too_wide', 'a footprint cannot straddle a step wider than its spread', 'rs', 'ledge', 'test_sled', { 1, 0, 1, 0 })

-- Without a target list `move` takes the cheapest rotation, then the one reached with the fewest
-- pivots, then the lowest.
walk('stride', 'without a rotation a move takes the cheapest one', 'rs', 'field', 'test_wyrm', { 0, 1, 1 }, moved, 'stride')
-- Without a rotation, a move to the figure's own space is refused rather than turning it: turning
-- in place takes an explicit `o`.
walk('stride_in_place', 'without a rotation a move to its own space is refused', 'rs', 'field', 'test_cart', { 0, 0, 1 }, nil, 'stride')

-- Falling: a three-level drop is one level beyond `max_fall = 2`.
walk('fall_harmless', 'a fall deals no damage by default', 'rs', 'cliff', 'test_soldier', { 2, 0, 1 },
  { expect.event('figure_moved'), expect.event('figure_fell') }, nil, 1, 4)
walk('fall_wounds', 'a fall wounds per level beyond the limit when configured', 'hard_falls', 'cliff', 'test_soldier', { 2, 0, 1 },
  { expect.event('figure_fell'), expect.event('wounds_applied', { figure = 'p0.test_soldier#1' }), expect.figure('p0.test_soldier#1', 'on_board') }, nil, 1, 4)
walk('fall_deadly', 'a fall can destroy a figure', 'hard_falls', 'cliff', 'test_scout', { 2, 0, 1 },
  { expect.event('figure_fell'), expect.figure('p0.test_scout#1', 'destroyed') }, nil, 1, 4)

-- With `core:engagement` in the ruleset, every step of a two-space body asks it about reprisals
-- for the body as a whole. The enemy on (-1, 1) is next to the anchor; the snake moves away.
test {
  id = 'engaged_snake', name = 'a snake next to an enemy moves with engagement in the ruleset',
  ruleset = 'engaged', map = 'field',
  place = {
    { unit = 'test_wyrm', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = -1, r = 1, z = 1 }} },
  },
  steps = {
    act.activate(0, 'p0.test_wyrm#1'),
    act.action('p0.test_wyrm#1', 'walk', { 1, 0, 1, 3 }),
    expect.event('figure_moved', { figure = 'p0.test_wyrm#1' }),
  },
}

-- Footprint-aware occupancy, per-cell climbing and surface fallback (#186, #190).

-- A rigid cart and a tall rigid cart, each two spaces with a wider spread.
footprint { id = 'wide', name = 'Wide', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }}, move = 'rigid', max_spread = 2 }
unit_of('test_wagon', 'wide', 1, 2)
unit {
  id = 'test_tall_cart', name = 'test_tall_cart', kind = 'squad', rarity = 'common', stats = { move = 1, life = 2 },
  figures = {{
    id = 'f', count = 1, footprint = 'rigid', height = 2,
    los = { target = 10, hit_zone = {0, 24}, samples = {'center'} },
  }},
}

-- (0, 0), (1, 0) and (0, 1) stand at level 1; (1, 1) stands at level 3.
map {
  id = 'steep', name = 'Steep', orientation = 'pointy',
  pieces = { at(0, 0, 0), at(1, 0, 0), at(0, 1, 0), at(1, 1, 0), at(1, 1, 1), at(1, 1, 2), at(-1, 0, 0) },
}

-- Like `steep`, but (1, 1) stands at level 2: one level above its neighbours.
map {
  id = 'gentle', name = 'Gentle', orientation = 'pointy',
  pieces = { at(0, 0, 0), at(1, 0, 0), at(0, 1, 0), at(1, 1, 0), at(1, 1, 1), at(-1, 0, 0) },
}

-- (-1, 0) and (0, 0) stand at level 2. Column (1, 0) has ground at 1 under a shelf with one free
-- level, and the shelf's top at 3: both a level from the anchor, the ground nearer on a tie.
-- Player 0 deploys on (-1, 0) to (1, 0), player 1 on (-2, 0), which stands at level 1.
map {
  id = 'shelf', name = 'Shelf', orientation = 'pointy',
  pieces = { at(-1, 0, 1), at(0, 0, 1), at(1, 0, 0), at(1, 0, 2), at(-2, 0, 0) },
  zones = {
    west = {{ q = -1, r = 0 }, { q = 0, r = 0 }, { q = 1, r = 0 }},
    east = {{ q = -2, r = 0 }},
  },
}

local function solo(id, name, map, unit, from, to, expected)
  local who = 'p0.' .. unit .. '#1'
  local steps = { act.activate(0, who), act.action(who, 'walk', to) }
  for _, e in ipairs(expected or {}) do steps[#steps + 1] = e end
  test {
    id = id, name = name, ruleset = 'rs', map = map,
    place = {
      { unit = unit, player = 0, at = { from } },
      { unit = 'test_soldier', player = 1, at = {{ q = -1, r = 0, z = 1 }} },
    },
    steps = steps,
  }
end
-- Sliding south-east, the rear would rise two levels: within the spread, beyond `max_climb`.
solo('rear_climb', 'a rigid rear cell cannot climb further than the anchor may', 'steep', 'test_wagon',
  { q = 0, r = 0, z = 1 }, { 0, 1, 1, 0 })
-- The same slide with the rear rising one level is legal.
solo('rear_climb_ok', 'a rigid rear cell may climb as far as the anchor may', 'gentle', 'test_wagon',
  { q = 0, r = 0, z = 1 }, { 0, 1, 1, 0 }, moved)
-- A figure two levels tall: the rear's nearest surface is under the shelf, so it stands on top.
test {
  id = 'shelf_top', name = 'a rear cell without headroom stands on the next surface up', ruleset = 'rs', map = 'shelf',
  place = {
    { unit = 'test_tall_cart', player = 0, at = {{ q = -1, r = 0, z = 2 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = -2, r = 0, z = 1 }} },
  },
  steps = {
    act.activate(0, 'p0.test_tall_cart#1'),
    act.action('p0.test_tall_cart#1', 'walk', { 0, 0, 2, 0 }),
    expect.event('figure_moved'),
  },
}
-- The soldier on (2, 1) cannot stop on the cart's rear cell on (3, 0).
test {
  id = 'onto_rear', name = 'a figure cannot stop on another figure\'s rear cell', ruleset = 'rs', map = 'field',
  place = {
    { unit = 'test_soldier', player = 0, at = {{ q = 2, r = 1, z = 1 }} },
    { unit = 'test_cart', player = 1, at = {{ q = 2, r = 0, z = 1 }} },
  },
  steps = { act.activate(0, 'p0.test_soldier#1'), act.action('p0.test_soldier#1', 'walk', { 3, 0, 1 }) },
}
-- Placing the soldier on the cart's rear cell is refused before the test starts.
test {
  id = 'placed_on_rear', name = 'a test cannot place a figure on another figure\'s rear cell', ruleset = 'rs', map = 'field',
  place = {
    { unit = 'test_cart', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 1, r = 0, z = 1 }} },
  },
  steps = {},
}
-- A test cannot place a figure above the ground, where there is no surface.
test {
  id = 'placed_in_air', name = 'a test cannot place a figure where there is no surface', ruleset = 'rs', map = 'field',
  place = {
    { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 5 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 3, r = 1, z = 1 }} },
  },
  steps = {},
}

-- Deployment stands cells where movement would: the tall cart's rear cannot fit under the shelf,
-- so it is placed on the shelf's top, and the cart can then walk west off it.
scenario {
  id = 'shelf_duel', name = 'Shelf duel', map = 'shelf', players = { min = 2, max = 2 },
  slots = { { id = 'a', start = 'west', deploy_order = 1 }, { id = 'b', start = 'east', deploy_order = 2 } },
}
ruleset {
  id = 'deploys', name = 'deploys',
  slots = {
    deployment = { id = 'core:deployment' },
    activation = { id = 'core:activation' },
    movement = { id = 'core:movement' },
    units = { id = 'core:units' },
  },
  addons = {{ id = 'footprints:rules' }},
}
test {
  id = 'deploy_shelf', name = 'a deployed rear cell without headroom stands on the next surface up',
  ruleset = 'deploys', scenario = 'shelf_duel',
  place = {
    { unit = 'test_tall_cart', player = 0, at = {} },
    { unit = 'test_soldier', player = 1, at = {{ q = -2, r = 0, z = 1 }} },
  },
  steps = {
    act.command(0, 'place_unit', { unit = 0, at = {{ 0, 0, 2 }} }),
    expect.event('deployment_done'),
    act.activate(0, 'p0.test_tall_cart#1'),
    act.action('p0.test_tall_cart#1', 'walk', { -1, 0, 2, 0 }),
    expect.event('figure_moved'),
  },
}

-- Turning in place, blocked and refused (#187). The soldier on (0, 1), south-east of the cart's
-- anchor, is in the way of the rear's first clockwise turn, and north-east of the anchor is off
-- the map, so the rear cannot sweep round to the south-west either.
test {
  id = 'pivot_occupied', name = 'a turn cannot sweep the rear cell through another figure', ruleset = 'rs', map = 'field',
  place = {
    { unit = 'test_cart', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 0, r = 1, z = 1 }} },
  },
  steps = { act.activate(0, 'p0.test_cart#1'), act.action('p0.test_cart#1', 'walk', { 0, 0, 1, 2 }) },
}
-- A three-space rigid shape never pivots, though every cell of the turned shape stands on the field.
footprint { id = 'tri', name = 'Tri', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }, { q = 0, r = 1 }}, move = 'rigid', max_spread = 1 }
unit_of('test_wain', 'tri', 1, 2)
walk('tri_no_pivot', 'a three-space rigid shape cannot turn in place', 'rs', 'field', 'test_wain', { 0, 0, 1, 1 })
