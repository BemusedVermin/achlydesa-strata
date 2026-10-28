-- Builds the sandbox in place. Arguments: the real globals, the original
-- pcall/xpcall, the "is this error fatal?" predicate, and the registry of
-- frozen tables (frozen table -> table holding its contents).
local globals, raw_pcall, raw_xpcall, is_fatal, frozen = ...

local next, type, error, setmetatable = next, type, error, setmetatable
local raw_get, raw_set, raw_len = rawget, rawset, rawlen
local sort = table.sort

for _, name in ipairs { "dofile", "loadfile", "load", "collectgarbage", "next" } do
  globals[name] = nil
end
math.random, math.randomseed = nil, nil

local function freeze(t)
  if type(t) ~= "table" then error("freeze expects a table", 2) end
  if frozen[t] then return t end
  if getmetatable(t) ~= nil then error("cannot freeze a table with a metatable", 2) end
  local shadow = {}
  for k, v in next, t do shadow[k] = v end
  for k in next, shadow do raw_set(t, k, nil) end
  frozen[t] = shadow
  setmetatable(t, {
    __index = shadow,
    __newindex = function() error("attempt to modify a frozen table", 2) end,
    __len = function() return #shadow end,
    __metatable = false,
  })
  return t
end

function globals.rawget(t, k) return raw_get(frozen[t] or t, k) end
function globals.rawlen(t) return raw_len(frozen[t] or t) end
function globals.rawset(t, k, v)
  if frozen[t] then error("attempt to modify a frozen table", 2) end
  return raw_set(t, k, v)
end

local function key_lt(a, b)
  if type(a) == type(b) then return a < b end
  return type(a) == "number"
end

function globals.pairs(t)
  if type(t) ~= "table" then
    error("bad argument #1 to 'pairs' (table expected, got " .. type(t) .. ")", 2)
  end
  local src, keys, n = frozen[t] or t, {}, 0
  for k in next, src do
    local kind = type(k)
    if kind ~= "number" and kind ~= "string" then
      error("pairs: cannot order keys of type " .. kind, 2)
    end
    n = n + 1
    keys[n] = k
  end
  sort(keys, key_lt)
  local i = 0
  return function()
    while true do
      i = i + 1
      local k = keys[i]
      if k == nil then return nil end
      local v = src[k]
      if v ~= nil then return k, v end
    end
  end, t, nil
end

local function settle(ok, ...)
  if not ok and is_fatal((...)) then error((...), 0) end
  return ok, ...
end

function globals.pcall(f, ...) return settle(raw_pcall(f, ...)) end
function globals.xpcall(f, handler, ...)
  return settle(raw_xpcall(f, function(e)
    if is_fatal(e) then return e end
    return handler(e)
  end, ...))
end

globals.freeze = freeze
for _, lib in ipairs { string, table, math, utf8 } do freeze(lib) end
