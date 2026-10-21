# The demos

Every page here is one shot of the physics core, replayed from the path it
recorded. They are generated files — open `index.html` in a browser, or start
from the source with `cargo run -p pool-visual -- demos`.

| Page | What it shows |
|---|---|
| `break.html` | a full rack, the cue ball into the head ball: the burst and the drops |
| `trio.html` | one shot at three spins — the cue ball goes three different ways |
| `cut.html` | three cut angles: the throw of the object ball off the line of centres |
| `english.html` | three amounts of side spin: the launch line is not the aim line |
| `cushion.html` | four incidence angles into the same rail |
| `jump.html` | an elevated cue: the ball hops off the slate |

The pages are self-contained: no server, no network, no build step. The table on
them is drawn from the core's own `TableGeometry`, and the balls are placed by
evaluating the same closed-form segments the core does, so the trails are the
model's trajectory and not a resampling of it.

To regenerate them after a change to the model:

```sh
cargo run --release -p pool-visual -- demos
```
