-- A second pack with its own `test_archer` and `iron`. Bare names in its formats mean its own
-- definitions first, so they never reach the `army` pack's archer or faction.
faction { id = 'iron', name = 'Iron', color = '#333333' }
unit {
  id = 'test_archer', name = 'test_archer', kind = 'squad', rarity = 'common', faction = 'iron',
  stats = { points = 10 },
  figures = {{
    id = 'f', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}

local function format_of(id, army)
  format {
    id = id, name = id, ruleset = 'rs', selection = 'build',
    scenarios = { pool = { 'plain' }, pick = 'alternate_veto' }, army = army,
  }
end
format_of('no_archers', { army_rule.banned 'test_archer' })
format_of('iron_only', { army_rule.require_faction 'iron' })
