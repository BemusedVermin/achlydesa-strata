-- The box holds three grass tiles, one water tile, no hills and one tree. The map spells grass
-- and water both with and without the pack, and the box counts them the same either way.
return map {
  id = "inventory", name = "Inventory", orientation = "pointy",
  pieces = {
    { piece = "grass_1",   at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = "t:grass_1", at = { q = 1, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1",   at = { q = 2, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1",   at = { q = 3, r = 0 }, rot = 0, level = 0 },
    { piece = "hill_2",    at = { q = 4, r = 0 }, rot = 0, level = 0 },
    { piece = "water_1",   at = { q = 5, r = 0 }, rot = 0, level = 0 },
  },
  features = {
    { feature = "tree", at = { q = 1, r = 0 }, rot = 0 },
    { feature = "t:tree", at = { q = 2, r = 0 }, rot = 0 },
  },
  zones = { start_a = { { q = 0, r = 0 } }, start_b = { { q = 3, r = 0 } } },
  constraints = { inventory = "box" },
}
