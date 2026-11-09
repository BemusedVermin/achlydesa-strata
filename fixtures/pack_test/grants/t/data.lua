-- Needs the native `t:grants` module the Rust tests register: `attach` and `detach` take
-- { what = 'sharp' | 'fury' | 'veil' | 'drill', owner = <player or unit> } and emit the matching
-- ability events. `fury` and `veil` are relic grants, attached to a player; `drill` is the
-- veteran's printed first ability, attached to a unit. `probe` takes { key, subject, context,
-- default } and emits a `probe` event, which `t:probe` answers with the vote; `default` is
-- false unless given.
stat { id = 'attack', name = 'Attack', min = 0, max = 9 }
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
local function figure(id, count)
  return {
    id = id, count = count or 1, footprint = 'one', height = 1,
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
-- Votes on questions asked about other figures. `guarded` is `on = 'context'`, so it joins pair
-- queries whose context is the guard, and `t:probe` declares `guarded` a pair query. `plain` is
-- on the subject, so it only answers questions about the guard itself. `counted` is on the
-- context too, but no module declares it a pair query, so its number context is not a figure.
unit {
  id = 'test_guard', name = 'Test Guard', kind = 'squad', rarity = 'common',
  stats = { attack = 3 }, figures = { figure('g') },
  abilities = {{
    name = 'Guard', text = 'Guards whoever is asked about it.', scope = 'unit',
    modifiers = {
      {
        query = 'guarded', layer = 'set', value = function() return true end, on = 'context',
        select = function(source, subject, context) return type(context) == 'number' end,
      },
      { query = 'plain', layer = 'set', value = function() return true end },
      { query = 'counted', layer = 'set', value = function() return true end, on = 'context' },
    },
  }},
}
-- "Enemies cannot target me": a `can_target` veto on the context side. It must not stop a
-- slippery figure from targeting anyone. Two figures, for pairs inside one unit.
unit {
  id = 'test_slippery', name = 'Test Slippery', kind = 'squad', rarity = 'common',
  stats = { attack = 3 }, figures = { figure('e', 2) },
  abilities = {{
    name = 'Slippery', text = 'Cannot be targeted.', scope = 'figure',
    modifiers = {{
      query = 'can_target', layer = 'set', value = function() return false end, on = 'context',
      select = function(_, _, c) return type(c) == 'number' end,
    }},
  }},
}
rule_module {
  id = 'probe',
  register = function(r)
    r.pair_query('guarded')
    r.trigger {
      event = 'probe',
      effect = function(ctx, event)
        local p = event.Custom.payload
        if ctx.query.vote(p.key, p.default == true, p.subject, p.context) then
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
-- A player-owned veto on the context side: no figure of the holder's army can be targeted.
relic {
  id = 'veil', name = 'Relic of Veils', mode = 'while_occupied', claim = 'ends_move_on',
  grants = { to = 'occupant_army', abilities = {{
    name = 'Veil', text = 'Cannot be targeted.', scope = 'holder',
    modifiers = {{
      query = 'can_target', layer = 'set', value = function() return false end, on = 'context',
    }},
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
  id = 'context_ability', name = "a pair query gathers the context figure's context-side modifiers",
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
    -- A context-side modifier never answers for the guard itself.
    act.command(0, 'probe', { key = 'guarded', subject = 1, context = 0 }),
    expect.event('denied'),
    -- A vote no module declares a pair query treats its number context as an input.
    act.command(0, 'probe', { key = 'counted', subject = 0, context = 1 }),
    expect.event('denied'),
  },
}
test {
  id = 'slippery', name = 'a context-side veto stops others targeting its owner, not the owner',
  ruleset = 'rs',
  place = {
    { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
    { unit = 'test_slippery', player = 1, at = {{ q = 0, r = 2, z = 1 }, { q = 0, r = 3, z = 1 }} },
  },
  steps = {
    -- The soldier cannot target a slippery figure.
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 0, context = 1 }),
    expect.event('denied'),
    -- A slippery figure can still target the soldier.
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 1, context = 0 }),
    expect.event('allowed'),
    -- Inside one unit, the context figure's veto still applies.
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 1, context = 2 }),
    expect.event('denied'),
    -- A figure as both subject and context gets its context side too.
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 1, context = 1 }),
    expect.event('denied'),
    -- The soldier as both subject and context has no veto at all.
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 0, context = 0 }),
    expect.event('allowed'),
  },
}
test {
  id = 'slippery_mirror', name = 'a context-side veto in mirror armies applies only to its owner',
  ruleset = 'rs',
  place = {
    { unit = 'test_slippery', player = 0, at = {{ q = 0, r = 0, z = 1 }, { q = 0, r = 1, z = 1 }} },
    { unit = 'test_slippery', player = 1, at = {{ q = 0, r = 3, z = 1 }, { q = 0, r = 4, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 0, r = 6, z = 1 }} },
  },
  steps = {
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 0, context = 2 }),
    expect.event('denied'),
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 2, context = 0 }),
    expect.event('denied'),
    -- Either side's slippery figures can target the soldier.
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 0, context = 4 }),
    expect.event('allowed'),
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 3, context = 4 }),
    expect.event('allowed'),
  },
}
test {
  id = 'veil', name = 'a player-owned grant on the context side shields that army',
  ruleset = 'rs', place = board(),
  steps = {
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 0, context = 1 }),
    expect.event('allowed'),
    act.command(0, 'attach', { what = 'veil', owner = 1 }),
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 0, context = 1 }),
    expect.event('denied'),
    -- The veiled army can still target.
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 1, context = 0 }),
    expect.event('allowed'),
    act.command(0, 'detach', { what = 'veil', owner = 1 }),
    act.command(0, 'probe', { key = 'can_target', default = true, subject = 0, context = 1 }),
    expect.event('allowed'),
  },
}
test {
  id = 'detach_nothing', name = 'detaching an ability that is not attached is refused',
  ruleset = 'rs', place = board(),
  steps = { act.command(0, 'detach', { what = 'sharp', owner = 0 }) },
}
