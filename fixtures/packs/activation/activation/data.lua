-- Pack tests for the `act.activate` and `act.end_activation` steps, through `core:activation`.
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = {}, figures = {{
    id = 's', count = 2, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}

ruleset {
  id = 'rs', name = 'Activation',
  slots = { activation = { id = 'core:activation' } },
}

local function board()
  return {
    { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }, { q = 1, r = 0, z = 1 }} },
    { unit = 'test_soldier', player = 1, at = {{ q = 0, r = 2, z = 1 }, { q = 1, r = 2, z = 1 }} },
  }
end

test {
  id = 'activate_end', name = 'a player activates a unit by one of its figures and ends it',
  ruleset = 'rs', place = board(),
  steps = {
    act.activate(0, 'p0.test_soldier#2'), expect.event('activation_started'),
    act.end_activation(0), expect.event('activation_ended'),
    -- The unit may be activated again once the activation has ended.
    act.activate(0, 'p0.test_soldier#1'), expect.event('activation_started'),
  },
}
test {
  id = 'activate_enemy', name = 'a player cannot activate an enemy unit',
  ruleset = 'rs', place = board(),
  steps = { act.activate(0, 'p1.test_soldier#1') },
}
test {
  id = 'end_without_activation', name = 'ending an activation that has not started is refused',
  ruleset = 'rs', place = board(),
  steps = { act.end_activation(0) },
}
test {
  id = 'unknown_figure', name = 'activating by an unknown figure handle fails the test',
  ruleset = 'rs', place = board(),
  steps = { act.activate(0, 'p0.nobody#1') },
}
