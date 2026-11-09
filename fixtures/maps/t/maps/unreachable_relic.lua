-- The island is across water, and the tower stands two levels above the grass beside it.
return map {
  id = "unreachable_relic", name = "Unreachable relic", orientation = "pointy",
  pieces = {
    { piece = "grass_1", at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 1, r = 0 }, rot = 0, level = 0 },
    { piece = "water_1", at = { q = 2, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 3, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 0, r = 1 }, rot = 0, level = 0 },
    { piece = "hill_2",  at = { q = 0, r = 1 }, rot = 0, level = 1 },
  },
  zones = { start_a = { { q = 0, r = 0 } }, start_b = { { q = 1, r = 0 } } },
  relic_spots = { island = { q = 3, r = 0 }, tower = { q = 0, r = 1 } },
}
