-- Pack tests for `core:army`. `expect.army` checks a list against a format's and a scenario's
-- army rules and lists the rule behind each violation: the format's first, then the scenario's.
-- Points: test_soldier 30, test_hero 100, test_archer 50. Footprint: test_soldier covers two
-- spaces (two one-space figures), test_hero two (one two-space figure), test_archer one.
stat { id = 'points', name = 'Points', min = 0, max = 999 }
faction { id = 'brass', name = 'Brass', color = '#B5893A' }
faction { id = 'iron', name = 'Iron', color = '#555555' }
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
footprint { id = 'two', name = 'Two', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }}, move = 'rigid', max_spread = 1 }

local function unit_of(id, rarity, faction, points, footprint, count)
  unit {
    id = id, name = id, kind = 'squad', rarity = rarity, faction = faction,
    stats = { points = points },
    figures = {{
      id = 'f', count = count, footprint = footprint, height = 1,
      los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
    }},
  }
end
unit_of('test_soldier', 'common', 'brass', 30, 'one', 2)
unit_of('test_hero', 'unique', 'brass', 100, 'two', 1)
unit_of('test_archer', 'common', 'iron', 50, 'one', 1)

terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
map {
  id = 'spot', name = 'Spot', orientation = 'pointy',
  pieces = {{ piece = 'meadow', at = { q = 0, r = 0 }, rot = 0, level = 0 }},
  zones = { a = {{ q = 0, r = 0 }} },
}
local function scenario_of(id, army)
  scenario {
    id = id, name = id, map = 'spot', players = { min = 2, max = 2 },
    slots = {{ id = 'a', start = 'a', deploy_order = 1 }, { id = 'b', start = 'a', deploy_order = 2 }},
    army = army,
  }
end
scenario_of('plain', {})
-- A scenario tightens the format: it bans archers, wants Brass and caps the footprint.
scenario_of('strict', {
  army_rule.banned { 'test_archer' },
  army_rule.require_faction('brass'),
  army_rule.max_footprint(6),
})
-- A mod's own constraint, as a Lua function of the army.
scenario_of('scripted', {
  function(army)
    if #army > 2 then return 'at most two entries' end
  end,
})
-- A Lua rule may answer `true` to pass and `false` to fail with a generic message.
scenario_of('yes_or_no', {
  function(army) return #army <= 2 end,
})

format {
  id = 'duel_200', name = 'Duel, 200 points', ruleset = 'rs', selection = 'build',
  scenarios = { pool = { 'plain', 'strict' }, pick = 'alternate_veto' },
  army = { army_rule.max_points(200), army_rule.unique_once(), army_rule.max_copies(3) },
}
-- A typo in a ban names nothing, which is an error rather than a ban that never applies.
format {
  id = 'typo', name = 'Typo', ruleset = 'rs', selection = 'build',
  scenarios = { pool = { 'plain' }, pick = 'alternate_veto' },
  army = { army_rule.banned { 'test_archr' } },
}
format {
  id = 'open', name = 'Open', ruleset = 'rs', selection = 'build',
  scenarios = { pool = { 'plain' }, pick = 'alternate_veto' }, army = {},
}

local function list_of(id, units)
  local entries = {}
  for i, pick in ipairs(units) do entries[i] = { unit = pick[1], copies = pick[2] } end
  army { id = id, name = id, format = 'duel_200', packs = { army = '1.0.0' }, units = entries }
end
list_of('legal', {{ 'test_soldier', 2 }, { 'test_hero', 1 }})
list_of('expensive', {{ 'test_hero', 1 }, { 'test_soldier', 4 }})
list_of('two_heroes', {{ 'test_hero', 1 }, { 'test_hero', 1 }})
list_of('mixed', {{ 'test_archer', 1 }, { 'test_hero', 1 }, { 'test_soldier', 2 }})

ruleset { id = 'rs', name = 'Army', slots = { army = { id = 'core:army' } } }
ruleset { id = 'bare', name = 'Without army rules', slots = {} }

local function check(id, name, list, opts)
  test { id = id, name = name, ruleset = 'rs', steps = { expect.army(list, opts) } }
end
check('legal', 'a list within every rule is legal', 'legal',
  { format = 'duel_200', scenario = 'strict' })
check('max_points', 'max_points and max_copies catch an expensive list', 'expensive',
  { format = 'duel_200', violations = { 'max_points', 'max_copies' } })
check('unique_once', 'a unique unit is taken once, across entries', 'two_heroes',
  { format = 'duel_200', violations = { 'unique_once' } })
check('both', "a scenario's army rules and its format's rules both apply", 'mixed', {
  format = 'duel_200', scenario = 'strict',
  violations = { 'max_points', 'banned', 'require_faction', 'max_footprint' },
})
check('scenario_only', 'a scenario alone applies its own rules', 'mixed',
  { scenario = 'strict', violations = { 'banned', 'require_faction', 'max_footprint' } })
check('open', 'a format without rules allows any list', 'mixed', { format = 'open', scenario = 'plain' })
check('script', 'a Lua function is an army rule', 'mixed',
  { format = 'open', scenario = 'scripted', violations = { 'script' } })
check('script_passes', 'a Lua army rule that returns nil passes', 'legal',
  { format = 'open', scenario = 'scripted' })
check('script_false', 'a Lua army rule that returns false fails', 'mixed',
  { format = 'open', scenario = 'yes_or_no', violations = { 'script' } })
check('script_true', 'a Lua army rule that returns true passes', 'legal',
  { format = 'open', scenario = 'yes_or_no' })
check('other_pack_ban', "a bare name in another pack's rule means that pack's unit", 'mixed',
  { format = 'rival:no_archers' })
check('other_pack_faction', "a bare faction in another pack's rule means that pack's faction",
  'mixed', {
    format = 'rival:iron_only',
    violations = { 'require_faction', 'require_faction', 'require_faction' },
  })
