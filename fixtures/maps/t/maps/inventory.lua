-- The box holds three grass tiles, no hills and one tree.
return map {
  id = "inventory", name = "Inventory", orientation = "pointy",
  pieces = {
    { piece = "grass_1", at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 1, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 2, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 3, r = 0 }, rot = 0, level = 0 },
    { piece = "hill_2",  at = { q = 4, r = 0 }, rot = 0, level = 0 },
  },
  features = { { feature = "tree", at = { q = 1, r = 0 }, rot = 0 } },
  zones = { start_a = { { q = 0, r = 0 } }, start_b = { { q = 3, r = 0 } } },
  constraints = { inventory = "box" },
}
