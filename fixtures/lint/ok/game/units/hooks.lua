local fixed = 4
ability {
  name = "Counter",
  text = "Counts 1 destroyed ally.",
  vars = { seen = 0 },
  after = {
    figure_destroyed = function(ctx, self, e)
      local total = fixed
      total = total + 1
      ctx.vars(self).seen = total
    end,
  },
}
local helper_state = 0
local function bump() helper_state = helper_state + 1 end
bump()
