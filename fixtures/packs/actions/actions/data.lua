-- Pack tests for the built-in `action` command, through `core:activation`. The Rust test supplies
-- the add-on `test:start`, whose `start` command begins the active player's first unit's
-- activation, because there is no pack-test step for that yet.
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
      id = 'shoot', slots = { 'attack' },
      targets = function(ctx, figure) return { 7 } end,
      resolve = function(ctx, figure, target) ctx.emit('shot', target) end,
    }
  end,
}

ruleset {
  id = 'rs', name = 'Actions',
  slots = { activation = { id = 'core:activation' } },
  addons = {{ id = 'test:start' }, { id = 'actions:rules' }},
}

local function board()
  return {
    { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }, { q = 1, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 0, r = 2, z = 1 }, { q = 1, r = 2, z = 1 }} },
  }
end
local function begin(player) return act.command(player, 'start', nil) end
local a, b = 'p0.test_soldier#1', 'p0.test_soldier#2'

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
