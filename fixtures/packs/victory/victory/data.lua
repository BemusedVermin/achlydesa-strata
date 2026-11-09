-- Pack tests for `core:victory`: each `win.*` condition ending a match, ties between conditions,
-- a Lua condition, and commands refused once the match is over. Player 0 sits in slot `a`,
-- player 1 in slot `b`, each a team of their own.
--
-- A round is one turn each: `start_round` rolls the initiative die (`c` beats `a`, so player 0
-- goes first), then each player activates a unit and ends the activation. The round ends after
-- player 1's turn. Points: test_soldier 30, test_archer 50, test_captain 60.
stat { id = 'life', name = 'Life', min = 1, max = 9 }
stat { id = 'range', name = 'Range', min = 0, max = 9 }
stat { id = 'attack', name = 'Attack', min = 0, max = 9 }
stat { id = 'defense', name = 'Defense', min = 0, max = 9 }
stat { id = 'points', name = 'Points', min = 0, max = 999 }
marker { id = 'wound', name = 'Wound', stacking = 'count', visibility = 'public' }
die { id = 'combat', name = 'Combat', faces = { 'hit', 'hit', 'hit', 'block', 'block', 'blank' } }
die { id = 'init', name = 'Initiative', faces = { 'a', 'b', 'c' } }
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }

