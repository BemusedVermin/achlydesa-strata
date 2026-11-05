-- Maps for `strata map check` and the editor's validation tests: `fair` is clean, and each other
-- map has the one finding its name says.
terrain { id = 'grass', name = 'Grass', standable = true }
terrain { id = 'water', name = 'Water', standable = false }
tile { id = 'grass_1', name = 'Grass', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'water_1', name = 'Water', terrain = 'water', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'hill_2', name = 'Hill', terrain = 'grass', thickness = 2, shape = {{ q = 0, r = 0 }} }
feature {
  id = 'tree', name = 'Tree', blocks_los = true,
  cells = {{ at = { q = 0, r = 0 }, segments = {{ from = 0, to = 2 }} }},
}
inventory { id = 'box', name = 'Box', tiles = { grass_1 = 3, water_1 = 1 }, features = { tree = 1 } }

for _, name in ipairs({ 'fair', 'overlap', 'unsupported', 'inventory', 'unstandable_zone',
                        'small_zone' }) do
  require('maps.' .. name)
end
