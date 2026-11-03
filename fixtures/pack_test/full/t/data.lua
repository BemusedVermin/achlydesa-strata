-- Needs the native `t:driver` module the Rust tests register: it handles `hit` and `kill`.
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
map {
  id = 'flat', name = 'Flat', orientation = 'pointy',
  pieces = {
    { piece = 'meadow', at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = 'meadow', at = { q = 1, r = 0 }, rot = 0, level = 0 },
  },
}
map {
  id = 'broken', name = 'Broken', orientation = 'pointy',
  pieces = {{ piece = 'ghost_tile', at = { q = 0, r = 0 }, rot = 0, level = 0 }},
}
stat { id = 'defense', name = 'Defense', min = 0, max = 9 }
die { id = 'd', name = 'D', faces = { 'six', 'one' } }
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'soldier', name = 'Soldier', kind = 'squad', rarity = 'common',
  stats = { defense = 4 },
  figures = {{
    id = 'f', count = 2, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}
unit {
  id = 'dummy', name = 'Dummy', kind = 'squad', rarity = 'common',
  stats = { defense = 1 },
  figures = {{
    id = 'd', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}
rule_module {
  id = 'rules',
  register = function(r)
    r.modifier { query = 'defense', layer = 'add', value = function() return 1 end }
    r.trigger {
      event = 'wounds_applied',
      effect = function(ctx, event)
        ctx.emit('reacted', 1)
        if #ctx.select({ sel = 'enemies' }, event.WoundsApplied.figure) == 1 then
          ctx.emit('one_enemy', 1)
        end
      end,
    }
  end,
}
rule_module {
  id = 'decide',
  register = function(r)
    r.trigger {
      event = 'wounds_applied',
      effect = function(ctx)
        local roll = ctx.dice.roll('d', 2)
        local pick = ctx.choose { chooser = 0, prompt = 'pick', number = { min = 1, max = 3 } }
        if roll:faces()[1] == 'six' and pick == 2 then ctx.emit('lucky', 1) end
      end,
    }
  end,
}
ruleset { id = 'rs', name = 'Test', addons = {{ id = 't:driver' }, { id = 't:rules' }} }
ruleset { id = 'rs_decide', name = 'Decide', addons = {{ id = 't:driver' }, { id = 't:decide' }} }

local function soldiers()
  return {{ unit = 'soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }, { q = 1, r = 0, z = 1 }} }}
end
local function dummy()
  return {{ unit = 'dummy', player = 0, at = {{ q = 0, r = 0, z = 1 }} }}
end
local function versus()
  local both = soldiers()
  both[2] = { unit = 'dummy', player = 1, at = {{ q = 2, r = 0, z = 1 }} }
  return both
end

test {
  id = 'passes', name = 'a modifier raises a stat and a script reacts to an event',
  ruleset = 'rs', map = 'flat', place = soldiers(),
  steps = {
    expect.stat('p0.soldier.f#2', 'defense', 5),
    act.command(0, 'hit', 1),
    expect.event('wounds_applied', { figure = 'p0.soldier.f#2' }),
    expect.event('reacted'),
  },
}
test {
  id = 'native_select', name = 'a script selects enemies with a native descriptor',
  ruleset = 'rs', place = versus(),
  steps = { act.command(0, 'hit', 0), expect.event('one_enemy') },
}
test {
  id = 'dice_and_decision', name = 'rigged dice survive answering a decision',
  ruleset = 'rs_decide', place = dummy(),
  steps = {
    act.command(0, 'hit', 0, { rig = {{ 'six', 'one' }} }),
    expect.decision('pick'),
    act.answer.number(2),
    expect.event('lucky'),
  },
}
test {
  id = 'figure_status', name = 'a destroyed figure is reported',
  ruleset = 'rs', place = soldiers(),
  steps = {
    act.command(0, 'kill', 1),
    expect.figure('p0.soldier.f#2', 'destroyed'),
    expect.figure('p0.soldier.f#1', 'on_board'),
  },
}
test {
  id = 'bare_unit_handle', name = 'a handle may leave out the figure name',
  ruleset = 'rs', place = dummy(),
  steps = { expect.stat('p0.dummy#1', 'defense', 2) },
}
test {
  id = 'wrong_stat', name = 'wrong stat', ruleset = 'rs', place = soldiers(),
  steps = { expect.stat('p0.soldier.f#1', 'defense', 5), expect.stat('p0.soldier.f#1', 'defense', 9) },
}
test {
  id = 'wrong_event', name = 'wrong event', ruleset = 'rs', place = soldiers(),
  steps = { act.command(0, 'hit'), expect.event('figure_destroyed') },
}
test {
  id = 'wrong_event_figure', name = 'wrong event figure', ruleset = 'rs', place = soldiers(),
  steps = { act.command(0, 'hit', 0), expect.event('wounds_applied', { figure = 'p0.soldier.f#2' }) },
}
test {
  id = 'wrong_roll', name = 'wrong roll', ruleset = 'rs_decide', place = dummy(),
  steps = {
    act.command(0, 'hit', 0, { rig = {{ 'one', 'one' }} }),
    act.answer.number(2),
    expect.event('lucky'),
  },
}
test {
  id = 'wrong_decision', name = 'wrong decision', ruleset = 'rs_decide', place = dummy(),
  steps = { act.command(0, 'hit', 0), expect.decision('other') },
}
test {
  id = 'wrong_status', name = 'wrong status', ruleset = 'rs', place = dummy(),
  steps = { expect.figure('p0.dummy#1', 'destroyed') },
}
test {
  id = 'unknown_face', name = 'unknown face', ruleset = 'rs_decide', place = dummy(),
  steps = { act.command(0, 'hit', 0, { rig = {{ 'six', 'seven' }} }) },
}
test {
  id = 'short_rig', name = 'short rig', ruleset = 'rs_decide', place = dummy(),
  steps = { act.command(0, 'hit', 0, { rig = {{ 'six' }} }) },
}
test {
  id = 'no_decision', name = 'answer without a decision', ruleset = 'rs', place = dummy(),
  steps = { act.answer.number(1) },
}
test {
  id = 'unknown_action', name = 'unknown action', ruleset = 'rs', place = dummy(),
  steps = { act.action('p0.dummy#1', 'nope') },
}
test {
  id = 'bad_handle', name = 'bad handle', ruleset = 'rs', place = dummy(),
  steps = { expect.stat('p0.dummy', 'defense', 2) },
}
test {
  id = 'missing_figure', name = 'missing figure', ruleset = 'rs', place = dummy(),
  steps = { expect.stat('p1.dummy#1', 'defense', 2) },
}
test {
  id = 'unknown_command', name = 'unknown command', ruleset = 'rs', place = dummy(),
  steps = { act.command(0, 'nope') },
}
test {
  id = 'unknown_unit', name = 'unknown unit', ruleset = 'rs',
  place = {{ unit = 'ghost', player = 0, at = {{ q = 0, r = 0, z = 1 }} }},
}
test {
  id = 'wrong_count', name = 'wrong figure count', ruleset = 'rs',
  place = {{ unit = 'soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} }},
}
test { id = 'no_ruleset', name = 'no ruleset' }
test { id = 'unknown_ruleset', name = 'unknown ruleset', ruleset = 'nope' }
test { id = 'unknown_map', name = 'unknown map', ruleset = 'rs', map = 'nope' }
test { id = 'broken_map', name = 'broken map', ruleset = 'rs', map = 'broken' }
