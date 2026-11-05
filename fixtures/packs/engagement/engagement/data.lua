-- Pack tests for `core:engagement`. The native `toy:driver` module supplies `move_to`, a stand-in
-- for the movement action: { figure, q, r, z } puts a figure there in one step, after resolving
-- the reprisals for leaving engagement. Figures are numbered in the order the tests place them.
--
-- The board, with each column's standing level:
--   (0,0) z1   (1,0) z1   (2,0) z1   (3,0) z1   (1,1) z1   (2,-1) z2, one level up
-- (1,0) is next to (2,0), (1,1) and (2,-1). (0,0) is next to none of those three, and (3,0)
-- is not next to (1,0).
stat { id = 'life', name = 'Life', min = 1, max = 9 }
marker { id = 'wound', name = 'Wound', stacking = 'count', visibility = 'public' }
die { id = 'combat', name = 'Combat', faces = { 'hit', 'blank' } }
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
local function at(q, r, level) return { piece = 'meadow', at = { q = q, r = r }, rot = 0, level = level } end
map {
  id = 'field', name = 'Field', orientation = 'pointy',
  pieces = { at(0, 0, 0), at(1, 0, 0), at(2, 0, 0), at(1, 1, 0), at(3, 0, 0),
    at(2, -1, 0), at(2, -1, 1) },
}
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
local function unit_of(id, life, height)
  unit {
    id = id, name = id, kind = 'squad', rarity = 'common', stats = { life = life },
    figures = {{
      id = 'f', count = 1, footprint = 'one', height = height,
      los = { target = 10, hit_zone = {0, 12 * height}, samples = {'center'} },
    }},
  }
end
unit_of('test_soldier', 2, 1)
unit_of('test_scout', 1, 1)
unit_of('test_giant', 2, 2)

-- Figure 0 never provokes reprisals.
rule_module {
  id = 'evasive',
  register = function(r)
    r.modifier {
      query = 'provokes_reprisal', layer = 'set', value = function() return false end,
      when = function(_, subject) return subject == 0 end,
    }
  end,
}
-- Figure 0 is engaged with every adjacent enemy, whatever their levels.
rule_module {
  id = 'reach',
  register = function(r)
    r.modifier {
      query = 'is_engaged', layer = 'set', value = function() return true end,
      when = function(_, subject) return subject == 0 end,
    }
  end,
}

local function rules(id, config, addon)
  local addons = {{ id = 'toy:driver' }}
  if addon then addons[2] = { id = 'engagement:' .. addon } end
  ruleset {
    id = id, name = id, addons = addons,
    slots = { engagement = { id = 'core:engagement', config = config }, units = { id = 'core:units' } },
  }
end
rules('rs', {})
rules('rs_ignore', { heights = { Str = 'ignore' } })
rules('rs_evasive', {}, 'evasive')
rules('rs_reach', {}, 'reach')

-- Figure 0 for player 0 on (1,0); figure 1, an enemy unless `ally`, on `q, r, z`.
local function pair(mover, q, r, z, ally)
  return {
    { unit = mover, player = 0, at = {{ q = 1, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = ally and 0 or 1, at = {{ q = q, r = r, z = z }} },
  }
end
local function go(figure, q, r, z) return { figure = figure, q = q, r = r, z = z or 1 } end
local function away(rig) return act.command(0, 'move_to', go(0, 0, 0), rig and { rig = { rig } }) end

test {
  id = 'same_level', name = 'leaving an adjacent enemy on the same level gives it a reprisal',
  ruleset = 'rs', map = 'field', place = pair('test_soldier', 2, 0, 1),
  steps = {
    away({ 'blank' }),
    expect.event('reprisal'),
    expect.event('dice_rolled'),
    expect.event('figure_moved', { figure = 'p0.test_soldier#1' }),
  },
}
test {
  id = 'hit', name = 'a reprisal hit wounds the leaving figure, which still moves',
  ruleset = 'rs', map = 'field', place = pair('test_soldier', 2, 0, 1),
  steps = {
    away({ 'hit' }),
    expect.event('wounds_applied', { figure = 'p0.test_soldier#1' }),
    expect.event('figure_moved', { figure = 'p0.test_soldier#1' }),
  },
}
test {
  id = 'destroyed', name = 'a reprisal that destroys the leaving figure stops its move',
  ruleset = 'rs', map = 'field', place = pair('test_scout', 2, 0, 1),
  steps = {
    away({ 'hit' }),
    expect.event('figure_destroyed', { figure = 'p0.test_scout#1' }),
    expect.figure('p0.test_scout#1', 'destroyed'),
  },
}
test {
  id = 'stay', name = 'moving to another space next to the enemy provokes nothing',
  ruleset = 'rs', map = 'field', place = pair('test_soldier', 2, 0, 1),
  steps = {
    act.command(0, 'move_to', go(0, 1, 1)),
    expect.event('figure_moved', { figure = 'p0.test_soldier#1' }),
  },
}
test {
  id = 'ally', name = 'leaving an adjacent ally provokes nothing',
  ruleset = 'rs', map = 'field', place = pair('test_soldier', 2, 0, 1, true),
  steps = { away(), expect.event('figure_moved') },
}
test {
  id = 'higher', name = 'an enemy a level up whose height range misses is not engaged',
  ruleset = 'rs', map = 'field', place = pair('test_soldier', 2, -1, 2),
  steps = { away(), expect.event('figure_moved') },
}
test {
  id = 'tall', name = 'a tall figure is engaged with an enemy a level up',
  ruleset = 'rs', map = 'field', place = pair('test_giant', 2, -1, 2),
  steps = { away({ 'blank' }), expect.event('reprisal'), expect.event('dice_rolled') },
}
test {
  id = 'ignore', name = 'with heights ignored, an enemy a level up is engaged',
  ruleset = 'rs_ignore', map = 'field', place = pair('test_soldier', 2, -1, 2),
  steps = { away({ 'blank' }), expect.event('reprisal') },
}
test {
  id = 'reach', name = 'an is_engaged modifier engages an enemy a level up',
  ruleset = 'rs_reach', map = 'field', place = pair('test_soldier', 2, -1, 2),
  steps = { away({ 'blank' }), expect.event('reprisal') },
}
test {
  id = 'denied', name = 'a figure denied reprisal by a modifier triggers no rolls',
  ruleset = 'rs_evasive', map = 'field', place = pair('test_soldier', 2, 0, 1),
  steps = { away(), expect.event('figure_moved', { figure = 'p0.test_soldier#1' }) },
}
test {
  id = 'denied_other', name = 'the denial is the figure\'s own: its enemy leaving still provokes',
  ruleset = 'rs_evasive', map = 'field', place = pair('test_soldier', 2, 0, 1),
  steps = {
    act.command(1, 'move_to', go(1, 3, 0), { rig = {{ 'blank' }} }),
    expect.event('reprisal'),
  },
}
