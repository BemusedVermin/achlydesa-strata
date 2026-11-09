-- Pack tests for `core:combat`: the `attack` action, its range and line of sight, the height
-- bonus, cover, a reaction in each of its windows and `ctx.combat.attack`. Figures are numbered
-- in the order the tests place them. Every column stands at level 1 unless noted.
--
-- `survey` emits `surveyed` with `{ count, targets }`, the figures `ctx.combat.targets` lists;
-- `peek` emits `peeked` with the pools `ctx.combat.preview` gives against its argument, or
-- `{ none = true }`. The Rust tests read both. Tests named `refused: ...` must fail at a step the
-- engine rejects; the Rust tests check each one's message.
stat { id = 'life', name = 'Life', min = 1, max = 9 }
stat { id = 'range', name = 'Range', min = 0, max = 9 }
stat { id = 'attack', name = 'Attack', min = 0, max = 9 }
stat { id = 'defense', name = 'Defense', min = 0, max = 9 }
marker { id = 'wound', name = 'Wound', stacking = 'count', visibility = 'public' }
die { id = 'combat', name = 'Combat', faces = { 'hit', 'hit', 'hit', 'block', 'block', 'blank' } }
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'hill', name = 'Hill', terrain = 'grass', thickness = 2, shape = {{ q = 0, r = 0 }} }
feature {
  id = 'wall2', name = 'High wall', blocks_los = true,
  cells = {{ at = { q = 0, r = 0 }, segments = {{ from = 0, to = 2, top = true }} }},
}
feature {
  id = 'wall1', name = 'Low wall', blocks_los = true,
  cells = {{ at = { q = 0, r = 0 }, segments = {{ from = 0, to = 1, top = true }} }},
}

