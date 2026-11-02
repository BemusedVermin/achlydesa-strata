local count = 0
ability {
  name = "Counter",
  after = {
    figure_destroyed = function(ctx, self, e)
      local local_total = 0
      local_total = local_total + 1
      count = count + 1
    end,
  },
}
