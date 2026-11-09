-- Check with an army size of 3: `start_a` has two spaces, `start_b` has three.
return map {
  id = "small_zone", name = "Small zone", orientation = "pointy",
  pieces = {
    { piece = "grass_1", at = { q = 0, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 1, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 2, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 3, r = 0 }, rot = 0, level = 0 },
    { piece = "grass_1", at = { q = 4, r = 0 }, rot = 0, level = 0 },
  },
  zones = {
    start_a = { { q = 0, r = 0 }, { q = 1, r = 0 } },
    start_b = { { q = 2, r = 0 }, { q = 3, r = 0 }, { q = 4, r = 0 } },
  },
}
