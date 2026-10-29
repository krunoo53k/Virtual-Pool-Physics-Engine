# The Godot project

The game. The physics is not here: it is in the Rust extension in
[`../pool-godot`](../pool-godot), and this project is the game around it.

![the table](docs/table.png)

## How it fits together

The core is the **only** physics in the game. Godot never simulates the balls:
the core resolves a shot in one call and hands back the path of every ball,
and the game plays that path back.

```
the player aims                →  PoolCore.preview_cue_strike(...)
the player shoots              →  PoolCore.simulate(...)        →  a PoolShot
every frame                    →  PoolShot.sample(t)            →  the positions
the shot ends                  →  PoolShot.get_final_state()    →  the rules
```

`preview_cue_strike` and `simulate` call the *same* strike in the core, so the
line the game draws is the line the ball takes, squirt included.

The table is built from `PoolCore.get_table_geometry()` — the rails, the
pockets and the cushion all come from the model, so the cushion stops exactly
where the model says the pocket begins.

![the break](docs/break.png)

## The controls

| | |
|---|---|
| the mouse | aims |
| **S** | draws the cue back; a flick forward lets it go |
| **E** | holds the english: the mouse then moves the tip on the cue ball |
| **B** | holds the fine aim: the mouse then trims the angle |
| **D** | raises the debug panel: the numbers of the shot, on the screen |

While **B** or **E** is held the mouse stops aiming and only trims or moves
the tip, so the two never fight.

The power of the shot is how far the cue is drawn back, up to 30 cm, and the
flick decides how much of that power the shot gets: a full draw with a lazy
flick is a soft shot, and a full draw with a hard flick is a break.

Once the stroke is taken, the mouse stops aiming: the flick that fires the
shot may land anywhere, and its sideways wobble does not turn the shot on
the way out. The aim and the tip are the player's last look, and the shot
that goes is the one the panel showed them.

The english is the one sign that is easy to get wrong. The core measures the
contact offset along `up x direction`, which is the shooter's **left**, so a
tip dragged to the shooter's right is a **negative** offset, and right english
throws the object ball to the shooter's left. The tests in
`../pool-godot/tests/mapping.rs` pin that with the physics.

## The debug panel and the log

**D** raises the debug panel: while the player aims it names the line of the
cue, the line the ball really leaves on (the squirt included), the tip in
millimetres and the ghost in metres; while the shot runs it names the speed
and direction of the cue ball, the spin it carries in radians a second and
the events resolved so far.

The whole report of every shot is printed to the log whether the panel is up
or not, so the numbers behind a shot can be handed over when a picture
cannot show them:

```sh
godot --path godot-project 2>&1 | tee /tmp/pool.log
```

The panel and the log quote the core's numbers rather than the picture's,
and the core is the only physics: what the panel says, a test can check.

## Building

The extension has to be built before the project will open:

```sh
./godot-project/build-extension.sh        # the release build
./godot-project/build-extension.sh debug  # the debug build
```

It builds `pool-godot` with cargo and copies the library into `bin/`, where
`bin/pool_godot.gdextension` points at it. The build needs no Godot binary:
the bindings come from the prebuilt API of the `godot` crate.

## Running

With Godot 4.6 or later:

```sh
godot --path godot-project
```

The tests, all headless and all with an exit code:

```sh
godot --headless --path godot-project scenes/smoke_test.tscn   # the data survives the trip
godot --headless --path godot-project scenes/play_test.tscn    # a break, and the balls move
godot --headless --path godot-project scenes/input_test.tscn   # the mouse and the keys do what they say
```

And a picture, if you want one:

```sh
godot --path godot-project -- --screenshot /tmp/table.png
```

## The frame

The core works in a right handed frame with `x` along the table, `y` across
it and `z` up. Godot has `y` up, so the extension converts once, in
`../pool-godot/src/frame.rs`:

```
core (x, y, z)  ->  Godot (x, z, -y)
```

One of the two in-plane axes has to change sign for the frame to stay right
handed, and that keeps the sense of the spin — so a ball that rolls to the
left rolls to the left on the screen.

## What is not here yet

The rules. The shot resolves and the balls come to rest, but nothing decides
who won: no 8-ball rules, no fouls beyond the ones the core reports, no
ball-in-hand and no opponent. The next milestone.
