-- Pack tests for `core:relics`. The native `toy:driver` module supplies `move_to`, a stand-in
-- for the movement action: { figure, q, r, z } puts a figure there. Figures are numbered in the
-- order the tests place them, so player 0's soldier is figure 0 and player 1's is figure 1.
stat { id = 'attack', name = 'Attack', min = 0, max = 9 }
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
map {
  id = 'row', name = 'Row', orientation = 'pointy',
  pieces = {
    { piece = 'meadow', at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = 'meadow', at = { q = 1, r = 0 }, rot = 0, level = 0 },
    { piece = 'meadow', at = { q = 2, r = 0 }, rot = 0, level = 0 },
  },
  zones = { west = {{ q = 0, r = 0 }}, east = {{ q = 2, r = 0 }} },
  relic_spots = { middle = { q = 1, r = 0 } },
}
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = { attack = 3 }, figures = {{
    id = 's', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}

relic {
  id = 'fury', name = 'Relic of Fury', mode = 'while_occupied', claim = 'ends_move_on',
  face_down_allowed = true,
  grants = { to = 'occupant_army', abilities = {{
    name = 'Fury', text = 'Add 2 attack.', scope = 'holder',
    modifiers = {{ query = 'attack', layer = 'add', value = function() return 2 end }},
  }} },
}
relic {
  id = 'boon', name = 'Relic of Boon', mode = 'on_claim', claim = 'ends_move_on',
  effect = function(ctx) ctx.emit('boon_taken', 1) end,
}

local function slot(id, start, order) return { id = id, start = start, deploy_order = order } end
local function scn(id, relic, face_down)
  return {
    id = id, name = id, map = 'row', players = { min = 2, max = 2 },
    slots = { slot('a', 'west', 1), slot('b', 'east', 2) },
    relics = {{ relic = relic, spot = 'middle', face_down = face_down }},
  }
end
scenario(scn('fury_up', 'fury', false))
scenario(scn('fury_down', 'fury', true))
scenario(scn('boon_up', 'boon', false))

ruleset {
  id = 'rs', name = 'Relics',
  slots = { relics = { id = 'core:relics' } }, addons = {{ id = 'toy:driver' }},
}

local function board()
  return {
    { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 2, r = 0, z = 1 }} },
  }
end
local function go(figure, q) return { figure = figure, q = q, r = 0, z = 1 } end

test {
  id = 'claim', name = 'ending a move on a relic claims it and its bonus applies',
  ruleset = 'rs', scenario = 'fury_up', place = board(),
  steps = {
    expect.stat('p0.test_soldier#1', 'attack', 3),
    act.command(0, 'move_to', go(0, 1)),
    expect.event('relic_claimed'),
    expect.stat('p0.test_soldier#1', 'attack', 5),
    expect.stat('p1.test_soldier#1', 'attack', 3),
  },
}
test {
  id = 'leave', name = 'the bonus disappears exactly when the occupant leaves',
  ruleset = 'rs', scenario = 'fury_up', place = board(),
  steps = {
    act.command(0, 'move_to', go(0, 1)),
    expect.stat('p0.test_soldier#1', 'attack', 5),
    act.command(0, 'move_to', go(0, 0)),
    expect.event('relic_released'),
    expect.event('ability_detached'),
    expect.stat('p0.test_soldier#1', 'attack', 3),
  },
}
test {
  id = 'swap', name = 'the bonus passes to the next player to occupy the relic',
  ruleset = 'rs', scenario = 'fury_up', place = board(),
  steps = {
    act.command(0, 'move_to', go(0, 1)),
    act.command(0, 'move_to', go(0, 0)),
    act.command(1, 'move_to', go(1, 1)),
    expect.stat('p1.test_soldier#1', 'attack', 5),
    expect.stat('p0.test_soldier#1', 'attack', 3),
  },
}
test {
  id = 'elsewhere', name = 'ending a move away from the relic claims nothing',
  ruleset = 'rs', scenario = 'fury_up', place = board(),
  steps = {
    act.command(0, 'move_to', go(0, 0)),
    expect.stat('p0.test_soldier#1', 'attack', 3),
  },
}
test {
  id = 'reveal', name = 'claiming a face-down relic turns it face up',
  ruleset = 'rs', scenario = 'fury_down', place = board(),
  steps = {
    act.command(0, 'move_to', go(0, 0)),
    act.command(1, 'move_to', go(1, 1)),
    expect.event('relic_revealed'),
    expect.event('relic_claimed'),
    expect.stat('p1.test_soldier#1', 'attack', 5),
  },
}
test {
  id = 'on_claim', name = 'an on-claim relic runs its effect for the first to claim it',
  ruleset = 'rs', scenario = 'boon_up', place = board(),
  steps = {
    act.command(0, 'move_to', go(0, 1)),
    expect.event('relic_claimed'),
    expect.event('boon_taken'),
  },
}
