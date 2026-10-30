# pool-core

A deterministic, event-driven 8-ball pool physics core.

The engine is **event driven and closed form**: between two events every
ball's state is given by an analytic expression, so nothing is ever integrated
step by step. The same shot produces the same bytes on every run and on every
platform.

```rust
use pool_core::{simulate_shot, BallState, ShotInput, TableConfig};
use pool_core::math::Vec3;

let cfg = TableConfig::default();
let mut balls = Vec::new();
balls.push(BallState::at_rest(&cfg, 0, Vec3::new(0.635, 0.635, cfg.ball.radius)));
balls.push(BallState::at_rest(&cfg, 1, Vec3::new(0.9, 0.635, cfg.ball.radius)));
let state = simulate_shot(ShotInput::new(1.5, Vec3::X, balls), &cfg);
println!("the object ball ended at x = {}", state.balls[1].position.x);
```

## What it gives you

| | |
|---|---|
| `simulate_shot` | a shot, and the state it ends in |
| `try_simulate_shot_traced` | the same, with the **path**: the piecewise-analytic segments a renderer interpolates |
| `TableConfig` | every tunable parameter, with an exact JSON round trip |
| `hash_state` | the hash of the canonical byte form of a state: the determinism check |

## The guarantees

- **Deterministic.** `f64` everywhere, the `libm` crate instead of the
  platform's libm, a total order on the events, no hash map iteration.
- **No panics.** The physics path contains no `unwrap`, no `expect` and no
  `panic!`; a degenerate case is reported through the phase of the state.
- **No magic numbers.** Every tunable parameter lives in `TableConfig`.
- **A complete state.** The balls, the phase, the foul and miscue flags, the
  elapsed time and the cue stick as the renderer wants it.

The model follows a research document, and the document is honest about its
own limits: the throw is capped, and the squirt is applied sideways whatever
the direction of the contact offset. Those are reproduced faithfully and
documented.

## The path of a shot

`try_simulate_shot_traced` hands back what a renderer interpolates: one segment
per motion state (§11.2), each of them a straight line in position, in velocity
and in spin. Two things about that path are worth knowing before you read a
renderer off it.

- **A rotation is an orientation, not a step.** `BallState::rotation` is the
  orientation the ball was left turned to by the shot before this one; the
  recorder starts its tally of turn there, and the last frame of the path is the
  orientation this shot leaves the ball in. That is §11.2 handing one shot to the
  next, seen from the input side, and it is what lets a renderer play the last
  shot to its last frame and play this one on from there with nothing integrated
  in between. A ball that was put on the table carries `Vec3::ZERO` and starts
  its turn over.
- **A segment is closed where the vertical spin dies.** §3.2 writes the vertical
  spin as a straight line that decays *towards zero and stops there*, and the
  stepping obeys that. A segment of the path is that same straight line and would
  run past zero, so the recorder closes the segment at the crossing
  (`trace.rs::spin_death`) and opens another from that time with the vertical
  spin and its decay snapped to exactly zero. Only the vertical component decays,
  so only the vertical component is split. The split is a boundary of the *path*
  and not an event: the simulation's stepping is untouched - the same events, the
  same state, the same hash - and a segment stays one motion state, so the
  sampling code stays the one straight line `w0 + wa * dt` and the two halves
  agree where they meet.

## Where to go next

- The full story: the repository's `README.md`.
- The research the model follows: `docs/virtual-pool-physics.md`.
- The validation fixtures: `tests/fixtures/`.
- The Godot binding: [`pool-godot`](https://github.com/krunoo53k/Virtual-Pool-Physics-Engine).

## Licence

MIT.
