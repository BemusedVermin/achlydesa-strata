-- A tiny sprite for the client's asset tests: one 4x6 idle frame per direction.
pixels_per_level = 6
hex_width_px = 32

sprite {
  key = "sprite.test_soldier",
  sheet = "sprites/test_soldier.png",
  frame = { w = 4, h = 6 },
  anchor = { x = 2, y = 6 },
  animations = {
    idle = { frames = 1, fps = 1 },
  },
}

-- Its sheet does not exist, so the figure falls back to a generated sprite.
sprite {
  key = "sprite.test_archer",
  sheet = "sprites/missing.png",
  frame = { w = 4, h = 6 },
  anchor = { x = 2, y = 6 },
  animations = {
    idle = { frames = 1, fps = 1 },
  },
}
