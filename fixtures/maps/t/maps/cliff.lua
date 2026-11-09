-- The start zone is on a column four levels above the grass beside it, and the relic spot is a
-- step further on.
return map {
  id = "cliff", name = "Cliff", orientation = "pointy",
  pieces = {
    { piece = "grass_1", at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = "hill_2",  at = { q = 0, r = 0 }, rot = 0, level = 1 },
    { piece = "hill_2",  at = { q = 0, r = 0 }, rot = 0, level = 3 },
    { piece = "grass_1", at = { q = 1, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 2, r = 0 }, rot = 0, level = 0 },
  },
  zones = { start_a = { { q = 0, r = 0 } } },
  relic_spots = { foot = { q = 2, r = 0 } },
}
