-- Needs the native `t:grants` module the Rust tests register: `attach` and `detach` take
-- { what = 'sharp' | 'fury' | 'drill', owner = <player or unit> } and emit the matching ability
-- events. `drill` is the veteran's printed first ability, attached to a unit. `probe` takes
-- { key, subject, context } and emits a `probe` event, which `t:probe` answers with the vote.
stat { id = 'attack', name = 'Attack', min = 0, max = 9 }
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
local function figure(id)
  return {
    id = id, count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }
end
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = { attack = 3 }, figures = { figure('s') },
}
-- A printed stat bonus: setup attaches it, so nothing else has to.
unit {
  id = 'test_veteran', name = 'Test Veteran', kind = 'squad', rarity = 'common',
  stats = { attack = 3 }, figures = { figure('v') },
  abilities = {{
    name = 'Drill', text = 'Add 2 attack.', scope = 'unit',
    modifiers = {{ query = 'attack', layer = 'add', value = 2 }},
  }},
}
-- Votes on questions asked about other figures. `guarded` has a selector that looks at the
-- context, so it also joins pair queries whose context is the guard; `plain` has none, so it
-- only answers questions about the guard itself.
unit {
  id = 'test_guard', name = 'Test Guard', kind = 'squad', rarity = 'common',
  stats = { attack = 3 }, figures = { figure('g') },
  abilities = {{
    name = 'Guard', text = 'Guards whoever is asked about it.', scope = 'unit',
    modifiers = {
      {
        query = 'guarded', layer = 'set', value = function() return true end,
        select = function(source, subject, context) return type(context) == 'number' end,
      },
      { query = 'plain', layer = 'set', value = function() return true end },
    },
  }},
}
rule_module {
  id = 'probe',
  register = function(r)
    r.trigger {
      event = 'probe',
      effect = function(ctx, event)
        local p = event.Custom.payload
        if ctx.query.vote(p.key, false, p.subject, p.context) then
          ctx.emit('allowed', 1)
        else
          ctx.emit('denied', 1)
        end
      end,
    }
  end,
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
ruleset { id = 'rs', name = 'Grants', addons = {{ id = 't:grants' }, { id = 't:probe' }} }

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
  id = 'printed_ability', name = 'a printed unit ability applies with no manual attach',
  ruleset = 'rs',
  place = {
    { unit = 'test_veteran', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 0, r = 2, z = 1 }} },
    -- In reserve: printed abilities belong to the unit, not to deployed figures.
    { unit = 'test_veteran', player = 1, at = {} },
  },
  steps = {
    expect.stat('p0.test_veteran#1', 'attack', 5),
    expect.stat('p1.test_soldier#1', 'attack', 3),
    expect.stat('p1.test_veteran#1', 'attack', 5),
    -- It is an ordinary attached instance, keyed by the unit and its position.
    act.command(0, 'detach', { what = 'drill', owner = 0 }),
    expect.stat('p0.test_veteran#1', 'attack', 3),
    expect.stat('p1.test_veteran#1', 'attack', 5),
    act.command(0, 'attach', { what = 'drill', owner = 0 }),
    expect.stat('p0.test_veteran#1', 'attack', 5),
  },
}
test {
  id = 'context_ability', name = "a pair query gathers the context figure's selected modifiers",
  ruleset = 'rs',
  place = {
    { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_guard', player = 1, at = {{ q = 0, r = 2, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 0, r = 4, z = 1 }} },
  },
  steps = {
    -- Asked about soldier 0 with the guard as context: the guard's selector applies.
    act.command(0, 'probe', { key = 'guarded', subject = 0, context = 1 }),
    expect.event('allowed'),
    -- Another figure of the guard's army has no such ability.
    act.command(0, 'probe', { key = 'guarded', subject = 0, context = 2 }),
    expect.event('denied'),
    -- A modifier with no selector does not reach across.
    act.command(0, 'probe', { key = 'plain', subject = 0, context = 1 }),
    expect.event('denied'),
    -- It still answers for the guard itself.
    act.command(0, 'probe', { key = 'plain', subject = 1, context = 0 }),
    expect.event('allowed'),
  },
}
test {
  id = 'detach_nothing', name = 'detaching an ability that is not attached is refused',
  ruleset = 'rs', place = board(),
  steps = { act.command(0, 'detach', { what = 'sharp', owner = 0 }) },
}
