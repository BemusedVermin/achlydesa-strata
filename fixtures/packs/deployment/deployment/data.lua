-- Pack tests for `core:deployment`. Units placed with no spaces wait in reserve, and
-- `act.deploy` puts their next figure on the board with the `place_unit` command.
--
-- The board is a row of meadow at level 1 from (0, 0) to (4, 0), one more meadow at (0, 1) and a
-- rock pillar at (1, 1) whose top is level 3. Slot `a` starts in `west`, slot `b` in `east`.
-- `west` also names (-1, 0), which has no terrain. Map `moved` is the same board with `west` at
-- (2, 0) only, for tests that name a map besides their scenario's.
stat { id = 'life', name = 'Life', min = 0, max = 9 }
terrain { id = 'grass', name = 'Grass', standable = true }
terrain { id = 'rock', name = 'Rock', standable = true }
tile { id = 'meadow', name = 'Meadow', terrain = 'grass', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'pillar', name = 'Pillar', terrain = 'rock', thickness = 3, shape = {{ q = 0, r = 0 }} }

local function piece(name, q, r) return { piece = name, at = { q = q, r = r }, rot = 0, level = 0 } end
local function hex(q, r) return { q = q, r = r } end
local function pieces()
  return {
    piece('meadow', 0, 0), piece('meadow', 1, 0), piece('meadow', 2, 0), piece('meadow', 3, 0),
    piece('meadow', 4, 0), piece('meadow', 0, 1), piece('pillar', 1, 1),
  }
end
map {
  id = 'moved', name = 'Moved', orientation = 'pointy', pieces = pieces(),
  zones = { west = { hex(2, 0) }, east = { hex(3, 0), hex(4, 0) } },
}
map {
  id = 'field', name = 'Field', orientation = 'pointy',
  pieces = pieces(),
  zones = {
    west = { hex(-1, 0), hex(0, 0), hex(1, 0), hex(0, 1), hex(1, 1) },
    east = { hex(3, 0), hex(4, 0) },
  },
}

footprint { id = 'one', name = 'One', cells = {{ q = 0, r = 0 }}, move = 'rigid', max_spread = 1 }
footprint {
  id = 'two', name = 'Two', cells = {{ q = 0, r = 0 }, { q = 1, r = 0 }}, move = 'snake', max_spread = 1,
}
local function figure(id, footprint)
  return {
    id = id, count = 1, footprint = footprint, height = 1,
    los = { target = 10, hit_zone = {0, 12}, samples = {'center'} },
  }
end
unit {
  id = 'test_soldier', name = 'Test Soldier', kind = 'squad', rarity = 'common', stats = { life = 1 },
  figures = { figure('s', 'one') },
}
unit {
  id = 'test_beast', name = 'Test Beast', kind = 'hero', rarity = 'common', stats = { life = 1 },
  figures = { figure('b', 'two') },
}

local function slot(id, start, order) return { id = id, start = start, deploy_order = order } end
local function scn(id, a_order, b_order)
  return {
    id = id, name = id, map = 'field', players = { min = 2, max = 2 },
    slots = { slot('a', 'west', a_order), slot('b', 'east', b_order) },
  }
end
scenario(scn('a_first', 1, 2))
scenario(scn('b_first', 2, 1))

ruleset { id = 'rs', name = 'Deployment', slots = { deployment = { id = 'core:deployment' } } }

local function reserve(unit, player) return { unit = unit, player = player, at = {} } end
local function at(q, r, z) return { q = q, r = r, z = z } end
local function deployment_test(t)
  t.ruleset, t.scenario = 'rs', t.scenario or 'a_first'
  t.place = t.place or { reserve('test_soldier', 0), reserve('test_soldier', 1) }
  test(t)
end