-- One row, q = 0..5 on r = 0, all at level 1. The middle zone is q = 2 and q = 3.
local pieces = {}
for q = 0, 5 do pieces[#pieces + 1] = { piece = 'meadow', at = { q = q, r = 0 }, rot = 0, level = 0 } end
map {
  id = 'field', name = 'Field', orientation = 'pointy', pieces = pieces,
  zones = {
    west = {{ q = 0, r = 0 }, { q = 1, r = 0 }},
    middle = {{ q = 2, r = 0 }, { q = 3, r = 0 }},
    east = {{ q = 4, r = 0 }, { q = 5, r = 0 }},
  },
}

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
local function unit_of(id, points)
  unit {
    id = id, name = id, kind = 'squad', rarity = 'common',
    stats = { life = 1, range = 1, attack = 2, defense = 2, points = points },
    figures = {{
      id = 'f', count = 1, footprint = 'one', height = 1,
      los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
    }},
  }
end
unit_of('test_soldier', 30)
unit_of('test_archer', 50)
unit_of('test_captain', 60)

local function scenario_of(id, victory)
  scenario {
    id = id, name = id, map = 'field', players = { min = 2, max = 2 },
    slots = {{ id = 'a', start = 'west', deploy_order = 1 }, { id = 'b', start = 'east', deploy_order = 2 }},
    victory = victory,
  }
end
scenario_of('last_stand', { win.eliminate_all() })
scenario_of('hill', { win.hold_zone('middle', { rounds = 2 }) })
scenario_of('short', { win.round_limit(1, { tiebreak = 'points_remaining' }) })
scenario_of('short_figures', { win.round_limit(1, { tiebreak = { 'points_remaining', 'figures_remaining' } }) })
scenario_of('short_draw', { win.round_limit(1) })
-- Both conditions are met at the end of round 1; the first listed decides.
scenario_of('ties', { win.hold_zone('middle'), win.round_limit(1, { tiebreak = 'points_remaining' }) })
scenario_of('ties_reversed', { win.round_limit(1, { tiebreak = 'points_remaining' }), win.hold_zone('middle') })
-- Player 1 (team 1) wins at the first round end.
scenario_of('scripted', {
  function(_, moment)
    if moment == 'round_end' then return 1 end
  end,
})

-- Conditions are checked after every command, then at the end of each round it ended. This one
-- counts commands: the fifth ends round 1, and since it is checked as a command first, team 0
-- wins there before the round end would give the match to team 1.
scenario_of('command_first', {
  function(ctx, moment)
    local seen = ctx.vars('test')
    if moment == 'command' then
      seen.commands = (seen.commands or 0) + 1
      if seen.commands == 5 then return 0 end
    elseif moment == 'round_end' then
      return 1
    end
  end,
})
-- Team 0 wins as soon as the first command resolves, for sessions that play to the end.
scenario_of('first_command', {
  function(_, moment)
    if moment == 'command' then return 0 end
  end,
})
-- A condition that fails while the match is played is skipped, and the next one still applies.
scenario_of('broken', { function() error('boom') end, win.round_limit(1) })
-- Malformed conditions, refused when a match is set up on them.
scenario_of('bad_zone', { win.hold_zone('nowhere') })
scenario_of('bad_tiebreak', { win.round_limit(3, { tiebreak = 'initiative_roll' }) })
scenario_of('bad_kind', { { win = 'king_of_the_hill' } })

ruleset {
  id = 'rs', name = 'Victory',
  slots = {
    turns = { id = 'core:rounds', config = {
      initiative = { Str = 'init' }, tie = { Str = 'reroll' }, turns_per_round = { Int = 1 },
    } },
    activation = { id = 'core:activation', config = { steps = { List = {{ Str = 'attack' }} } } },
    units = { id = 'core:units' },
    los = { id = 'core:los' },
    combat = { id = 'core:combat' },
    victory = { id = 'core:victory' },
  },
}

local function at(unit, player, q)
  return { unit = unit, player = player, at = {{ q = q, r = 0, z = 1 }} }
end
local function reserve(unit, player) return { unit = unit, player = player, at = {} } end
local start = act.command(0, 'start_round', nil, { rig = {{ 'c' }, { 'a' }} })
local function round(p0, p1)
  return {
    start,
    act.activate(0, p0), act.end_activation(0),
    act.activate(1, p1), act.end_activation(1),
  }
end
local function steps(...)
  local out = {}
  for _, part in ipairs { ... } do
    if part[1] then
      for _, step in ipairs(part) do out[#out + 1] = step end
    else
      out[#out + 1] = part
    end
  end
  return out
end
local function play(id, name, scenario, place, list)
  test { id = id, name = name, ruleset = 'rs', scenario = scenario, place = place, steps = list }
end
local me, foe = 'p0.test_soldier#1', 'p1.test_soldier#1'
local kill = act.attack(me, foe, { rig = {{ 'hit', 'hit' }, { 'blank', 'blank' }} })

play('eliminate', 'eliminate_all: destroying the last enemy figure wins', 'last_stand',
  { at('test_soldier', 0, 2), at('test_soldier', 1, 3) },
  steps(start, act.activate(0, me), expect.result(), kill,
    expect.event('match_ended'), expect.result('eliminate_all', { 0 })))
play('eliminate_reserve', 'eliminate_all: a figure in reserve keeps its team in the match',
  'last_stand',
  { at('test_soldier', 0, 2), at('test_soldier', 1, 3), reserve('test_archer', 1) },
  steps(start, act.activate(0, me), kill, expect.figure(foe, 'destroyed'), expect.result()))
play('over', 'refused: no command is taken once the match is over', 'last_stand',
  { at('test_soldier', 0, 2), at('test_soldier', 1, 3) },
  steps(start, act.activate(0, me), kill, act.end_activation(0)))

play('hold', 'hold_zone: holding the zone at enough round ends in a row wins', 'hill',
  { at('test_soldier', 0, 2), at('test_soldier', 1, 5) },
  steps(round(me, foe), expect.event('round_ended'), expect.result(),
    round(me, foe), expect.event('match_ended'), expect.result('hold_zone', { 0 })))
play('contested', 'hold_zone: a zone both teams stand in is held by nobody', 'hill',
  { at('test_soldier', 0, 2), at('test_soldier', 1, 3) },
  steps(round(me, foe), round(me, foe), expect.result()))

play('limit', 'round_limit: the end of the last round ends the match, by points remaining',
  'short', { at('test_soldier', 0, 0), at('test_archer', 1, 5) },
  steps(start, act.activate(0, me), act.end_activation(0), expect.result(),
    act.activate(1, 'p1.test_archer#1'), act.end_activation(1),
    expect.result('round_limit', { 1 })))
play('limit_tied', 'round_limit: a tie on every tiebreak is a draw', 'short',
  { at('test_soldier', 0, 0), at('test_soldier', 1, 5) },
  steps(round(me, foe), expect.result('round_limit')))
play('limit_next', 'round_limit: a later tiebreak settles a tie on an earlier one',
  'short_figures',
  { at('test_captain', 0, 0), at('test_soldier', 1, 4), at('test_soldier', 1, 5) },
  steps(start, act.activate(0, 'p0.test_captain#1'), act.end_activation(0),
    act.activate(1, foe), act.end_activation(1), expect.result('round_limit', { 1 })))
play('limit_draw', 'round_limit: without a tiebreak the match is drawn', 'short_draw',
  { at('test_soldier', 0, 0), at('test_archer', 1, 5) },
  steps(start, act.activate(0, me), act.end_activation(0),
    act.activate(1, 'p1.test_archer#1'), act.end_activation(1), expect.result('round_limit')))

play('ties', 'the first listed condition wins a tie', 'ties',
  { at('test_soldier', 0, 2), at('test_archer', 1, 5) },
  steps(start, act.activate(0, me), act.end_activation(0),
    act.activate(1, 'p1.test_archer#1'), act.end_activation(1), expect.result('hold_zone', { 0 })))
play('ties_reversed', 'the first listed condition wins a tie, in either order', 'ties_reversed',
  { at('test_soldier', 0, 2), at('test_archer', 1, 5) },
  steps(start, act.activate(0, me), act.end_activation(0),
    act.activate(1, 'p1.test_archer#1'), act.end_activation(1), expect.result('round_limit', { 1 })))

play('scripted', 'a Lua function is a victory condition', 'scripted',
  { at('test_soldier', 0, 0), at('test_soldier', 1, 5) },
  steps(round(me, foe), expect.result('script', { 1 })))

play('command_first', 'a command that ends a round is checked as a command first', 'command_first',
  { at('test_soldier', 0, 0), at('test_soldier', 1, 5) },
  steps(start, act.activate(0, me), act.end_activation(0), act.activate(1, foe), expect.result(),
    act.end_activation(1), expect.result('script', { 0 })))
play('broken', 'a condition that fails is skipped and reported, and play goes on', 'broken',
  { at('test_soldier', 0, 0), at('test_soldier', 1, 5) },
  steps(start, expect.event('victory_condition_failed'), act.activate(0, me),
    act.end_activation(0), act.activate(1, foe), act.end_activation(1),
    expect.result('round_limit')))
