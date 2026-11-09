-- The ordinary `move` action over rows of hexes, through `core:movement` and `core:activation`:
-- walking, stop-on-enter terrain, occupied spaces, climbing, bridges, falls and rule queries.
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

-- A deck over q = 2 at level 3 (surface 4): a bridge with two free levels under it.
local bridge = row(-1)
bridge[#bridge + 1] = { piece = 'meadow', at = { q = 2, r = 0 }, rot = 0, level = 3 }
map { id = 'bridge', name = 'Bridge', orientation = 'pointy', pieces = bridge }

-- Steps up from q = 0 (surface 1) to q = 1 (surface 2) and q = 2 (surface 4).
local steps = {}
for q, level in ipairs({ 0, 1, 3 }) do
  steps[#steps + 1] = { piece = 'meadow', at = { q = q - 1, r = 0 }, rot = 0, level = level }
end
map { id = 'steps', name = 'Steps', orientation = 'pointy', pieces = steps }

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'soldier', name = 'Soldier', kind = 'squad', rarity = 'common', stats = { move = 3 },
  figures = {{
    id = 's', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}
unit {
  id = 'giant', name = 'Giant', kind = 'squad', rarity = 'common', stats = { move = 3 },
  figures = {{
    id = 'g', count = 1, footprint = 'one', height = 3,
    los = { target = 30, hit_zone = {0, 36}, samples = {'center'} },
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

-- Each of these changes one answer the movement module asks the rules.
rule_module {
  id = 'costly',
  register = function(r)
    r.modifier {
      query = 'move_cost', layer = 'add', value = function() return 1 end,
      when = function(source, subject, context) return context.terrain == 'grass' end,
    }
  end,
}
rule_module {
  id = 'no_climbing',
  register = function(r)
    r.modifier { query = 'can_climb', layer = 'set', value = function() return false end }
  end,
}
rule_module {
  id = 'locked_gate',
  register = function(r)
    r.modifier {
      query = 'can_enter', layer = 'set', value = function() return false end,
      when = function(source, subject, context) return context[1] == 2 end,
    }
  end,
}

local function setup(id, movement, addons)
  local list = {{ id = 't:rules' }}
  for _, name in ipairs(addons or {}) do list[#list + 1] = { id = 't:' .. name } end
  ruleset {
    id = id, name = id,
    slots = {
      activation = { id = 'core:activation' },
      movement = { id = 'core:movement', config = movement },
    },
    addons = list,
  }
end
setup('rs', {})
setup('climber', { max_climb = { Int = 2 } })
setup('tall_climber', { max_climb = { Int = 3 } })
setup('lenient', { headroom = { Str = 'ignore' } })
setup('costly', {}, { 'costly' })
setup('no_climbing', {}, { 'no_climbing' })
setup('locked_gate', {}, { 'locked_gate' })

local function at(player, q, z, unit)
  return { unit = unit or 'soldier', player = player, at = {{ q = q, r = 0, z = z or 1 }} }
end
local function walk(id, name, ruleset, map, place, who, to, expected)
  local steps = { act.activate(0, who), act.action(who, 'walk', to) }
  if expected then steps[#steps + 1] = expect.event(expected) end
  test { id = id, name = name, ruleset = ruleset, map = map, place = place, steps = steps }
end
local soldier, giant = 'p0.soldier#1', 'p0.giant#1'
local away = at(1, 4)
local top, foot = at(1, 2, 4), at(1, 0, 1)

-- Flat ground, stop-on-enter terrain and other figures.
walk('far', 'a figure walks as far as its move allows', 'rs', 'open', { at(0, 0), away }, soldier, { 3, 0, 1 }, 'figure_moved')
walk('too_far', 'a figure cannot walk past its move', 'rs', 'open', { at(0, 0), away }, soldier, { 4, 0, 1 })
walk('into_mud', 'a figure can enter stop-on-enter terrain', 'rs', 'muddy', { at(0, 0), away }, soldier, { 2, 0, 1 }, 'figure_moved')
walk('past_mud', 'a figure cannot walk through stop-on-enter terrain', 'rs', 'muddy', { at(0, 0), away }, soldier, { 3, 0, 1 })
walk('onto_figure', 'a figure cannot stop on an occupied space', 'rs', 'open', { at(0, 0), at(1, 2) }, soldier, { 2, 0, 1 })
walk('through_figure', 'a figure can walk through an occupied space', 'rs', 'open', { at(0, 0), at(1, 2) }, soldier, { 3, 0, 1 }, 'figure_moved')

-- Climbing: a rise of one is allowed by default, a rise of two needs `max_climb = 2`.
walk('climb_one', 'a figure climbs one level', 'rs', 'steps', { at(0, 0), top }, soldier, { 1, 0, 2 }, 'figure_moved')
walk('climb_two', 'a figure cannot climb two levels by default', 'rs', 'steps', { at(0, 1, 2), foot }, soldier, { 2, 0, 4 })
walk('climb_two_wide', 'a wider climb limit lets a figure climb two levels', 'climber', 'steps', { at(0, 1, 2), foot }, soldier, { 2, 0, 4 }, 'figure_moved')
walk('climb_vetoed', 'a can_climb veto stops a climb', 'no_climbing', 'steps', { at(0, 0), top }, soldier, { 1, 0, 2 })

-- Bridges: under it only if the figure fits, over it only by climbing, and a drop off it falls.
walk('under', 'a short figure walks under a bridge', 'rs', 'bridge', { at(0, 0), away }, soldier, { 3, 0, 1 }, 'figure_moved')
walk('too_tall', 'a tall figure cannot walk under a bridge', 'rs', 'bridge', { at(0, 0, 1, 'giant'), away }, giant, { 3, 0, 1 })
walk('lenient', 'headroom can be ignored', 'lenient', 'bridge', { at(0, 0, 1, 'giant'), away }, giant, { 3, 0, 1 }, 'figure_moved')
walk('over_blocked', 'a figure cannot climb onto a bridge deck by default', 'rs', 'bridge', { at(0, 1), away }, soldier, { 2, 0, 4 })
walk('over', 'a figure with a long climb can reach a bridge deck', 'tall_climber', 'bridge', { at(0, 1), away }, soldier, { 2, 0, 4 }, 'figure_moved')
walk('fall', 'a figure can step off a bridge and fall', 'rs', 'bridge', { at(0, 2, 4), away }, soldier, { 3, 0, 1 }, 'figure_moved')
walk('fall_ends_move', 'a fall ends the move', 'rs', 'bridge', { at(0, 2, 4), away }, soldier, { 4, 0, 1 })

-- Rule queries: move_cost and can_enter.
walk('costly', 'a move_cost modifier makes steps dearer', 'costly', 'open', { at(0, 0), away }, soldier, { 1, 0, 1 }, 'figure_moved')
walk('costly_far', 'a dearer step shortens the move', 'costly', 'open', { at(0, 0), away }, soldier, { 2, 0, 1 })
walk('gate', 'a can_enter veto closes a space', 'locked_gate', 'open', { at(0, 0), away }, soldier, { 2, 0, 1 })
walk('gate_around', 'a can_enter veto leaves other spaces open', 'locked_gate', 'open', { at(0, 0), away }, soldier, { 1, 0, 1 }, 'figure_moved')
