-- Pack tests for reprisals in the ordinary move action and the `provokes_reprisal` flag of
-- `core:movement`, with `core:engagement` in the ruleset. Figure 0 belongs to player 0 and moves;
-- figure 1 is player 1's enemy. Every column stands at level 1, except the spire of `loop`.
--
-- `survey` emits one `previewed` event per stop of `ctx.movement.preview`, carrying its
-- `{ q, r, o, flagged, path }`, so the Rust tests can read the flag and the route.
stat { id = 'move', name = 'Move', min = 0, max = 9 }
stat { id = 'life', name = 'Life', min = 1, max = 9 }
marker { id = 'wound', name = 'Wound', stacking = 'count', visibility = 'public' }
die { id = 'combat', name = 'Combat', faces = { 'hit', 'blank' } }
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'spire', name = 'Spire', terrain = 'grass', thickness = 3, shape = {{ q = 0, r = 0 }} }
local function at(q, r, piece) return { piece = piece or 'meadow', at = { q = q, r = r }, rot = 0, level = 0 } end

-- Open ground over q = -3..3, r = -1..1.
local field = {}
for q = -3, 3 do
  for r = -1, 1 do field[#field + 1] = at(q, r) end
end
map { id = 'field', name = 'Field', orientation = 'pointy', pieces = field }

-- A lane from (0, 0) through (1, -1) and (2, -1) to (3, -1), with the enemy's column at (0, -1).
-- (0, 0) and (1, -1) are next to (0, -1); (2, -1) and (3, -1) are not, so the only route to
-- (3, -1) starts engaged with the enemy and then leaves it.
map {
  id = 'lane', name = 'Lane', orientation = 'pointy',
  pieces = { at(0, 0), at(0, -1), at(1, -1), at(2, -1), at(3, -1) },
}

-- A detour from (0, 0) through (1, 0) to (1, 1), with the enemy's column at (2, -1). Only (1, 0)
-- is next to it, so the route to (1, 1) starts and ends clear of the enemy and passes it midway.
map {
  id = 'detour', name = 'Detour', orientation = 'pointy',
  pieces = { at(0, 0), at(1, 0), at(1, 1), at(2, -1) },
}

-- A loop around the enemy's spire at (2, -1): (0, 0), (1, 0), (1, 1), (2, 1), (3, 0), (3, -1),
-- (4, -2). Of these only (1, 0) and (3, -1) are next to the spire, so the route to (4, -2)
-- engages the enemy, leaves it, engages it again and leaves again. The spire stands two levels
-- above the loop, too high to climb, so no route cuts across it; the `reach` ruleset ignores
-- heights so the enemy on it is still engaged.
map {
  id = 'loop', name = 'Loop', orientation = 'pointy',
  pieces = { at(0, 0), at(1, 0), at(1, 1), at(2, 1), at(3, 0), at(3, -1), at(4, -2), at(2, -1, 'spire') },
}

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
footprint { id = 'snake', name = 'Snake', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }}, move = 'snake', max_spread = 1 }

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
unit_of('test_wyrm', 'snake', 2, 2)
unit_of('test_runner', 'one', 6, 2)

rule_module {
  id = 'rules',
  register = function(r)
    r.action {
      id = 'walk', slots = { 'move' },
      targets = function(ctx, figure) return ctx.movement.reachable(figure) end,
      resolve = function(ctx, figure, q, r, z, o) ctx.movement.move(figure, q, r, z, o) end,
    }
    r.action {
      id = 'survey', slots = { 'move' },
      resolve = function(ctx, figure)
        for _, stop in ipairs(ctx.movement.preview(figure)) do
          ctx.emit('previewed', { q = stop.q, r = stop.r, o = stop.o, flagged = stop.provokes_reprisal, path = stop.path })
        end
      end,
    }
  end,
}

local function setup(id, engagement)
  ruleset {
    id = id, name = id,
    slots = {
      activation = { id = 'core:activation' },
      movement = { id = 'core:movement' },
      units = { id = 'core:units' },
      engagement = engagement,
    },
    addons = {{ id = 'reprisals:rules' }},
  }
end
setup('engaged', { id = 'core:engagement' })
setup('plain', nil)
setup('reach', { id = 'core:engagement', config = { heights = { Str = 'ignore' } } })

local function pair(unit, q, r, z)
  return {
    { unit = unit, player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = q, r = r, z = z or 1 }} },
  }