-- Two rows, q = 0..6 on r = 0 and r = 1. `hill` raises (1, 0) to level 2, and `ridge` raises
-- (1, 0) and (1, 1).
local function row(raised, ridge)
  local pieces = {}
  for q = 0, 6 do
    for r = 0, 1 do
      local high = raised and q == 1 and (r == 0 or ridge)
      local piece = high and 'hill' or 'meadow'
      pieces[#pieces + 1] = { piece = piece, at = { q = q, r = r }, rot = 0, level = 0 }
    end
  end
  return pieces
end
map { id = 'field', name = 'Field', orientation = 'pointy', pieces = row() }
map { id = 'hill', name = 'Hill', orientation = 'pointy', pieces = row(true) }
map { id = 'ridge', name = 'Ridge', orientation = 'pointy', pieces = row(true, true) }
map {
  id = 'high_wall', name = 'High wall', orientation = 'pointy', pieces = row(),
  features = {{ feature = 'wall2', at = { q = 2, r = 0 }, rot = 0 }},
}
map {
  id = 'low_wall', name = 'Low wall', orientation = 'pointy', pieces = row(),
  features = {{ feature = 'wall1', at = { q = 3, r = 0 }, rot = 0 }},
}

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
local function unit_of(id, stats, eye)
  unit {
    id = id, name = id, kind = 'squad', rarity = 'common', stats = stats,
    figures = {{
      id = 'f', count = 1, footprint = 'one', height = 1,
      los = { target = eye or 10, hit_zone = {0, 12}, samples = {'center'} },
    }},
  }
end
unit_of('test_soldier', { life = 2, range = 1, attack = 2, defense = 2 })
unit_of('test_archer', { life = 2, range = 3, attack = 2, defense = 1 })
unit_of('test_scout', { life = 2, range = 6, attack = 2, defense = 1 }, 20)

rule_module {
  id = 'tools',
  register = function(r)
    r.action {
      id = 'survey', slots = {},
      resolve = function(ctx, figure)
        local found = ctx.combat.targets(figure)
        ctx.emit('surveyed', { count = #found, targets = found })
      end,
    }
    r.action {
      id = 'peek', slots = {},
      resolve = function(ctx, figure, target)
        ctx.emit('peeked', ctx.combat.preview(figure, target) or { none = true })
      end,
    }
    -- `measured` carries the figure's attack asked with and without `target` as context.
    r.action {
      id = 'measure', slots = {},
      resolve = function(ctx, figure, target)
        local against = ctx.query.stat(figure, 'attack', target)
        ctx.emit('measured', { against = against, alone = ctx.query.stat(figure, 'attack') })
      end,
    }
    -- A special attack: three dice at range 5, whatever the attacker's stats.
    r.action {
      id = 'volley', slots = { 'attack' },
      resolve = function(ctx, figure, target)
        ctx.combat.attack { attacker = figure, target = target, dice = 3, range = 5 }
      end,
    }
    -- A special attack and a preview with a negative pool, which are refused.
    r.action {
      id = 'misfire', slots = { 'attack' },
      resolve = function(ctx, figure, target)
        ctx.combat.attack { attacker = figure, target = target, dice = -3 }
      end,
    }
    r.action {
      id = 'misjudge', slots = {},
      resolve = function(ctx, figure, target)
        ctx.combat.preview(figure, target, { dice = -1 })
      end,
    }
  end,
}

-- One reaction per window, each offered to the target's controller.
local function target_controller(ctx)
  local attack = ctx.combat.current()
  return ctx.state.figures[attack.target + 1].controller
end
local reactions = {
  before_attack_roll = function(ctx) ctx.combat.set { attack_dice = 3 } end,
  before_defense_roll = function(ctx) ctx.combat.set { defense_dice = 0 } end,
  after_defense_roll = function(ctx) ctx.combat.set { blocks = 0 } end,
  after_wounds = function(ctx) ctx.units.wound(ctx.combat.current().attacker, 1) end,
}
for window, effect in pairs(reactions) do
  rule_module {
    id = window,
    register = function(r)
      r.reaction { window = 'combat.' .. window, when = target_controller, effect = effect }
    end,
  }
end

-- Reactions the engine refuses or that end the attack early, each offered to the target's
-- controller: setting a field that no longer matters, attacking during an attack, and the
-- target destroying the attacker before either roll.
local refusals = {
  set_hits_early = { 'before_attack_roll', function(ctx) ctx.combat.set { hits = 3 } end },
  set_late = { 'after_wounds', function(ctx) ctx.combat.set { blocks = 0 } end },
  counter = { 'before_attack_roll', function(ctx)
    local attack = ctx.combat.current()
    ctx.combat.attack { attacker = attack.target, target = attack.attacker }
  end },
  ambush_attack = { 'before_attack_roll', function(ctx)
    ctx.units.wound(ctx.combat.current().attacker, 2)
  end },
  ambush_defense = { 'before_defense_roll', function(ctx)
    ctx.units.wound(ctx.combat.current().attacker, 2)
  end },
}
for id, reaction in pairs(refusals) do
  rule_module {
    id = id,
    register = function(r)
      r.reaction { window = 'combat.' .. reaction[1], when = target_controller, effect = reaction[2] }
    end,
  }
end

-- Figure 0 may target nobody.
rule_module {
  id = 'pacifist',
  register = function(r)
    r.modifier {
      query = 'can_target', layer = 'set', value = function() return false end,
      when = function(_, subject) return subject == 0 end,
    }
  end,
}

-- Modifiers that read the opponent from the query's context: the attacker's `attack` and
-- `range` are asked with the target as context, the target's `defense` with the attacker. A
-- query script sees no state, so the tests that use these name the opponent by figure ID: in
-- `high_ground`, figure 1 is the only enemy standing lower than figure 0.
rule_module {
  id = 'high_ground',
  register = function(r)
    r.modifier {
      query = 'attack', layer = 'add', value = function() return 1 end,
      when = function(_, subject, context) return subject == 0 and context == 1 end,
    }
    r.modifier {
      query = 'defense', layer = 'add', value = function() return 1 end,
      when = function(_, subject, context) return subject == 2 and context == 0 end,
    }
  end,
}
-- Figure 0 reaches one hex further against figure 2 alone.
rule_module {
  id = 'long_reach',
  register = function(r)
    r.modifier {
      query = 'range', layer = 'add', value = function() return 1 end,
      when = function(_, subject, context) return subject == 0 and context == 2 end,
    }
  end,
}

local function rules(id, config, addon)
  local addons = {{ id = 'combat:tools' }}
  if addon then addons[2] = { id = 'combat:' .. addon } end
  ruleset {
    id = id, name = id, addons = addons,
    slots = {
      activation = { id = 'core:activation', config = { steps = { List = {{ Str = 'attack' }} } } },
      units = { id = 'core:units' },
      los = { id = 'core:los' },
      combat = { id = 'core:combat', config = config },
    },
  }
end
rules('plain', {})
rules('high', { height_bonus = { Int = 1 } })
rules('covered', { cover = { Bool = true } })
rules('pacifist', {}, 'pacifist')
rules('high_ground', {}, 'high_ground')
rules('long_reach', {}, 'long_reach')
for window in pairs(reactions) do rules('react_' .. window, {}, window) end
for id in pairs(refusals) do rules(id, {}, id) end

local function at(unit, player, q, r, z)
  return { unit = unit, player = player, at = {{ q = q, r = r, z = z or 1 }} }
end
local function fight(id, name, ruleset, map, place, steps)
  test { id = id, name = name, ruleset = ruleset, map = map, place = place, steps = steps }
end
local me, foe = 'p0.test_soldier#1', 'p1.test_soldier#1'
local archer, scout = 'p0.test_archer#1', 'p0.test_scout#1'
local duel = { at('test_soldier', 0, 0, 0), at('test_soldier', 1, 1, 0) }

fight('wound', 'hits beyond blocks wound the target', 'plain', 'field', duel, {
  act.activate(0, me),
  act.attack(me, foe, { rig = { { 'hit', 'hit' }, { 'block', 'blank' } } }),
  expect.event('wounds_applied', { figure = foe }), expect.event('attack'),
  expect.figure(foe, 'on_board'),
})
fight('destroy', 'wounds that reach life destroy the target', 'plain', 'field', duel, {
  act.activate(0, me),
  act.attack(me, foe, { rig = { { 'hit', 'hit' }, { 'blank', 'blank' } } }),
  expect.event('figure_destroyed', { figure = foe }), expect.figure(foe, 'destroyed'),
})
fight('blocked', 'blocks that match the hits leave the target unhurt', 'plain', 'field', duel, {
  act.activate(0, me),
  act.attack(me, foe, { rig = { { 'hit', 'blank' }, { 'block', 'block' } } }),
  expect.event('attack'), expect.figure(foe, 'on_board'),
})
fight('unrigged', 'an attack rolls from the command stream', 'plain', 'field', duel, {
  act.activate(0, me), act.attack(me, foe), expect.event('dice_rolled'), expect.event('attack'),
})

fight('vote', 'a can_target vote can deny an adjacent target', 'pacifist', 'field', duel, {
  act.activate(0, me), act.action(me, 'survey'),
})

-- The archer on (0, 0) has range 3: (3, 0) is in range, (4, 0) is not.
local volley = { at('test_archer', 0, 0, 0), at('test_soldier', 1, 3, 0), at('test_soldier', 1, 4, 0) }
fight('range', 'an archer lists the enemies within its range', 'plain', 'field', volley, {
  act.activate(0, archer), act.action(archer, 'survey'),
})
fight('range_attack', 'an archer attacks an enemy at its full range', 'plain', 'field', volley, {
  act.activate(0, archer),
  act.attack(archer, 'p1.test_soldier#1', { rig = { { 'hit', 'hit' }, { 'blank', 'blank' } } }),
  expect.event('wounds_applied', { figure = 'p1.test_soldier#1' }),
})
-- A high wall on (2, 0) hides (3, 0) from (0, 0); (0, 1) is adjacent, and (1, 1), two hexes
-- away, is in the open.
fight('sight', 'beyond adjacency only enemies in sight are listed', 'plain', 'high_wall',
  { at('test_archer', 0, 0, 0), at('test_soldier', 1, 3, 0), at('test_soldier', 1, 0, 1),
    at('test_soldier', 1, 1, 1) }, {
  act.activate(0, archer), act.action(archer, 'survey'),
})

-- On `hill`, (1, 0) stands at level 2 and (2, 0) at level 1.
fight('height_attack', 'the higher attacker adds the height bonus to its attack', 'high', 'hill',
  { at('test_soldier', 0, 1, 0, 2), at('test_soldier', 1, 2, 0) }, {
  act.activate(0, me),
  act.attack(me, foe, { rig = { { 'hit', 'hit', 'hit' }, { 'blank', 'blank' } } }),
  expect.figure(foe, 'destroyed'),
})
fight('height_defense', 'the higher target adds the height bonus to its defense', 'high', 'hill',
  { at('test_soldier', 0, 2, 0), at('test_soldier', 1, 1, 0, 2) }, {
  act.activate(0, me),
  act.attack(me, foe, { rig = { { 'hit', 'hit' }, { 'block', 'blank', 'blank' } } }),
  expect.event('wounds_applied', { figure = foe }), expect.figure(foe, 'on_board'),
})
fight('height_off', 'without a height bonus higher ground adds nothing', 'plain', 'hill',
  { at('test_soldier', 0, 1, 0, 2), at('test_soldier', 1, 2, 0) }, {
  act.activate(0, me),
  act.attack(me, foe, { rig = { { 'hit', 'hit' }, { 'blank', 'blank' } } }),
  expect.figure(foe, 'destroyed'),
})

-- The tall scout on (0, 0) sees the soldier on (4, 0) over the low wall on (3, 0), which hides
-- most of it.
local cover = { at('test_scout', 0, 0, 0), at('test_soldier', 1, 4, 0) }
fight('cover', 'a mostly hidden target adds a defense die for cover', 'covered', 'low_wall', cover, {
  act.activate(0, scout),
  act.attack(scout, foe, { rig = { { 'hit', 'hit' }, { 'block', 'blank', 'blank' } } }),
  expect.event('wounds_applied', { figure = foe }),
})
fight('cover_off', 'without the cover rule the target rolls its defense alone', 'plain', 'low_wall', cover, {
  act.activate(0, scout),
  act.attack(scout, foe, { rig = { { 'hit', 'hit' }, { 'block', 'blank' } } }),
  expect.event('wounds_applied', { figure = foe }),
})
fight('cover_open', 'a target in the open has no cover', 'covered', 'field', cover, {
  act.activate(0, scout),
  act.attack(scout, foe, { rig = { { 'hit', 'hit' }, { 'block', 'blank' } } }),
  expect.event('wounds_applied', { figure = foe }),
})
fight('peek', 'a preview shows the pools with cover', 'covered', 'low_wall', cover, {
  act.activate(0, scout), act.action(scout, 'peek', 1),
})

-- Each reaction is offered to the target's controller, player 1, who takes it.
local function react(window, rig, expected)
  local steps = {
    act.activate(0, me), act.attack(me, foe, { rig = rig }),
    expect.decision('window:combat.' .. window), act.answer.option(1),
  }
  for _, e in ipairs(expected) do steps[#steps + 1] = e end
  fight('react_' .. window, 'a reaction in ' .. window, 'react_' .. window, 'field', duel, steps)
end
-- Three attack dice instead of two.
react('before_attack_roll', { { 'hit', 'blank', 'blank' }, { 'blank', 'blank' } },
  { expect.event('wounds_applied', { figure = foe }), expect.figure(foe, 'on_board') })
-- No defense dice: the empty roll still comes from the rig.
react('before_defense_roll', { { 'hit', 'hit' }, {} }, { expect.figure(foe, 'destroyed') })
-- The blocks are thrown away.
react('after_defense_roll', { { 'hit', 'hit' }, { 'block', 'block' } }, { expect.figure(foe, 'destroyed') })
-- The wounded target strikes back.
react('after_wounds', { { 'hit', 'blank' }, { 'blank', 'blank' } },
  { expect.event('wounds_applied', { figure = me }), expect.figure(foe, 'on_board') })
fight('pass', 'a reaction may be passed', 'react_after_defense_roll', 'field', duel, {
  act.activate(0, me), act.attack(me, foe, { rig = { { 'hit', 'hit' }, { 'block', 'block' } } }),
  act.answer.option(0), expect.event('attack'), expect.figure(foe, 'on_board'),
})

-- `volley` rolls three dice at range 5, beyond the soldier's own range of 1.
fight('special', 'a special attack resolves through ctx.combat.attack', 'plain', 'field',
  { at('test_soldier', 0, 0, 0), at('test_soldier', 1, 5, 0) }, {
  act.activate(0, me),
  act.action(me, 'volley', 1, { rig = { { 'hit', 'hit', 'hit' }, { 'block', 'blank' } } }),
  expect.event('attack'), expect.figure(foe, 'destroyed'),
})

-- Each refused reaction is taken; answering re-runs the attack, which the engine rejects.
local function refused(id, name, rig)
  fight(id, 'refused: ' .. name, id, 'field', duel, {
    act.activate(0, me), act.attack(me, foe, { rig = rig }), act.answer.option(1),
  })
end
refused('set_hits_early', 'hits cannot be set before the attack roll', {})
refused('set_late', 'nothing can be set after the wounds', { { 'hit', 'blank' }, { 'blank', 'blank' } })
refused('counter', 'an attack cannot start during another', {})
fight('misfire', 'refused: a special attack cannot roll a negative pool', 'plain', 'field', duel, {
  act.activate(0, me), act.action(me, 'misfire', 1),
})
fight('misjudge', 'refused: a preview cannot roll a negative pool', 'plain', 'field', duel, {
  act.activate(0, me), act.action(me, 'misjudge', 1),
})

-- The target destroys the attacker in a window before a roll: the attack ends there.
fight('ambush_attack', 'an attacker destroyed before its roll rolls nothing', 'ambush_attack',
  'field', duel, {
  act.activate(0, me), act.attack(me, foe), act.answer.option(1),
  expect.figure(me, 'destroyed'), expect.event('attack'), expect.figure(foe, 'on_board'),
})
fight('ambush_defense', 'an attacker destroyed after its roll wounds nothing', 'ambush_defense',
  'field', duel, {
  act.activate(0, me), act.attack(me, foe, { rig = { { 'hit', 'hit' } } }), act.answer.option(1),
  expect.figure(me, 'destroyed'), expect.event('attack'), expect.figure(foe, 'on_board'),
})

-- On `ridge`, figure 0 stands on (1, 0) at level 2. Figure 1 on (2, 0) stands lower; figure 2
-- on (1, 1) stands as high. `high_ground` adds an attack die against figure 1 alone, and a
-- defense die to figure 2 against figure 0.
local ridge = {
  at('test_soldier', 0, 1, 0, 2), at('test_soldier', 1, 2, 0), at('test_soldier', 1, 1, 1, 2),
}
local low, level = 'p1.test_soldier#1', 'p1.test_soldier#2'
fight('high_ground', 'an attack modifier reads the target from the context', 'high_ground',
  'ridge', ridge, {
  act.activate(0, me),
  act.attack(me, low, { rig = { { 'hit', 'hit', 'hit' }, { 'blank', 'blank' } } }),
  expect.figure(low, 'destroyed'),
})
fight('high_ground_level', 'the attack modifier skips a target that is not lower', 'high_ground',
  'ridge', ridge, {
  act.activate(0, me),
  act.attack(me, level, { rig = { { 'hit', 'hit' }, { 'block', 'blank', 'blank' } } }),
  expect.event('wounds_applied', { figure = level }), expect.figure(level, 'on_board'),
})
fight('peek_low', 'a preview reads the lower target from the context', 'high_ground',
  'ridge', ridge, {
  act.activate(0, me), act.action(me, 'peek', 1),
})
fight('peek_level', 'a preview reads the level target from the context', 'high_ground',
  'ridge', ridge, {
  act.activate(0, me), act.action(me, 'peek', 2),
})
fight('measure', 'ctx.query.stat takes a context as its third argument', 'high_ground', 'ridge',
  ridge, { act.activate(0, me), act.action(me, 'measure', 1) })
fight('high_ground_unrigged', 'a context modifier rolls from the command stream', 'high_ground',
  'ridge', ridge, {
  act.activate(0, me), act.attack(me, low), expect.event('attack'),
})

-- The soldier on (0, 0) has range 1. Figure 1 on (0, 1) is adjacent; figures 2 and 3 on (2, 0)
-- and (1, 1) stand two hexes away, and `long_reach` brings only figure 2 into range.
local reach = { at('test_soldier', 0, 0, 0), at('test_soldier', 1, 0, 1),
  at('test_soldier', 1, 2, 0), at('test_soldier', 1, 1, 1) }
fight('long_reach', 'a range modifier reads the target from the context', 'long_reach', 'field',
  reach, { act.activate(0, me), act.action(me, 'survey') })
