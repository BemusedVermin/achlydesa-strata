-- Builds the sandbox in place. Arguments: the real globals, the original
-- pcall/xpcall, the "is this error fatal?" predicate, the registry of frozen
-- tables (frozen table -> table holding its contents), the host's `lock(t)`,
-- which freezes one table and returns nil or why it cannot, and `where()`,
-- which returns "file:line: " of the innermost script frame.
-- Returns the metamethods frozen tables share; the host freezes the libraries.
local globals, raw_pcall, raw_xpcall, is_fatal, frozen, lock, where = ...

local next, type, error, tostring = next, type, error, tostring
local raw_get, raw_set, raw_len = rawget, rawset, rawlen
local sort = table.sort

for _, name in ipairs { "dofile", "loadfile", "load", "collectgarbage", "next" } do
  globals[name] = nil
end
math.random, math.randomseed = nil, nil

-- Strings share one metatable; hide it so no script can add metamethods that outlive a call.
getmetatable("").__metatable = false

-- Errors raised here name the script line through `where()` rather than an error level, so
-- they stay right when a C function such as `table.insert` did the write.
local function frozen_write()
  error(where() .. "attempt to modify a frozen table", 0)
end

local function global_write(_, key)
  error(where() .. "cannot set global `" .. tostring(key) .. "`: globals are read-only once "
    .. "the packs have loaded; use a local, or keep state in the game through ctx", 0)
end

local function frozen_len(t) return raw_len(frozen[t]) end

-- Makes one table read-only (see lock.rs). Not a tail call, so `where()` sees the caller.
function globals.freeze(t)
  local problem = lock(t)
  if problem then error(where() .. problem, 0) end
  return t
end

function globals.rawget(t, k) return raw_get(frozen[t] or t, k) end
function globals.rawlen(t) return raw_len(frozen[t] or t) end
function globals.rawset(t, k, v)
  if frozen[t] then frozen_write() end
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

return { frozen = frozen_write, global = global_write, len = frozen_len }
