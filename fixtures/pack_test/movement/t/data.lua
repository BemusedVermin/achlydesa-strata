-- The ordinary `move` action over a row of hexes, through `core:movement` and `core:activation`.
stat { id = 'move', name = 'Move', min = 0, max = 9 }
terrain { id = 'grass', name = 'Grass', standable = true }
terrain { id = 'mud', name = 'Mud', standable = true, tags = { 'stops_here' } }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'bog', name = 'Bog', terrain = 'mud', thickness = 1, shape = {{ q = 0, r = 0 }} }

local function row(mud_at)
  local pieces = {}
  for q = 0, 4 do
    local piece = (q == mud_at) and 'bog' or 'meadow'
    pieces[#pieces + 1] = { piece = piece, at = { q = q, r = 0 }, rot = 0, level = 0 }
  end
  return pieces
end
map { id = 'open', name = 'Open', orientation = 'pointy', pieces = row(-1) }
map { id = 'muddy', name = 'Muddy', orientation = 'pointy', pieces = row(2) }

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'soldier', name = 'Soldier', kind = 'squad', rarity = 'common', stats = { move = 3 },
  figures = {{
    id = 's', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}

rule_module {
  id = 'rules',
  register = function(r)
    r.action {
      id = 'walk', slots = { 'move' },
      targets = function(ctx, figure) return ctx.movement.reachable(figure) end,
      resolve = function(ctx, figure, q, r, z) ctx.movement.move(figure, q, r, z) end,
    }
  end,
}
ruleset {
  id = 'rs', name = 'Walk',
  slots = { activation = { id = 'core:activation' }, movement = { id = 'core:movement' } },
  addons = {{ id = 't:rules' }},
}

local me = 'p0.soldier#1'
local function at(player, q) return { unit = 'soldier', player = player, at = {{ q = q, r = 0, z = 1 }} } end
local function walk(id, name, map, place, to, expected)
  local steps = { act.activate(0, me), act.action(me, 'walk', to) }
  if expected then steps[#steps + 1] = expect.event(expected) end
  test { id = id, name = name, ruleset = 'rs', map = map, place = place, steps = steps }
end

walk('far', 'a figure walks as far as its move allows', 'open', { at(0, 0), at(1, 4) }, { 3, 0, 1 }, 'figure_moved')
walk('too_far', 'a figure cannot walk past its move', 'open', { at(0, 0), at(1, 4) }, { 4, 0, 1 })
walk('into_mud', 'a figure can enter stop-on-enter terrain', 'muddy', { at(0, 0), at(1, 4) }, { 2, 0, 1 }, 'figure_moved')
walk('past_mud', 'a figure cannot walk through stop-on-enter terrain', 'muddy', { at(0, 0), at(1, 4) }, { 3, 0, 1 })
walk('onto_figure', 'a figure cannot stop on an occupied space', 'open', { at(0, 0), at(1, 2) }, { 2, 0, 1 })
walk('through_figure', 'a figure can walk through an occupied space', 'open', { at(0, 0), at(1, 2) }, { 3, 0, 1 }, 'figure_moved')
