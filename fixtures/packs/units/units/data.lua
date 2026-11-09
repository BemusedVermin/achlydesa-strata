-- Pack tests for `core:units` and the dice API. The native `toy:driver` supplies the `attack`
-- command, and the `harness` module below turns its payload into `ctx.units` and `ctx.dice` calls.
stat { id = 'life', name = 'Life', min = 1, max = 9 }
stat { id = 'attack', name = 'Attack', min = 0, max = 9 }
marker { id = 'wound', name = 'Wound', stacking = 'count', visibility = 'public' }
die { id = 'combat', name = 'Combat', faces = { 'hit', 'hit', 'hit', 'block', 'block', 'blank' } }

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
local function figure(id, count)
  return {
    id = id, count = count, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }
end
unit {
  id = 'test_hero', name = 'Test Hero', kind = 'hero', rarity = 'rare',
  stats = { life = 3, attack = 2 }, figures = { figure('h', 1) },
}
unit {
  id = 'test_squad', name = 'Test Squad', kind = 'squad', rarity = 'common',
  stats = { life = 1, attack = 2 }, figures = { figure('s', 2) },
}

rule_module {
  id = 'harness',
  register = function(r)
    r.trigger {
      event = 'attack_declared',
      effect = function(ctx, event)
        local p = event.Custom.payload
        if p.op == 'wound' then
          ctx.units.wound(p.figure, p.amount)
        elseif p.op == 'heal' then
          ctx.units.heal(p.figure, p.amount)
        elseif p.op == 'destroy' then
          ctx.units.destroy(p.figure)
        elseif p.op == 'remove' then
          ctx.units.remove(p.figure)
        elseif p.op == 'reroll' then
          local roll = ctx.dice.pool('combat', p.figure, 'attack'):reroll(0, 1)
          ctx.vars('units:harness').hits = roll:count('hit')
        end
      end,
    }
  end,
}
ruleset {
  id = 'rs', name = 'Units',
  slots = { units = { id = 'core:units' } },
  addons = {{ id = 'toy:driver' }, { id = 'units:harness' }},
}

local function board()
  return {
    { unit = 'test_hero', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_squad', player = 1, at = {{ q = 1, r = 0, z = 1 }, { q = 2, r = 0, z = 1 }} },
  }
end
-- Figure 0 is the hero; figures 1 and 2 are the squad's.
local function op(name, figure, amount, opts)
  return act.command(0, 'attack', { op = name, figure = figure, amount = amount }, opts)
end

test {
  id = 'wound_below_life', name = 'wounds below life leave the figure standing',
  ruleset = 'rs', place = board(),
  steps = { op('wound', 0, 2), expect.event('wounds_applied'), expect.figure('p0.test_hero#1', 'on_board') },
}
test {
  id = 'wound_to_life', name = 'wounds at life destroy the figure',
  ruleset = 'rs', place = board(),
  steps = {
    op('wound', 0, 2), op('wound', 0, 1),
    expect.event('figure_destroyed', { figure = 'p0.test_hero#1' }),
    expect.figure('p0.test_hero#1', 'destroyed'),
  },
}
test {
  id = 'wound_past_life', name = 'wounds above life destroy the figure',
  ruleset = 'rs', place = board(),
  steps = { op('wound', 0, 5), expect.figure('p0.test_hero#1', 'destroyed') },
}
test {
  id = 'wound_destroyed', name = 'a destroyed figure cannot be wounded again',
  ruleset = 'rs', place = board(),
  steps = { op('wound', 0, 3), op('wound', 0, 1) },
}
test {
  id = 'heal', name = 'healing takes wounds back off',
  ruleset = 'rs', place = board(),
  steps = {
    op('wound', 0, 2), op('heal', 0, 1), op('wound', 0, 1),
    expect.figure('p0.test_hero#1', 'on_board'),
    op('wound', 0, 1), expect.figure('p0.test_hero#1', 'destroyed'),
  },
}
test {
  id = 'squad_removed', name = 'a squad is removed with its last figure',
  ruleset = 'rs', place = board(),
  steps = {
    op('wound', 1, 1), expect.figure('p1.test_squad#1', 'destroyed'),
    op('destroy', 2), expect.event('unit_removed'),
  },
}
test {
  id = 'remove', name = 'a removed figure leaves play without being destroyed',
  ruleset = 'rs', place = board(),
  steps = {
    op('remove', 1), expect.event('figure_removed', { figure = 'p1.test_squad#1' }),
    expect.figure('p1.test_squad#1', 'removed'),
    op('remove', 2), expect.event('unit_removed'), expect.figure('p1.test_squad#2', 'removed'),
  },
}
test {
  id = 'reroll', name = 'a reroll is a decision then a replacement roll',
  ruleset = 'rs', place = board(),
  steps = {
    op('reroll', 0, nil, { rig = {{ 'blank', 'blank' }, { 'hit' }} }),
    expect.decision('reroll'),
    act.answer.yes_no(true),
    expect.event('var_set'),
  },
}
test {
  id = 'reroll_declined', name = 'declining a reroll keeps the first roll',
  ruleset = 'rs', place = board(),
  steps = {
    op('reroll', 0, nil, { rig = {{ 'hit', 'blank' }} }),
    expect.decision('reroll'),
    act.answer.yes_no(false),
    expect.event('var_set'),
  },
}
