# The Godot project

The game. The physics is not here: it is in the Rust extension in
[`../pool-godot`](../pool-godot), and this project is the game around it.

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

Headless, as the smoke test:

```sh
godot --headless --path godot-project
```

The smoke test fires the shot that `pool-godot/tests/headless.rs` describes,
checks the hash against the value both sides pin, and exits with 1 if it does
not match.

## The frame

The core works in a right handed frame with `x` along the table, `y` across it
and `z` up. Godot has `y` up, so the extension converts once, in
`pool-godot/src/frame.rs`:

```
core (x, y, z)  ->  Godot (x, z, -y)
```

One of the two in-plane axes has to change sign for the frame to stay right
handed, and that keeps the sense of the spin - so a ball that rolls to the
left rolls to the left on the screen.

The english follows the same rule, and it is the one that is easy to get
wrong: the core measures the contact offset along `up x direction`, which is
the shooter's **left**, so a tip dragged to the shooter's right is a
**negative** `contact_offset_x`. The tests in `pool-godot/tests/mapping.rs`
pin that with the physics: right english throws the object ball to the
shooter's left.

## What is not here yet

This is the plumbing: the extension, the frame and the smoke test. The table,
the balls, the cue and the rules are the next milestones.
