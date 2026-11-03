-- The M2 gate pack: one combat slot, filled by Rust (`toy:combat`) or by the Lua module below.
-- The native `toy:driver` module supplies the `attack` command both rulesets use.
stat { id = 'attack', name = 'Attack', min = 0, max = 9 }
stat { id = 'defense', name = 'Defense', min = 0, max = 9 }
die { id = 'combat', name = 'Combat', faces = { 'hit', 'hit', 'hit', 'block', 'block', 'blank' } }

terrain { id = 'grass', name = 'Grass', standable = true }
terrain { id = 'rock', name = 'Rock', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'boulder', name = 'Boulder', terrain = 'rock', thickness = 1, shape = {{ q = 0, r = 0 }} }

local pieces = {}
for q = 0, 9 do
  for r = 0, 9 do
    local piece = ((q + r) % 5 == 4) and 'boulder' or 'meadow'
    pieces[#pieces + 1] = { piece = piece, at = { q = q, r = r }, rot = 0, level = 0 }
  end
end
map { id = 'flat', name = 'Flat', orientation = 'pointy', pieces = pieces }

local function figure(id)
  return {
    id = id, count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }
end
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = { attack = 3, defense = 2 }, figures = { figure('s') },
}
unit {
  id = 'test_archer', name = 'Test Archer', kind = 'squad', rarity = 'common',
  stats = { attack = 4, defense = 1 }, figures = { figure('a') },
}

-- The Lua twin of `toy:combat`: the same dice, in the same order, and the same wound tally.
rule_module {
  id = 'combat_lua',
  slot = 'combat',
  register = function(r)
    r.trigger {
      event = 'attack_declared',
      effect = function(ctx, event)
        local fields = event.Custom.payload
        local attacker, target = fields.attacker, fields.target
        local hits = ctx.dice.roll('combat', ctx.query.stat(attacker, 'attack')):count('hit')
        local blocks = ctx.dice.roll('combat', ctx.query.stat(target, 'defense')):count('block')
        local tally = ctx.vars('toy:combat')
        local name = 'wounds_' .. target
        tally[name] = (tally[name] or 0) + math.max(0, hits - blocks)
      end,
    }
  end,
}

-- The rulesets differ only in the combat slot.
ruleset {
  id = 'toy_rust', name = 'Toy (Rust combat)',
  slots = { combat = { id = 'toy:combat' } }, addons = {{ id = 'toy:driver' }},
}
ruleset {
  id = 'toy_lua', name = 'Toy (Lua combat)',
  slots = { combat = { id = 'toy:combat_lua' } }, addons = {{ id = 'toy:driver' }},
}

-- The same tests run against both rulesets.
local function versus()
  return {
    { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_archer', player = 1, at = {{ q = 1, r = 0, z = 1 }} },
  }
end
local function attack(attacker, target)
  return act.command(0, 'attack', { attacker = attacker, target = target })
end

for _, ruleset in ipairs { 'toy_rust', 'toy_lua' } do
  test {
    id = 'stats_' .. ruleset, name = ruleset .. ': units carry their stats',
    ruleset = ruleset, map = 'flat', place = versus(),
    steps = {
      expect.stat('p0.test_soldier#1', 'attack', 3),
      expect.stat('p1.test_archer#1', 'defense', 1),
    },
  }
  test {
    id = 'soldier_' .. ruleset, name = ruleset .. ': the soldier attacks with its attack stat',
    ruleset = ruleset, map = 'flat', place = versus(),
    steps = { attack(0, 1), expect.event('dice_rolled'), expect.event('var_set') },
  }
  test {
    id = 'archer_' .. ruleset, name = ruleset .. ': the archer attacks with its attack stat',
    ruleset = ruleset, map = 'flat', place = versus(),
    steps = { attack(1, 0), expect.event('dice_rolled'), expect.event('var_set') },
  }
  test {
    id = 'repeat_' .. ruleset, name = ruleset .. ': wounds accumulate over two attacks',
    ruleset = ruleset, map = 'flat', place = versus(),
    steps = { attack(0, 1), attack(0, 1), expect.event('var_set') },
  }
  test {
    id = 'rigged_' .. ruleset, name = ruleset .. ': rigged dice decide the wounds',
    ruleset = ruleset, map = 'flat', place = versus(),
    steps = {
      act.command(0, 'attack', { attacker = 0, target = 1 }, { rig = {{ 'hit', 'hit', 'hit' }, { 'block' }} }),
      expect.event('var_set'),
    },
  }
end
