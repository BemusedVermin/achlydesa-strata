-- Line of sight on a row of seven hexes: walls, a bridge deck, cover and intervening figures.
-- Needs the native `t:driver` module the Rust tests register (it handles `hit`) and `core:los`.
terrain { id = 'grass', name = 'Grass', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
feature {
  id = 'wall2', name = 'High wall', blocks_los = true,
  cells = {{ at = { q = 0, r = 0 }, segments = {{ from = 0, to = 2, top = true }} }},
}
feature {
  id = 'wall1', name = 'Low wall', blocks_los = true,
  cells = {{ at = { q = 0, r = 0 }, segments = {{ from = 0, to = 1, top = true }} }},
}

local function row()
  local pieces = {}
  for q = 0, 6 do pieces[#pieces + 1] = { piece = 'meadow', at = { q = q, r = 0 }, rot = 0, level = 0 } end
  return pieces
end
map { id = 'open', name = 'Open', orientation = 'pointy', pieces = row() }
map {
  id = 'high_wall', name = 'High wall', orientation = 'pointy', pieces = row(),
  features = {{ feature = 'wall2', at = { q = 3, r = 0 }, rot = 0 }},
}
map {
  id = 'low_wall', name = 'Low wall', orientation = 'pointy', pieces = row(),
  features = {{ feature = 'wall1', at = { q = 3, r = 0 }, rot = 0 }},
}
local bridge = row()
bridge[#bridge + 1] = { piece = 'meadow', at = { q = 3, r = 0 }, rot = 0, level = 4 }
map { id = 'bridge', name = 'Bridge', orientation = 'pointy', pieces = bridge }

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
local function figure(eye)
  return {
    id = 'f', count = 1, footprint = 'one', height = 1,
    los = { target = eye, hit_zone = {0, 12}, samples = {'center'} },
  }
end
unit { id = 'soldier', name = 'Soldier', kind = 'squad', rarity = 'common', stats = {}, figures = { figure(10) } }
unit { id = 'scout', name = 'Scout', kind = 'squad', rarity = 'common', stats = {}, figures = { figure(20) } }

-- Reports how many enemies the wounded figure can see.
rule_module {
  id = 'rules',
  register = function(r)
    r.trigger {
      event = 'wounds_applied',
      effect = function(ctx, event)
        local seen = ctx.select({ sel = 'enemies', in_los = 'self' }, event.WoundsApplied.figure)
        ctx.emit('seen_' .. #seen, 1)
      end,
    }
  end,
}
ruleset {
  id = 'rs', name = 'Sight',
  slots = { los = { id = 'core:los' } }, addons = {{ id = 't:driver' }, { id = 't:rules' }},
}

local function at(unit, player, q, z)
  return { unit = unit, player = player, at = {{ q = q, r = 0, z = z or 1 }} }
end
local function look(id, name, map, place, seen)
  test {
    id = id, name = name, ruleset = 'rs', map = map, place = place,
    steps = { act.command(0, 'hit', 0), expect.event('seen_' .. seen) },
  }
end

look('open', 'open ground is seen across', 'open', { at('soldier', 0, 0), at('soldier', 1, 5) }, 1)
look('wall', 'a high wall blocks sight', 'high_wall', { at('soldier', 0, 0), at('soldier', 1, 5) }, 0)
look('low_short', 'a low wall blocks a short figure', 'low_wall', { at('soldier', 0, 0), at('soldier', 1, 5) }, 0)
look('low_tall', 'a taller figure sees over a low wall', 'low_wall', { at('scout', 0, 0), at('soldier', 1, 5) }, 1)
look('bridge', 'a deck overhead does not block', 'bridge', { at('soldier', 0, 0), at('soldier', 1, 5) }, 1)
look('crowd', 'an intervening figure hides the one behind it', 'open',
  { at('soldier', 0, 0), at('soldier', 1, 5), at('soldier', 1, 2) }, 1)
look('deck', 'a figure on the deck sees past it', 'bridge', { at('soldier', 0, 3, 5), at('soldier', 1, 6) }, 1)
