-- A real match from setup to a unit's activation, for the client's command input. Player 0
-- deploys one soldier with `place_unit`, starts the round, places a token and activates the
-- soldier, which may then `walk`. Player 1 has no army, so their turns pass.
die { id = 'init', name = 'Initiative', faces = { 'a', 'b', 'c' } }
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
local pieces = {}
for q = 0, 3 do
  pieces[#pieces + 1] = { piece = 'meadow', at = { q = q, r = 0 }, rot = 0, level = 0 }
end
map {
  id = 'strip', name = 'Strip', orientation = 'pointy', pieces = pieces,
  zones = { west = {{ q = 0, r = 0 }, { q = 1, r = 0 }}, east = {{ q = 2, r = 0 }, { q = 3, r = 0 }} },
}
scenario {
  id = 'duel', name = 'Duel', map = 'strip', players = { min = 2, max = 2 },
  slots = {
    { id = 'a', start = 'west', deploy_order = 1 },
    { id = 'b', start = 'east', deploy_order = 2 },
  },
}

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = {}, figures = {{
    id = 's', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}

rule_module {
  id = 'rules',
  register = function(r)
    r.action { id = 'walk', slots = { 'move' }, resolve = function(ctx, figure) ctx.emit('walked', figure) end }
  end,
}

ruleset {
  id = 'rs', name = 'Command',
  slots = {
    deployment = { id = 'core:deployment' },
    turns = { id = 'core:rounds', config = {
      initiative = { Str = 'init' }, tie = { Str = 'reroll' }, turns_per_round = { Int = 1 },
    } },
    tokens = { id = 'core:command_tokens', config = { numbered = { Int = 1 }, decoys = { Int = 0 } } },
    activation = { id = 'core:activation' },
    units = { id = 'core:units' },
  },
  addons = {{ id = 'command:rules' }},
}
