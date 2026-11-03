stat { id = 'defense', name = 'Defense', min = 0, max = 9 }
footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
unit {
  id = 'soldier', name = 'Soldier', kind = 'squad', rarity = 'common',
  stats = { defense = 4 },
  figures = {{
    id = 'f', count = 1, footprint = 'one', height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }},
}
ruleset { id = 'empty', name = 'Empty' }
test {
  id = 'base_defense', name = 'a soldier has its base defense', ruleset = 'empty',
  place = {{ unit = 'soldier', player = 0, at = {{ q = 0, r = 0, z = 1 }} }},
  steps = { expect.stat('p0.soldier#1', 'defense', 4) },
}
