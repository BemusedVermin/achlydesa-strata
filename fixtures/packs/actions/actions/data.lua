-- Pack tests for the built-in `action` command and the slot rules, through `core:activation`.
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = {}, figures = {{
    id = 's', count = 2, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}

rule_module {
  id = 'rules',
  register = function(r)
    r.action { id = 'walk', slots = { 'move' }, resolve = function(ctx, figure) ctx.emit('walked', figure) end }
    r.action {
      id = 'boost', slots = { 'attack' },
      resolve = function(ctx, figure) ctx.activation.grant(figure, 'move', 1) end,
    }
    r.action {
      id = 'bad_boost', slots = { 'attack' },
      resolve = function(ctx, figure) ctx.activation.grant(2, 'move', 1) end,
    }
    r.action {
      id = 'shoot', slots = { 'attack' },
      targets = function(ctx, figure) return { 7 } end,
      resolve = function(ctx, figure, target) ctx.emit('shot', target) end,
    }
  end,
}

ruleset {
  id = 'rs', name = 'Actions',
  slots = { activation = { id = 'core:activation' } },
  addons = {{ id = 'actions:rules' }},
}
ruleset {
  id = 'per_figure', name = 'One figure at a time',
  slots = { activation = { id = 'core:activation', config = { all_figures_per_step = { Bool = false } } } },
  addons = {{ id = 'actions:rules' }},
}

local function board()
  return {
    { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }, { q = 1, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 0, r = 2, z = 1 }, { q = 1, r = 2, z = 1 }} },
  }
end
local a, b = 'p0.test_soldier#1', 'p0.test_soldier#2'
local function begin(player) return act.activate(player, player == 0 and a or 'p1.test_soldier#1') end

test {
  id = 'move_then_attack', name = 'a squad moves every figure before any attacks',
  ruleset = 'rs', place = board(),
  steps = {
    begin(0),
    act.action(a, 'walk'), expect.event('walked'),
    act.action(b, 'walk'),
    act.action(a, 'shoot', 7), expect.event('shot'),
    act.action(b, 'shoot', 7),
    expect.event('slots_changed'),
  },
}
test {
  id = 'attack_early', name = 'a figure cannot attack while another has yet to move',
  ruleset = 'rs', place = board(),
  steps = { begin(0), act.action(a, 'walk'), act.action(a, 'shoot', 7) },
}
test {
  id = 'spent_slot', name = 'a figure cannot spend a slot twice',
  ruleset = 'rs', place = board(),
  steps = { begin(0), act.action(a, 'walk'), act.action(a, 'walk') },
}
test {
  id = 'bad_target', name = 'an illegal target is refused',
  ruleset = 'rs', place = board(),
  steps = { begin(0), act.action(a, 'walk'), act.action(b, 'walk'), act.action(a, 'shoot', 9) },
}
test {
  id = 'enemy_figure', name = 'a figure outside the active unit cannot act',
  ruleset = 'rs', place = board(),
  steps = { begin(0), act.action('p1.test_soldier#1', 'walk') },
}
test {
  id = 'no_activation', name = 'nothing can be done before a unit is activated',
  ruleset = 'rs', place = board(),
  steps = { act.action(a, 'walk') },
}
test {
  id = 'one_at_a_time', name = 'with one figure per step, a figure finishes before the next starts',
  ruleset = 'per_figure', place = board(),
  steps = {
    begin(0),
    act.action(a, 'walk'), act.action(a, 'shoot', 7), act.action(b, 'walk'),
  },
}
test {
  id = 'one_at_a_time_wait', name = 'with one figure per step, the next figure cannot move early',
  ruleset = 'per_figure', place = board(),
  steps = { begin(0), act.action(a, 'walk'), act.action(b, 'walk') },
}
test {
  id = 'end_early', name = 'an activation can end early and then nothing is spent',
  ruleset = 'rs', place = board(),
  steps = {
    begin(0), act.action(a, 'walk'),
    act.end_activation(0), expect.event('activation_ended'),
    act.action(b, 'walk'),
  },
}
test {
  id = 'grant_reopens', name = 'a granted move reopens the move step',
  ruleset = 'rs', place = board(),
  steps = {
    begin(0),
    act.action(a, 'walk'), act.action(b, 'walk'),
    act.action(a, 'boost'),
    act.action(a, 'walk'),
    act.action(b, 'shoot', 7), expect.event('shot'),
  },
}
test {
  id = 'grant_blocks', name = 'attacks wait for a granted move',
  ruleset = 'rs', place = board(),
  steps = {
    begin(0),
    act.action(a, 'walk'), act.action(b, 'walk'),
    act.action(a, 'boost'),
    act.action(b, 'shoot', 7),
  },
}
test {
  id = 'grant_outside', name = 'a grant to a figure outside the activation is refused',
  ruleset = 'rs', place = board(),
  steps = { begin(0), act.action(a, 'walk'), act.action(b, 'walk'), act.action(a, 'bad_boost') },
}
