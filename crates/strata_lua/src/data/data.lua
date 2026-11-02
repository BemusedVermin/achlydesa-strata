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

function globals.pairs(t)
  if t == data then
    error("cannot iterate `data` with pairs; use data.each(kind, filter)", 2)
  end
  return sorted_pairs(t)
end
