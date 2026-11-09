-- Water separates the start zones.
return map {
  id = "apart", name = "Apart", orientation = "pointy",
  pieces = {
    { piece = "grass_1", at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = "water_1", at = { q = 1, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 2, r = 0 }, rot = 0, level = 0 },
  },
  zones = { start_a = { { q = 0, r = 0 } }, start_b = { { q = 2, r = 0 } } },
  relic_spots = { west = { q = 0, r = 0 } },
}
