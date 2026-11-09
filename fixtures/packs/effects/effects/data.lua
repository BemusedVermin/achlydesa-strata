-- Pack tests for the effect descriptors of design §8.2 and the conditions that need modules.
-- Every effect and condition here is plain data, so it runs natively, without entering Lua; only
-- the reactions' `when`, which names the player who may react, is a function.
--
-- Actions run their effect for the acting figure, aimed at their target. Figures are numbered in
-- the order the tests place them, and every space stands at level 1.
stat { id = 'life', name = 'Life', min = 1, max = 9 }
stat { id = 'range', name = 'Range', min = 0, max = 9 }
stat { id = 'attack', name = 'Attack', min = 0, max = 9 }
stat { id = 'defense', name = 'Defense', min = 0, max = 9 }
marker { id = 'wound', name = 'Wound', stacking = 'count', visibility = 'public' }
marker { id = 'stunned', name = 'Stunned', stacking = 'count', visibility = 'public' }
die { id = 'combat', name = 'Combat', faces = { 'hit', 'hit', 'hit', 'block', 'block', 'blank' } }
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }

-- Two rows, q = 0..6 on r = 0 and r = 1.
local pieces = {}
for q = 0, 6 do
  for r = 0, 1 do
    pieces[#pieces + 1] = { piece = 'meadow', at = { q = q, r = r }, rot = 0, level = 0 }
  end
end
map { id = 'field', name = 'Field', orientation = 'pointy', pieces = pieces }

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
local function unit_of(id, stats)
  unit {
    id = id, name = id, kind = 'squad', rarity = 'common', stats = stats,
    figures = {{
      id = 'f', count = 1, footprint = 'one', height = 1,
      los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
    }},
  }
end
unit_of('test_soldier', { life = 3, range = 1, attack = 2, defense = 2 })
unit_of('test_archer', { life = 3, range = 4, attack = 2, defense = 2 })

local enemies = { sel = 'enemies' }
rule_module {
  id = 'effects',
  register = function(r)
    local function act_with(id, resolve, targets)
      r.action { id = id, slots = {}, targets = targets, resolve = resolve }
    end
    act_with('smite', { eff = 'wound', amount = 2 }, enemies)
    act_with('mend', { eff = 'heal' }, enemies)
    act_with('blink', { eff = 'place', who = 'self', within = 3 })
    act_with('banish', { eff = 'remove' }, enemies)
    act_with('shove', { eff = 'move', up_to = 1 }, enemies)
    act_with('focus', { eff = 'grant_slot', slot = 'attack' })
    act_with('daze', { eff = 'add_marker', marker = 'stunned', count = 2, rounds = 1 }, enemies)
    act_with('sway', { eff = 'take_control' }, enemies)
  end,
}

-- The target's controller may react.
local function target_controller(ctx)
  local attack = ctx.combat.current()
  return ctx.state.figures[attack.target + 1].controller
end
rule_module {
  id = 'evasion',
  register = function(r)
    r.reaction { window = 'combat.after_defense_roll', when = target_controller,
                 effect = { eff = 'cancel_wounds' } }
  end,
}
-- The same reaction in the other combat windows.
for _, window in ipairs { 'before_attack_roll', 'before_defense_roll', 'after_wounds' } do
  rule_module {
    id = 'evasion_' .. window,
    register = function(r)
      r.reaction { window = 'combat.' .. window, when = target_controller,
                   effect = { eff = 'cancel_wounds' } }
    end,
  }
end
rule_module {
  id = 'riposte',
  register = function(r)
    r.reaction { window = 'combat.after_defense_roll', when = target_controller,
                 effect = { eff = 'excess_blocks_wound_attacker' } }
  end,
}
rule_module {
  id = 'ward',
  register = function(r)
    r.replacement { event = 'wounds_applied', replace = { eff = 'cancel_wounds' } }
  end,
}
-- Replacements whose effect emits the very kind of event they replace: the replacement does not
-- apply again to the event it emits itself.
rule_module {
  id = 'echo',
  register = function(r)
    r.replacement { event = 'wounds_applied', replace = { eff = 'wound', amount = 1 } }
  end,
}
rule_module {
  id = 'drift',
  register = function(r)
    r.replacement { event = 'figure_moved', replace = { eff = 'move', up_to = 1 } }
  end,
}
-- Native filters and `when`s see `target`. `evasion_native` lets the defender react while its
-- attacker is engaged; `when` answers true, so the defender's controller reacts. `watchful`
-- stuns a wounded figure only while it is engaged.
rule_module {
  id = 'evasion_native',
  register = function(r)
    r.reaction { window = 'combat.after_defense_roll',
                 when = { cond = 'engaged', who = { sel = 'target' } },
                 effect = { eff = 'cancel_wounds' } }
  end,
}
rule_module {
  id = 'watchful',
  register = function(r)
    r.trigger { event = 'wounds_applied', filter = { cond = 'engaged', who = { sel = 'target' } },
                effect = { eff = 'add_marker', marker = 'stunned' } }
  end,
}
-- Follow-through (§8.3): after attacking, the attacker may attack again. core:combat's `attack`
-- event gives the attacker as `self`; `unit_removed` is about no figure, so its effect selects
-- nothing.
rule_module {
  id = 'follow_through',
  register = function(r)
    r.trigger { event = 'attack', effect = { eff = 'grant_slot', slot = 'attack' } }
    r.trigger { event = 'unit_removed', effect = { eff = 'grant_slot', slot = 'attack' } }
  end,
}
-- A wound from a melee attack stuns the wounded figure.
rule_module {
  id = 'stagger',
  register = function(r)
    r.trigger { event = 'wounds_applied', filter = { cond = 'attack_kind', is = 'melee' },
                effect = { eff = 'add_marker', marker = 'stunned' } }
  end,
}
-- Engaged figures attack with one more die; figures that see an enemy defend with one more.
rule_module {
  id = 'senses',
  register = function(r)
    r.modifier { query = 'attack', layer = 'add', value = 1, when = { cond = 'engaged' } }
    r.modifier { query = 'defense', layer = 'add', value = 1,
                 when = { cond = 'in_los', to = { sel = 'enemies' } } }
  end,
}

local function rules(id, ...)
  local addons = {{ id = 'effects:effects' }}
  for _, addon in ipairs { ... } do addons[#addons + 1] = { id = 'effects:' .. addon } end
  ruleset {
    id = id, name = id, addons = addons,
    slots = {
      activation = { id = 'core:activation', config = { steps = { List = {{ Str = 'attack' }} } } },
      units = { id = 'core:units' },
      los = { id = 'core:los' },
      engagement = { id = 'core:engagement' },
      combat = { id = 'core:combat' },
    },
  }
end
rules('plain')
local addons = {
  'evasion', 'riposte', 'ward', 'stagger', 'senses', 'echo', 'drift', 'follow_through',
  'evasion_before_attack_roll', 'evasion_before_defense_roll', 'evasion_after_wounds',
}
rules('early_riposte', 'evasion_before_defense_roll', 'riposte')
rules('native_when', 'evasion_native', 'riposte')
rules('watchful', 'watchful')
for _, addon in ipairs(addons) do rules(addon, addon) end

local function at(unit, player, q, r)
  return { unit = unit, player = player, at = {{ q = q, r = r, z = 1 }} }
end
local function case(id, name, ruleset, place, steps)
  test { id = id, name = name, ruleset = ruleset, map = 'field', place = place, steps = steps }
end
local me, foe, archer = 'p0.test_soldier#1', 'p1.test_soldier#1', 'p0.test_archer#1'
local duel = { at('test_soldier', 0, 0, 0), at('test_soldier', 1, 1, 0) }
local apart = { at('test_soldier', 0, 0, 0), at('test_soldier', 1, 4, 0) }
local function acting(steps)
  table.insert(steps, 1, act.activate(0, me))
  return steps
end

case('wound', 'eff.wound wounds the target', 'plain', duel, acting {
  act.action(me, 'smite', 1),
  expect.event('wounds_applied', { figure = foe }), expect.figure(foe, 'on_board'),
})
case('heal', 'eff.heal takes a wound off the target', 'plain', duel, acting {
  act.action(me, 'smite', 1), act.action(me, 'mend', 1),
  expect.event('marker_changed', { figure = foe }),
})
case('place', 'eff.place puts the figure on a chosen empty space', 'plain', duel, acting {
  act.action(me, 'blink'), expect.decision('place'), act.answer.space(3, 0, 1),
  expect.event('figure_moved', { figure = me }),
})
case('remove', 'eff.remove takes the target out of the game', 'plain', duel, acting {
  act.action(me, 'banish', 1),
  expect.event('figure_removed', { figure = foe }), expect.figure(foe, 'removed'),
})
case('move', 'eff.move pushes the target to a chosen space', 'plain', duel, acting {
  act.action(me, 'shove', 1), expect.decision('move'), act.answer.space(2, 0, 1),
  expect.event('figure_moved', { figure = foe }),
})
case('grant_slot', 'eff.grant_slot gives the figure another attack', 'plain', duel, acting {
  act.action(me, 'focus'), expect.event('slots_changed'),
})
case('add_marker', 'eff.add_marker marks the target for the round', 'plain', duel, acting {
  act.action(me, 'daze', 1), expect.event('marker_changed', { figure = foe }),
})
case('take_control', 'eff.take_control hands the target over', 'plain', duel, acting {
  act.action(me, 'sway', 1), expect.event('control_changed'),
})
case('cancel_wounds', 'eff.cancel_wounds as a reaction stops the wounds', 'evasion', duel, acting {
  act.attack(me, foe, { rig = { { 'hit', 'hit' }, { 'blank', 'blank' } } }),
  expect.decision('window:combat.after_defense_roll'), act.answer.option(1),
  expect.event('attack'), expect.figure(foe, 'on_board'),
})
local function window(name) return expect.decision('window:combat.' .. name) end
case('cancel_before_attack', 'eff.cancel_wounds before the attack roll stops the wounds',
  'evasion_before_attack_roll', duel, acting {
  act.attack(me, foe, { rig = { { 'hit', 'hit' }, { 'blank', 'blank' } } }),
  window('before_attack_roll'), act.answer.option(1),
  expect.event('attack'), expect.figure(foe, 'on_board'),
})
case('cancel_before_defense', 'eff.cancel_wounds before the defense roll stops the wounds',
  'evasion_before_defense_roll', duel, acting {
  act.attack(me, foe, { rig = { { 'hit', 'hit' }, { 'blank', 'blank' } } }),
  window('before_defense_roll'), act.answer.option(1),
  expect.event('attack'), expect.figure(foe, 'on_board'),
})
case('cancel_then_riposte', 'cancelled wounds leave the blocks beyond the hits alone',
  'early_riposte', duel, acting {
  act.attack(me, foe, { rig = { { 'hit', 'hit' }, { 'block', 'blank' } } }),
  window('before_defense_roll'), act.answer.option(1),
  window('after_defense_roll'), act.answer.option(1),
  expect.event('attack'), expect.figure(foe, 'on_board'),
})
case('cancel_after_wounds', 'eff.cancel_wounds after the wounds is refused',
  'evasion_after_wounds', duel, acting {
  act.attack(me, foe, { rig = { { 'hit', 'hit' }, { 'blank', 'blank' } } }),
  window('after_wounds'), act.answer.option(1),
})
case('ward', 'eff.cancel_wounds as a replacement cancels the wounds', 'ward', duel, acting {
  act.action(me, 'smite', 1), expect.figure(foe, 'on_board'),
})
case('riposte', 'eff.excess_blocks_wound_attacker wounds the attacker', 'riposte', duel, acting {
  act.attack(me, foe, { rig = { { 'hit', 'blank' }, { 'block', 'block' } } }),
  expect.decision('window:combat.after_defense_roll'), act.answer.option(1),
  expect.event('wounds_applied', { figure = me }),
})
case('stagger_melee', 'cond.attack_kind matches a melee attack', 'stagger', duel, acting {
  act.attack(me, foe, { rig = { { 'hit', 'blank' }, { 'blank', 'blank' } } }),
  expect.event('marker_changed', { figure = foe }),
})
case('stagger_ranged', 'cond.attack_kind does not match a ranged attack', 'stagger',
  { at('test_archer', 0, 0, 0), at('test_soldier', 1, 3, 0) }, {
  act.activate(0, archer),
  act.attack(archer, foe, { rig = { { 'hit', 'blank' }, { 'blank', 'blank' } } }),
  expect.event('wounds_applied', { figure = foe }),
})
case('stagger_no_attack', 'cond.attack_kind is false outside an attack', 'stagger', duel, acting {
  act.action(me, 'smite', 1), expect.event('wounds_applied', { figure = foe }),
})
case('engaged', 'cond.engaged holds next to an enemy', 'senses', duel, {
  expect.stat(me, 'attack', 3),
})
case('not_engaged', 'cond.engaged fails away from enemies', 'senses', apart, {
  expect.stat(me, 'attack', 2),
})
case('in_los', 'cond.in_los holds when an enemy is in sight', 'senses', apart, {
  expect.stat(me, 'defense', 3),
})
case('echo', 'a wound replaced by a wound applies once', 'echo', duel, acting {
  act.action(me, 'smite', 1), expect.event('wounds_applied', { figure = foe }),
})
case('drift', 'a move replaced by a move happens once', 'drift', duel, acting {
  act.action(me, 'blink'), expect.decision('place'), act.answer.space(3, 0, 1),
  expect.decision('move'), act.answer.space(0, 1, 1),
  expect.event('figure_moved', { figure = me }),
})
case('follow_through', 'Follow-through grants the attacker another attack', 'follow_through',
  duel, acting {
  act.attack(me, foe, { rig = { { 'blank', 'blank' }, { 'blank', 'blank' } } }),
  expect.event('attack'), expect.event('slots_changed', { figure = me }),
})
case('no_subject', 'an effect on an event about no figure does nothing', 'follow_through',
  duel, acting {
  act.action(me, 'banish', 1), expect.event('unit_removed'),
})
case('native_when', 'a native when sees the target and lets the defender react', 'native_when',
  duel, acting {
  act.attack(me, foe, { rig = { { 'hit', 'hit' }, { 'block', 'blank' } } }),
  -- Both reactions are the defender's, so they are offered together.
  expect.decision('window:combat.after_defense_roll'), act.answer.option(2),
  expect.decision('window:combat.after_defense_roll'), act.answer.option(1),
  expect.event('attack'), expect.figure(foe, 'on_board'),
})
case('watchful_engaged', 'a native filter sees the target: engaged', 'watchful', duel, acting {
  act.action(me, 'smite', 1), expect.event('marker_changed', { figure = foe }),
})
case('watchful_apart', 'a native filter sees the target: apart', 'watchful', apart, acting {
  act.action(me, 'smite', 1), expect.event('wounds_applied', { figure = foe }),
})
