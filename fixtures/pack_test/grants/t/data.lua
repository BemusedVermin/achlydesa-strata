-- Needs the native `t:grants` module the Rust tests register: `attach` and `detach` take
-- { what = 'sharp' | 'fury' | 'drill', owner = <player or unit> } and emit the matching ability
-- events. `drill` is the soldier's own first ability, attached to a unit.
stat { id = 'attack', name = 'Attack', min = 0, max = 9 }
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = { attack = 3 },
  figures = {{
    id = 's', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
  abilities = {{
    name = 'Drill', text = 'Add 2 attack.', scope = 'unit',
    modifiers = {{ query = 'attack', layer = 'add', value = 2 }},
  }},
}
ability {
  id = 'sharp', name = 'Sharp', text = 'Add 1 attack.', scope = 'army',
  modifiers = {{ query = 'attack', layer = 'add', value = function() return 1 end }},
}
relic {
  id = 'fury', name = 'Relic of Fury', mode = 'while_occupied', claim = 'ends_move_on',
  grants = { to = 'occupant_army', abilities = {{
    name = 'Fury', text = 'Add 2 attack.', scope = 'holder',
    modifiers = {{ query = 'attack', layer = 'add', value = function() return 2 end }},
  }} },
}
ruleset { id = 'rs', name = 'Grants', addons = {{ id = 't:grants' }} }

local function board()
  return {
    { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 0, r = 2, z = 1 }} },
  }
end

test {
  id = 'ability_on_off', name = 'an attached ability changes a stat until it is detached',
  ruleset = 'rs', place = board(),
  steps = {
    expect.stat('p0.test_soldier#1', 'attack', 3),
    act.command(0, 'attach', { what = 'sharp', owner = 0 }),
    expect.event('ability_attached'),
    expect.stat('p0.test_soldier#1', 'attack', 4),
    -- The other army is untouched.
    expect.stat('p1.test_soldier#1', 'attack', 3),
    act.command(0, 'detach', { what = 'sharp', owner = 0 }),
    expect.event('ability_detached'),
    expect.stat('p0.test_soldier#1', 'attack', 3),
  },
}
test {
  id = 'relic_grant', name = 'a relic grant applies to the army it is attached to',
  ruleset = 'rs', place = board(),
  steps = {
    act.command(0, 'attach', { what = 'fury', owner = 1 }),
    expect.stat('p1.test_soldier#1', 'attack', 5),
    expect.stat('p0.test_soldier#1', 'attack', 3),
    act.command(0, 'detach', { what = 'fury', owner = 1 }),
    expect.stat('p1.test_soldier#1', 'attack', 3),
  },
}
test {
  id = 'unit_ability', name = 'a unit ability attached to its unit applies to that unit only',
  ruleset = 'rs', place = board(),
  steps = {
    act.command(0, 'attach', { what = 'drill', owner = 0 }),
    expect.stat('p0.test_soldier#1', 'attack', 5),
    expect.stat('p1.test_soldier#1', 'attack', 3),
    act.command(0, 'detach', { what = 'drill', owner = 0 }),
    expect.stat('p0.test_soldier#1', 'attack', 3),
  },
}
test {
  id = 'detach_nothing', name = 'detaching an ability that is not attached is refused',
  ruleset = 'rs', place = board(),
  steps = { act.command(0, 'detach', { what = 'sharp', owner = 0 }) },
}
