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

## Where to go next

- The full story: the repository's `README.md`.
- The research the model follows: `docs/virtual-pool-physics.md`.
- The validation fixtures: `tests/fixtures/`.
- The Godot binding: [`pool-godot`](https://github.com/krunoo53k/Virtual-Pool-Physics-Engine).

## Licence

MIT.
