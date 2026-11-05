-- A duel whose Lua ability asks the attacking player to pick a figure: the attacker or its
-- target. The client must highlight exactly those two.
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
local pieces = {}
for q = 0, 3 do
  pieces[#pieces + 1] = { piece = 'meadow', at = { q = q, r = 0 }, rot = 0, level = 0 }
end
map {
  id = 'strip', name = 'Strip', orientation = 'pointy', pieces = pieces,
  zones = { west = {{ q = 0, r = 0 }}, east = {{ q = 3, r = 0 }} },
}
scenario {
  id = 'duel', name = 'Duel', map = 'strip', players = { min = 2, max = 2 },
  slots = {
    { id = 'a', start = 'west', deploy_order = 1 },
    { id = 'b', start = 'east', deploy_order = 2 },
  },
}

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
local function figure(id)
  return {
    id = id, count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }
end
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = {}, figures = { figure('s') },
}
unit {
  id = 'test_archer', name = 'Test Archer', kind = 'squad', rarity = 'common',
  stats = {}, figures = { figure('a') },
}

rule_module {
  id = 'rally',
  register = function(r)
    r.trigger {
      event = 'attack_declared',
      effect = function(ctx, event)
        local fields = event.Custom.payload
        local picked = ctx.choose {
          chooser = 0, prompt = 'rally', figures = { fields.attacker, fields.target },
        }
        ctx.emit('rallied', picked)
      end,
    }
  end,
}
ruleset { id = 'rs', name = 'Decide', addons = {{ id = 'toy:driver' }, { id = 'decide:rally' }} }
