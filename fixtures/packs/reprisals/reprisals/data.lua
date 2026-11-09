-- Pack tests for reprisals in the ordinary move action and the `provokes_reprisal` flag of
-- `core:movement`, with `core:engagement` in the ruleset. Figure 0 belongs to player 0 and moves;
-- figure 1 is player 1's enemy. Every column stands at level 1.
--
-- `survey` emits one `previewed` event per stop of `ctx.movement.preview`, carrying its
-- `{ q, r, o, flagged }`, so the Rust tests can read the flag.
stat { id = 'move', name = 'Move', min = 0, max = 9 }
stat { id = 'life', name = 'Life', min = 1, max = 9 }
marker { id = 'wound', name = 'Wound', stacking = 'count', visibility = 'public' }
die { id = 'combat', name = 'Combat', faces = { 'hit', 'blank' } }
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
local function at(q, r) return { piece = 'meadow', at = { q = q, r = r }, rot = 0, level = 0 } end

-- Open ground over q = -3..3, r = -1..1.
local field = {}
for q = -3, 3 do
  for r = -1, 1 do field[#field + 1] = at(q, r) end
end
map { id = 'field', name = 'Field', orientation = 'pointy', pieces = field }

-- A lane from (0, 0) through (1, -1) and (2, -1) to (3, -1), with the enemy's column at (0, -1).
-- (1, -1) is next to (0, -1); (2, -1) and (3, -1) are not, so the only route to (3, -1) passes
-- the enemy and then leaves it.
map {
  id = 'lane', name = 'Lane', orientation = 'pointy',
  pieces = { at(0, 0), at(0, -1), at(1, -1), at(2, -1), at(3, -1) },
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
          ctx.emit('previewed', { q = stop.q, r = stop.r, o = stop.o, flagged = stop.provokes_reprisal })
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
      engagement = engagement and { id = 'core:engagement' } or nil,
    },
    addons = {{ id = 'reprisals:rules' }},
  }
end
setup('engaged', true)
setup('plain', false)

local function pair(unit, q, r)
  return {
    { unit = unit, player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = q, r = r, z = 1 }} },
  }
end
local function survey(id, name, ruleset, unit, q, r)
  local who = 'p0.' .. unit .. '#1'
  test {
    id = id, name = name, ruleset = ruleset, map = 'field', place = pair(unit, q, r),
    steps = { act.activate(0, who), act.action(who, 'survey') },
  }
end
local function walk(id, name, map, unit, enemy, to, rig, expected)
  local who = 'p0.' .. unit .. '#1'
  local steps = { act.activate(0, who), act.action(who, 'walk', to, rig and { rig = { rig } }) }
  for _, e in ipairs(expected) do steps[#steps + 1] = e end
  test { id = id, name = name, ruleset = 'engaged', map = map, place = pair(unit, enemy[1], enemy[2]), steps = steps }
end

-- The enemy on (-1, 0) is next to the mover on (0, 0).
survey('flags', 'stops whose route leaves an engaged enemy are flagged', 'engaged', 'test_soldier', -1, 0)
survey('flags_plain', 'without an engagement module no stop is flagged', 'plain', 'test_soldier', -1, 0)
-- The enemy on (-1, 1) is next to the snake's anchor on (0, 0); its rear is on (1, 0).
survey('flags_snake', 'a snake is flagged only once no cell stays engaged', 'engaged', 'test_wyrm', -1, 1)

walk('pass_then_leave', 'a move that passes an enemy and then leaves it gives it one reprisal', 'lane',
  'test_soldier', { 0, -1 }, { 3, -1, 1 }, { 'blank' },
  { expect.event('reprisal'), expect.event('figure_moved', { figure = 'p0.test_soldier#1' }) })
walk('destroyed', 'a reprisal that destroys the mover stops the move', 'lane',
  'test_scout', { 0, -1 }, { 3, -1, 1 }, { 'hit' },
  { expect.event('figure_destroyed', { figure = 'p0.test_scout#1' }), expect.figure('p0.test_scout#1', 'destroyed') })
walk('snake_keeps', 'a snake that keeps contact through its rear provokes no reprisal', 'field',
  'test_wyrm', { -1, 1 }, { 1, 0, 1, 3 }, nil,
  { expect.event('figure_moved', { figure = 'p0.test_wyrm#1' }) })
walk('snake_leaves', 'a snake gives one reprisal once its rear leaves too', 'field',
  'test_wyrm', { -1, 1 }, { 2, 0, 1, 3 }, { 'blank' },
  { expect.event('reprisal'), expect.event('figure_moved', { figure = 'p0.test_wyrm#1' }) })
