-- Builds the `ctx` a script receives for one call (design §9.2).
-- Arguments: the host's `op(name, args)` function and a table of module namespaces, each
-- listing the functions it exports. `op` returns `ok, value`; a false `ok` means the call must
-- stop (an unanswered decision or a rejected command), which is raised as `Halt`.
local op, namespaces = ...
local freeze, setmetatable, error, type, tostring = freeze, setmetatable, error, type, tostring
local select, concat, ipairs, pairs = select, table.concat, ipairs, pairs

local function call(name, args)
  local ok, value = op(name, args)
  if not ok then error(Halt, 0) end
  return value
end

local function deep(t)
  if type(t) == "table" then
    for _, v in pairs(t) do deep(v) end
    freeze(t)
  end
  return t
end

local function read_only(what)
  return function() error(what .. " is read-only", 2) end
end

local ctx = {}

local cache = {}
ctx.state = setmetatable({}, {
  __index = function(_, key)
    if cache[key] == nil then cache[key] = deep(call("state", key)) end
    return cache[key]
  end,
  __newindex = read_only("ctx.state"),
  __metatable = false,
})

function ctx.select(selector, subject)
  return deep(call("select", { selector = selector, subject = subject }))
end

ctx.query = setmetatable({}, {
  __index = function(_, name)
    return function(...) return call("query", { name = name, args = { ... } }) end
  end,
  __newindex = read_only("ctx.query"),
  __metatable = false,
})

function ctx.choose(spec) return call("choose", spec) end

local Roll = {}
Roll.__index = Roll

function Roll:count(symbol)
  local n = 0
  for _, face in ipairs(self._faces) do
    if face == symbol then n = n + 1 end
  end
  return n
end

function Roll:sum()
  local total = 0
  for _, result in ipairs(self._results) do total = total + result end
  return total
end

function Roll:faces()
  local copy = {}
  for i, face in ipairs(self._faces) do copy[i] = face end
  return copy
end

-- Asks `chooser` whether to replace the die at `index` (one-based). If they agree, one new die
-- is rolled and takes its place. Returns the resulting roll, or this one if they decline.
function Roll:reroll(chooser, index)
  if index < 1 or index > #self._results then error("reroll: no die at that index", 2) end
  if not call("choose", { chooser = chooser, prompt = "reroll", yes_no = true }) then
    return self
  end
  local again = call("roll", { die = self._die, n = 1 })
  local results, faces = {}, {}
  for i, result in ipairs(self._results) do results[i] = result end
  for i, face in ipairs(self._faces) do faces[i] = face end
  results[index], faces[index] = again.results[1], again.faces[1]
  return setmetatable({ _die = self._die, _results = results, _faces = faces }, Roll)
end

local function rolled(die, n)
  local r = call("roll", { die = die, n = n })
  return setmetatable({ _die = die, _results = r.results, _faces = r.faces }, Roll)
end

ctx.dice = {
  roll = rolled,
  -- Rolls `die` as many times as `figure`'s `stat` says, modifiers included.
  pool = function(die, figure, stat)
    return rolled(die, call("query", { name = "stat", args = { figure, stat } }))
  end,
}

function ctx.vars(owner)
  owner = tostring(owner)
  return setmetatable({}, {
    __index = function(_, name) return deep(call("var_get", { owner = owner, name = name })) end,
    __newindex = function(_, name, value)
      call("var_set", { owner = owner, name = name, value = value })
    end,
    __metatable = false,
  })
end

function ctx.emit(kind, data) call("emit", { kind = kind, data = data }) end

function ctx.log(...)
  local parts = {}
  for i = 1, select("#", ...) do parts[i] = tostring((select(i, ...))) end
  call("log", concat(parts, " "))
end

for namespace, names in pairs(namespaces) do
  local api = {}
  for _, name in ipairs(names) do
    api[name] = function(...)
      return call("api", { namespace = namespace, name = name, args = { ... } })
    end
  end
  ctx[namespace] = freeze(api)
end

return freeze(ctx)
