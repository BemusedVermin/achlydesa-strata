-- Pack tests for `core:command_tokens`. The Rust test supplies the add-on `test:pass`, whose
-- `pass` command ends the active player's activation, which ends a turn. The native
-- `toy:driver` supplies `attack`, which the `harness` module turns into `ctx.units.destroy`.
-- The initiative die has three faces, so a roll's result is its face number: a=1, b=2, c=3.
-- Token kinds: 1 and 2 are the numbered tokens, 0 is the decoy.
die { id = 'init', name = 'Initiative', faces = { 'a', 'b', 'c' } }
stat { id = 'life', name = 'Life', min = 1, max = 9 }

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = { life = 1 }, figures = {{
    id = 's', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}

rule_module {
  id = 'harness',
  register = function(r)
    r.trigger {
      event = 'attack_declared',
      effect = function(ctx, event)
        local p = event.Custom.payload
        if p.op == 'destroy' then ctx.units.destroy(p.figure) end
      end,
    }
  end,
}

local function turns(per_round)
  return { id = 'core:rounds', config = {
    initiative = { Str = 'init' }, tie = { Str = 'reroll' }, turns_per_round = { Int = per_round },
  } }
end
local function tokens(numbered, decoys)
  return { id = 'core:command_tokens', config = { numbered = { Int = numbered }, decoys = { Int = decoys } } }
end
local addons = {{ id = 'test:pass' }, { id = 'toy:driver' }, { id = 'tokens:harness' }}
ruleset {
  id = 'rs', name = 'Tokens',
  slots = { turns = turns(2), tokens = tokens(2, 1), units = { id = 'core:units' } },
  addons = addons,
}
-- With the tokens slot empty, players choose units freely.
ruleset {
  id = 'rs_free', name = 'Free choice',
  slots = { turns = turns(2), units = { id = 'core:units' } },
  addons = addons,
}

local function board()
  local army = {}
  for player = 0, 1 do
    for q = 0, 2 do
      army[#army + 1] = { unit = 'test_soldier', player = player, at = {{ q = q, r = player, z = 1 }} }
    end
  end
  return army
end
local function start(rig) return act.command(0, 'start_round', nil, { rig = rig }) end
local function pass(player) return act.command(player, 'pass', nil) end
-- Each player puts token `a` on their first soldier, `b` on their second, and `c` on their third.
local function place(player, a, b, c)
  local who = 'p' .. player .. '.test_soldier#'
  return act.place_tokens(player, {
    { figure = who .. '1', kind = a }, { figure = who .. '2', kind = b }, { figure = who .. '3', kind = c },
  })
end
local first = { { 'c' }, { 'a' } }

test {
  id = 'turn_order', name = 'tokens are revealed in order and decide who acts',
  ruleset = 'rs', place = board(),
  steps = {
    start(first), place(0, 2, 1, 0), place(1, 1, 2, 0),
    expect.event('token_revealed'),
    pass(0), pass(1), pass(0), pass(1),
    expect.event('round_ended'),
  },
}
test {
  id = 'destroyed_holder', name = 'a destroyed unit loses its token and its turn',
  ruleset = 'rs', place = board(),
  steps = {
    start(first), place(0, 2, 1, 0), place(1, 1, 2, 0),
    pass(0),
    -- Player 0's first soldier holds their second token, which is not yet revealed.
    act.command(0, 'attack', { op = 'destroy', figure = 0 }),
    expect.event('tokens_discarded'),
    pass(1),
    expect.event('turn_skipped'),
    pass(1),
    expect.event('round_ended'),
  },
}
test {
  id = 'place_twice', name = 'a player cannot place their tokens twice',
  ruleset = 'rs', place = board(),
  steps = { start(first), place(0, 2, 1, 0), place(0, 2, 1, 0) },
}
test {
  id = 'place_on_enemy', name = 'a token cannot go on an enemy unit',
  ruleset = 'rs', place = board(),
  steps = {
    start(first),
    act.place_tokens(0, {
      { figure = 'p1.test_soldier#1', kind = 1 }, { figure = 'p0.test_soldier#2', kind = 2 },
      { figure = 'p0.test_soldier#3', kind = 0 },
    }),
  },
}
test {
  id = 'place_before_phase', name = 'tokens cannot be placed outside the placement phase',
  ruleset = 'rs', place = board(),
  steps = { place(0, 2, 1, 0) },
}
test {
  id = 'free_choice', name = 'an empty tokens slot skips placement',
  ruleset = 'rs_free', place = board(),
  steps = { start(first), expect.event('turn_started'), pass(0), pass(1), pass(0), pass(1), expect.event('round_ended') },
}
