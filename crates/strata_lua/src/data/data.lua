-- Adds `data.each` and guards raw iteration. Arguments: the `data` table and the sandbox globals.
local data, globals = ...
local sorted_pairs, type, error = globals.pairs, type, error

function data.each(kind, filter)
  local records = data[kind]
  if type(records) ~= "table" or kind == "each" then
    error("data.each: unknown kind `" .. tostring(kind) .. "`", 2)
  end
  local next_record = sorted_pairs(records)
  return function()
    for id, record in next_record do
      local keep = true
      for field, want in sorted_pairs(filter or {}) do
        if record[field] ~= want then keep = false end
      end
      if keep then return id, record end
    end
  end
end

-- Step constructors for `test { steps = { ... } }` (design 16.4). They return plain data.
globals.act = {
  command = function(player, id, payload, opts)
    return { Custom = { player = player, id = id, payload = payload, rig = opts and opts.rig } }
  end,
  place_tokens = function(player, placements)
    return { PlaceTokens = { player = player, placements = placements } }
  end,
  activate = function(player, figure)
    return { Activate = { player = player, figure = figure } }
  end,
  end_activation = function(player)
    return { EndActivation = { player = player } }
  end,
  action = function(figure, action, args, opts)
    return { Action = { figure = figure, action = action, args = args, rig = opts and opts.rig } }
  end,
  attack = function(figure, target, opts)
    return { Attack = { figure = figure, target = target, rig = opts and opts.rig } }
  end,
  answer = {
    figure = function(id) return { Answer = { Figure = id } } end,
    space = function(q, r, z) return { Answer = { Space = { q = q, r = r, z = z } } } end,
    option = function(index) return { Answer = { Option = index } } end,
    yes_no = function(yes) return { Answer = { YesNo = yes } } end,
    number = function(n) return { Answer = { Number = n } } end,
  },
}
globals.expect = {
  stat = function(figure, stat, value)
    return { ExpectStat = { figure = figure, stat = stat, value = value } }
  end,
  event = function(kind, fields)
    return { ExpectEvent = { kind = kind, figure = fields and fields.figure } }
  end,
  figure = function(figure, status)
    return { ExpectFigure = { figure = figure, status = status } }
  end,
  decision = function(prompt)
    return { ExpectDecision = { prompt = prompt } }
  end,
}

function globals.pairs(t)
  if t == data then
    error("cannot iterate `data` with pairs; use data.each(kind, filter)", 2)
  end
  return sorted_pairs(t)
end