end
local function survey(id, name, ruleset, unit, q, r, map)
  local who = 'p0.' .. unit .. '#1'
  test {
    id = id, name = name, ruleset = ruleset, map = map or 'field', place = pair(unit, q, r),
    steps = { act.activate(0, who), act.action(who, 'survey') },
  }
end
local function walk(id, name, map, unit, enemy, to, rig, expected, ruleset)
  local who = 'p0.' .. unit .. '#1'
  local steps = { act.activate(0, who), act.action(who, 'walk', to, rig and { rig = { rig } }) }
  for _, e in ipairs(expected) do steps[#steps + 1] = e end
  test {
    id = id, name = name, ruleset = ruleset or 'engaged', map = map,
    place = pair(unit, enemy[1], enemy[2], enemy[3]), steps = steps,
  }
end

-- The enemy on (-1, 0) is next to the mover on (0, 0).
survey('flags', 'stops whose route leaves an engaged enemy are flagged', 'engaged', 'test_soldier', -1, 0)
survey('flags_plain', 'without an engagement module no stop is flagged', 'plain', 'test_soldier', -1, 0)
-- The enemy on (-1, 1) is next to the snake's anchor on (0, 0); its rear is on (1, 0).
survey('flags_snake', 'a snake is flagged only once no cell stays engaged', 'engaged', 'test_wyrm', -1, 1)
-- The enemy on (2, -1) is next to (1, 0) only.
survey('preview_detour', 'a preview carries the route past an enemy and flags it', 'engaged', 'test_soldier', 2, -1, 'detour')

walk('engaged_then_leave', 'a move that starts engaged and then leaves an enemy gives it one reprisal', 'lane',
  'test_soldier', { 0, -1 }, { 3, -1, 1 }, { 'blank' },
  { expect.event('reprisal'), expect.event('figure_moved', { figure = 'p0.test_soldier#1' }) })
walk('pass_then_leave', 'a move that passes an enemy midway and then leaves it gives it one reprisal', 'detour',
  'test_soldier', { 2, -1 }, { 1, 1, 1 }, { 'blank' },
  { expect.event('reprisal'), expect.event('figure_moved', { figure = 'p0.test_soldier#1' }) })
walk('leave_twice', 'a move that leaves the same enemy twice gives it one reprisal', 'loop',
  'test_runner', { 2, -1, 3 }, { 4, -2, 1 }, { 'blank' },
  { expect.event('reprisal'), expect.event('figure_moved', { figure = 'p0.test_runner#1' }) }, 'reach')
walk('destroyed', 'a reprisal that destroys the mover stops the move', 'lane',
  'test_scout', { 0, -1 }, { 3, -1, 1 }, { 'hit' },
  { expect.event('figure_destroyed', { figure = 'p0.test_scout#1' }), expect.figure('p0.test_scout#1', 'destroyed') })
walk('snake_keeps', 'a snake that keeps contact through its rear provokes no reprisal', 'field',
  'test_wyrm', { -1, 1 }, { 1, 0, 1, 3 }, nil,
  { expect.event('figure_moved', { figure = 'p0.test_wyrm#1' }) })
walk('snake_leaves', 'a snake gives one reprisal once its rear leaves too', 'field',
  'test_wyrm', { -1, 1 }, { 2, 0, 1, 3 }, { 'blank' },
  { expect.event('reprisal'), expect.event('figure_moved', { figure = 'p0.test_wyrm#1' }) })

-- A two-space enemy engages with its rear cell too (#190). Its anchor is on (-2, 0), two columns
-- from the mover on (0, 0); its rear on (-1, 0) is next to the mover.
footprint { id = 'rigid', name = 'Rigid', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }}, move = 'rigid', max_spread = 1 }
unit_of('test_cart', 'rigid', 1, 2)
test {
  id = 'rear_engages', name = 'leaving an enemy that is engaged only through its rear cell gives it a reprisal',
  ruleset = 'engaged', map = 'field',
  place = {
    { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_cart', player = 1, at = {{ q = -2, r = 0, z = 1 }} },
  },
  steps = {
    act.activate(0, 'p0.test_soldier#1'),
    act.action('p0.test_soldier#1', 'walk', { 2, 0, 1 }, { rig = { { 'blank' } } }),
    expect.event('reprisal'),
    expect.event('figure_moved', { figure = 'p0.test_soldier#1' }),
  },
}
