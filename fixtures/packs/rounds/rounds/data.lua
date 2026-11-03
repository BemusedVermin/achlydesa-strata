-- Pack tests for `core:rounds`. The Rust test supplies the add-on `test:pass`, whose `pass`
-- command ends the active player's activation, which is what ends a turn.
-- The initiative die has three faces, so a roll's result is its face number: a=1, b=2, c=3.
die { id = 'init', name = 'Initiative', faces = { 'a', 'b', 'c' } }

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = {}, figures = {{
    id = 's', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}

local function rules(id, tie, turns)
  ruleset {
    id = id, name = id,
    slots = { turns = { id = 'core:rounds', config = { initiative = { Str = 'init' }, tie = { Str = tie }, turns_per_round = { Int = turns } } } },
    addons = {{ id = 'test:pass' }},
  }
end
rules('rs_reroll', 'reroll', 1)
rules('rs_keep', 'keep', 1)
rules('rs_long', 'reroll', 2)

local function board()
  return {
    { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 1, r = 0, z = 1 }} },
  }
end
local function start(rig) return act.command(0, 'start_round', nil, { rig = rig }) end
local function pass(player) return act.command(player, 'pass', nil) end

test {
  id = 'high_roll_first', name = 'the higher initiative roll goes first',
  ruleset = 'rs_reroll', place = board(),
  steps = { start({{ 'c' }, { 'a' }}), expect.event('round_started'), expect.event('turn_started') },
}
test {
  id = 'low_roll_second', name = 'the lower initiative roll goes second',
  ruleset = 'rs_reroll', place = board(),
  steps = { start({{ 'a' }, { 'c' }}), expect.event('turn_started') },
}
test {
  id = 'tie_rerolls', name = 'a tie is rerolled by the tied players',
  ruleset = 'rs_reroll', place = board(),
  steps = { start({{ 'b' }, { 'b' }, { 'a' }, { 'c' }}), expect.event('turn_started') },
}
test {
  id = 'tie_keeps', name = 'a kept tie favours the previous first player',
  ruleset = 'rs_keep', place = board(),
  steps = {
    start({{ 'a' }, { 'c' }}), pass(1), pass(0), expect.event('round_ended'),
    start({{ 'b' }, { 'b' }}), expect.event('turn_started'),
  },
}
test {
  id = 'turns_alternate', name = 'players alternate turns until the round ends',
  ruleset = 'rs_long', place = board(),
  steps = {
    start({{ 'c' }, { 'a' }}), pass(0), pass(1), pass(0), pass(1),
    expect.event('round_ended'),
  },
}
test {
  id = 'round_in_progress', name = 'a second round cannot start mid-round',
  ruleset = 'rs_reroll', place = board(),
  steps = { start({{ 'c' }, { 'a' }}), start({{ 'c' }, { 'a' }}) },
}
test {
  id = 'wrong_player', name = 'a player cannot pass out of turn',
  ruleset = 'rs_reroll', place = board(),
  steps = { start({{ 'c' }, { 'a' }}), pass(1) },
}
