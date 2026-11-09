-- Two packs that both define `rock`, for the palette's naming tests.
terrain { id = 'stone', name = 'Stone', standable = true }
tile { id = 'rock', name = 'Rock', terrain = 'stone', thickness = 1, shape = {{ q = 0, r = 0 }} }
tile { id = 'boulder', name = 'Boulder', terrain = 'stone', thickness = 2, shape = {{ q = 0, r = 0 }} }
