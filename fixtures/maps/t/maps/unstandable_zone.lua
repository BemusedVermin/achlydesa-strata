return map {
  id = "unstandable_zone", name = "Unstandable zone", orientation = "pointy",
  pieces = {
    { piece = "grass_1", at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 1, r = 0 }, rot = 0, level = 0 },
    { piece = "water_1", at = { q = 2, r = 0 }, rot = 0, level = 0 },
  },
  zones = {
    ford = { { q = 1, r = 0 }, { q = 2, r = 0 }, { q = 5, r = 5 } },
    start_a = { { q = 0, r = 0 } },
    start_b = { { q = 1, r = 0 } },
  },
}
