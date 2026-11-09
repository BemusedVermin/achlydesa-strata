-- Builds the registrar that a rule module's `register` receives.
-- Returns it and the list it fills: one `{ kind, def }` entry per registration.
local list, r = {}, {}

local kinds = { "action", "modifier", "trigger", "replacement", "reaction", "window", "pair_query" }
for _, kind in ipairs(kinds) do
  r[kind] = function(def)
    list[#list + 1] = { kind = kind, def = def }
  end
end

return r, list
