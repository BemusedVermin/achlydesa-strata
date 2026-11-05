-- Two rows of grass with a pond and a hill between the start zones.
return map {
  id = "fair", name = "Fair", orientation = "pointy",
  pieces = {
    { piece = "grass_1", at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 1, r = 0 }, rot = 0, level = 0 },
    { piece = "water_1", at = { q = 2, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 3, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 4, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 0, r = 1 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 1, r = 1 }, rot = 0, level = 0 },
    { piece = "hill_2",  at = { q = 2, r = 1 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 3, r = 1 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 4, r = 1 }, rot = 0, level = 0 },
  },
  zones = {
    start_a = { { q = 0, r = 0 }, { q = 0, r = 1 } },
    start_b = { { q = 4, r = 0 }, { q = 4, r = 1 } },
  },
  relic_spots = { east = { q = 3, r = 1 }, hill = { q = 2, r = 1 } },
  constraints = { require_support = true },
}
