-- A duel whose ruleset adds 1 to every figure's defense, so cards show a buffed stat.
stat { id = 'attack', name = 'Attack', abbrev = 'ATK', order = 1, min = 0, max = 9 }
stat { id = 'defense', name = 'Defense', abbrev = 'DEF', order = 2, min = 0, max = 9 }
marker { id = 'wound', name = 'Wound', stacking = 'count', visibility = 'public' }
die { id = 'combat', name = 'Combat', faces = { 'hit', 'blank' } }

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
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = { attack = 3, defense = 2 },
  abilities = {{ name = 'Shieldwall', text = 'Adds 1 to defense.', scope = 'unit' }},
  figures = {{
    id = 's', count = 2, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}

rule_module {
  id = 'aura',
  register = function(r)
    r.modifier { query = 'defense', layer = 'add', value = function() return 1 end }
  end,
}
ruleset { id = 'rs', name = 'Cards', addons = {{ id = 'cards:aura' }} }
