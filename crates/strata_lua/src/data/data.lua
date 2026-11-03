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
  command = function(player, id, payload)
    return { Custom = { player = player, id = id, payload = payload } }
  end,
}
globals.expect = {
  stat = function(figure, stat, value)
    return { ExpectStat = { figure = figure, stat = stat, value = value } }
  end,
  event = function(kind)
    return { ExpectEvent = { kind = kind } }
  end,
}

function globals.pairs(t)
  if t == data then
    error("cannot iterate `data` with pairs; use data.each(kind, filter)", 2)
  end
  return sorted_pairs(t)
end
