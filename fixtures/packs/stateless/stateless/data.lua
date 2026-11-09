-- Pack tests for frozen pack environments: runtime scripts cannot keep state between calls.
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common',
  stats = {}, figures = {{
    id = 's', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}

seen = {}

rule_module {
  id = 'rules',
  register = function(r)
    r.action {
      id = 'count', slots = { 'move' },
      targets = function(ctx, figure)
        calls = (calls or 0) + 1
        return { calls }
      end,
      resolve = function(ctx, figure, target) ctx.emit('counted', target) end,
    }
    r.action {
      id = 'remember', slots = { 'move' },
      targets = function(ctx, figure)
        seen[#seen + 1] = figure
        return { #seen }
      end,
      resolve = function(ctx, figure, target) ctx.emit('remembered', target) end,
    }
    r.action {
      id = 'steady', slots = { 'move' },
      targets = function(ctx, figure)
        local out = {}
        for i = 1, 2 do out[i] = i end
        return out
      end,
      resolve = function(ctx, figure, target) ctx.emit('steady', target) end,
    }
  end,
}

ruleset {
  id = 'rs', name = 'Stateless',
  slots = { activation = { id = 'core:activation' } },
  addons = {{ id = 'stateless:rules' }},
}

local board = {
  { unit = 'test_soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} },
  { unit = 'test_soldier', player = 1, at = {{ q = 0, r = 2, z = 1 }} },
}
local a = 'p0.test_soldier#1'

test {
  id = 'steady', name = 'a targets script that keeps only locals works',
  ruleset = 'rs', place = board,
  steps = { act.activate(0, a), act.action(a, 'steady', 2), expect.event('steady') },
}
test {
  id = 'count', name = 'a targets script that sets a global is rejected',
  ruleset = 'rs', place = board,
  steps = { act.activate(0, a), act.action(a, 'count', 1) },
}
test {
  id = 'remember', name = 'a targets script that writes into a global table is rejected',
  ruleset = 'rs', place = board,
  steps = { act.activate(0, a), act.action(a, 'remember', 1) },
}
