# pool-godot

The Godot 4 binding of the [pool physics core](../pool-core): a
[GDExtension](https://godotengine.org) that hands the core to a Godot project.

The core is the **only** physics in the game. Godot never simulates the balls:
the core resolves a shot in one call and hands back the path of every ball,
and the game plays that path back.

## The classes

| Class | What it is |
|---|---|
| `PoolCore` | the table, the aim preview and the shot |
| `PoolShot` | a resolved shot: the path to play back and the state it ends in |

```gdscript
var core := PoolCore.new()
var shot := core.simulate(8.0, Vector3(0, 0, -1), Vector2.ZERO, positions, numbers)
print(shot.get_duration())
print(shot.sample(1.5))     # where every ball is at t = 1.5 s
```

`PoolCore.preview_cue_strike` and `PoolCore.simulate` call the *same* strike
in the core, so the line the game draws at the player is the line the ball
takes, squirt included.

A ball has an orientation as well as a place, and the core hands it across
shots. `PoolCore.simulate` starts every ball straight - what a fresh rack is.
`PoolCore.simulate_with_rotations` takes one more list, the orientation every
ball carries into the shot, in the Godot frame:

```gdscript
var turned := shot.sample_rotation(shot.get_duration())  # where the last shot left them
var next := core.simulate_with_rotations(8.0, Vector3(0, 0, -1), Vector2.ZERO,
    positions, numbers, turned)
```

`PoolShot.sample_rotation(t)` reports the turn every ball has made up to `t`,
counted from the orientation it carried in rather than from zero, and the last
frame of the path is the orientation the shot leaves the ball in (§11.2, the
hand-off). The game plays a shot back to that frame and hands the same numbers
back to `simulate_with_rotations`, so the phase of a ball's turn survives across
shots with nothing integrated in the gap. `godot-project/scripts/ball.gd` puts
the orientation the core reports straight onto the mesh; the rotation of a
previous shot is never added up in GDScript, because the core is the only
physics in the game. A carried orientation moves no hash: the canonical byte
form of a state does not name it.

## The frame

The core works in a right handed frame with `x` along the table, `y` across
it and `z` up. Godot has `y` up, so the extension converts once:

```
core (x, y, z)  ->  Godot (x, z, -y)
```

One of the two in-plane axes has to change sign for the frame to stay right
handed, and that keeps the sense of the spin — so a ball that rolls to the
left rolls to the left on the screen.

The english follows the same rule, and it is the one that is easy to get
wrong: the core measures the contact offset along `up x direction`, which is
the shooter's **left**, so a tip dragged to the shooter's right is a
**negative** `contact_offset_x`.

## Building

```sh
cargo build --release -p pool-godot
```

The bindings come from the prebuilt API of the `godot` crate, so no Godot
binary is needed to build. The library lands in `target/release/` as
`libpool_godot.so` (`.dll` on Windows, `.dylib` on macOS), and the
`.gdextension` file points at it.

The crate is built against the Godot 4.6 API (`api-4-6`), so it runs on
Godot 4.6 and later.

## Where to go next

- The game that uses it: `godot-project/` in the repository.
- The core: [`pool-core`](https://github.com/krunoo53k/Virtual-Pool-Physics-Engine).

## Licence

MIT.