deployment_test {
  id = 'legal', name = 'each player deploys inside their start zone in turn',
  steps = {
    act.deploy(0, 'p0.test_soldier#1', at(0, 0, 1)),
    expect.event('figure_moved', { figure = 'p0.test_soldier#1' }),
    expect.figure('p0.test_soldier#1', 'on_board'),
    expect.figure('p1.test_soldier#1', 'reserve'),
    act.deploy(1, 'p1.test_soldier#1', at(4, 0, 1)),
    expect.figure('p1.test_soldier#1', 'on_board'),
  },
}
deployment_test {
  id = 'order', name = 'a player cannot deploy before an earlier slot has finished',
  steps = { act.deploy(1, 'p1.test_soldier#1', at(4, 0, 1)) },
}
deployment_test {
  id = 'reversed', name = 'the slot with the lowest deploy order deploys first',
  scenario = 'b_first',
  steps = {
    act.deploy(1, 'p1.test_soldier#1', at(3, 0, 1)),
    act.deploy(0, 'p0.test_soldier#1', at(1, 0, 1)),
    expect.figure('p0.test_soldier#1', 'on_board'),
  },
}
deployment_test {
  id = 'outside', name = 'a placement outside the start zone is refused',
  steps = { act.deploy(0, 'p0.test_soldier#1', at(2, 0, 1)) },
}
deployment_test {
  id = 'no_terrain', name = 'a placement where the board has no terrain is refused',
  steps = { act.deploy(0, 'p0.test_soldier#1', at(-1, 0, 1)) },
}
deployment_test {
  id = 'wrong_level', name = 'a placement at a level with no surface is refused',
  steps = { act.deploy(0, 'p0.test_soldier#1', at(0, 0, 2)) },
}
deployment_test {
  id = 'pillar', name = 'a figure may deploy on top of a pillar in its zone',
  steps = {
    act.deploy(0, 'p0.test_soldier#1', at(1, 1, 3)),
    expect.figure('p0.test_soldier#1', 'on_board'),
  },
}
deployment_test {
  id = 'taken', name = 'a placement on another figure is refused',
  place = { reserve('test_soldier', 0), reserve('test_soldier', 0), reserve('test_soldier', 1) },
  steps = {
    act.deploy(0, 'p0.test_soldier#1', at(0, 0, 1)),
    act.deploy(0, 'p0.test_soldier#2', at(0, 0, 1)),
  },
}
deployment_test {
  id = 'enemy', name = 'a player cannot deploy an enemy unit',
  steps = { act.deploy(0, 'p1.test_soldier#1', at(0, 0, 1)) },
}
deployment_test {
  id = 'placed', name = 'a unit with nothing in reserve cannot be deployed again',
  place = { reserve('test_soldier', 0), reserve('test_soldier', 1) },
  steps = {
    act.deploy(0, 'p0.test_soldier#1', at(0, 0, 1)),
    act.deploy(0, 'p0.test_soldier#1', at(1, 0, 1)),
  },
}
deployment_test {
  id = 'turned', name = 'a two-space figure deploys turned so both cells are in the zone',
  place = { reserve('test_beast', 0), reserve('test_soldier', 1) },
  steps = {
    act.deploy(0, 'p0.test_beast#1', at(1, 0, 1), 3),
    expect.figure('p0.test_beast#1', 'on_board'),
  },
}
deployment_test {
  id = 'unturned', name = 'a two-space figure with a cell outside the zone is refused',
  place = { reserve('test_beast', 0), reserve('test_soldier', 1) },
  steps = { act.deploy(0, 'p0.test_beast#1', at(1, 0, 1)) },
}
deployment_test {
  id = 'spread', name = 'a two-space figure across too great a level difference is refused',
  place = { reserve('test_beast', 0), reserve('test_soldier', 1) },
  steps = { act.deploy(0, 'p0.test_beast#1', at(0, 1, 1)) },
}
deployment_test {
  id = 'overlap', name = 'a two-space figure overlapping another figure is refused',
  place = { reserve('test_soldier', 0), reserve('test_beast', 0), reserve('test_soldier', 1) },
  steps = {
    act.deploy(0, 'p0.test_soldier#1', at(1, 0, 1)),
    act.deploy(0, 'p0.test_beast#1', at(0, 0, 1)),
  },
}
deployment_test {
  id = 'map_zone', name = 'the start zone comes from the map the test names',
  map = 'moved',
  steps = {
    act.deploy(0, 'p0.test_soldier#1', at(2, 0, 1)),
    expect.figure('p0.test_soldier#1', 'on_board'),
  },
}
deployment_test {
  id = 'map_zone_outside', name = 'the scenario map\'s start zone does not apply under another map',
  map = 'moved',
  steps = { act.deploy(0, 'p0.test_soldier#1', at(0, 0, 1)) },
}
