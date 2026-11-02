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
