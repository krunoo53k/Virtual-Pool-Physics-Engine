# Validation fixtures

Acceptance criteria for the physics core. Each `*.json` file is a shot specification plus the
expected outcome, so a test can be written directly against it without re-deriving anything.

The physics these fixtures pin down is specified in
[`docs/virtual-pool-physics.md`](../../docs/virtual-pool-physics.md).

## Files

| Fixture | What it pins down |
|---|---|
| `slide_to_roll.json` | the sign of the sliding-state angular velocity (doc §3.3) |
| `stun_separation_90.json` | the 90° rule, the ball-ball normal components and the throw impulse |
| `cut_throw_sweep.json` | the 1/7 sticking constant and the Coulomb cap on the tangential impulse |
| `cushion_rebound.json` | the cushion impulse model, the derived contact height and the vertical flip |
| `draw_stop_shot.json` | the draw/stop behaviour and the analytic reversal threshold |
| `pocket_drop.json` | the pocket state machine, the rattle and the drop threshold |
| `squirt_curve.json` | the squirt calibration curve (anchors still to be measured) |
| `determinism_golden.json` | bit-exact reproducibility of a full shot |

## Regenerating

```sh
cargo build -p pool-cli          # the generator reads the defaults from this binary
python3 tests/fixtures/generate_fixtures.py
```

The generator computes the expected values from the same equations as the document, reading
the input constants from the core's own default config via `pool-cli config`, so the
expectations cannot drift away from the model. It refuses to run if the CLI is older than
`pool-core/src/config.rs`; set `POOL_CLI` to use a binary other than the default debug one.
Two files contain values that are **filled in by hand** and must survive regeneration:

- `squirt_curve.json` — the `squirt_deg` anchors are `null` and marked `TO BE CALIBRATED`;
  fill them from Cross (2008) or from high-speed video of your table.
- `determinism_golden.json` — `final_state_hash` is the hex hash of the final state of the
  break shot. The generator writes `null` for it; the value in the file was written by hand
  by the first implementation and is the golden value from then on.

The generator never overwrites those fields (it writes `null` for them), so edit them in the
JSON and keep the generator's `TO BE FILLED` markers in place.

## Two quirks of the generator

- `stun_separation_90.json` holds its first expectation inside a nested list. The generator
  builds that entry with a trailing comma, which makes it a one-element tuple. The test
  harness flattens the nested lists, so the file is read either way.
- The break layout of `determinism_golden.json` used to sit at `y = 1.270`, which is the far
  rail of a 1.27 m wide table: the rack was off the table and the shot was not a break. The
  foot spot of a 9 ft table is 25" (0.635 m) from the foot rail and on the centre line of
  the width, so the cue ball and the rack now start at `(0.635, 0.635)`.

## Schema

```jsonc
{
  "id": "slide_to_roll",
  "title": "...",
  "purpose": "what bug this fixture is here to catch",
  "sources": ["..."],
  "setup": {
    "balls": [ { "id": "b0", "position": [x, y], "velocity": [vx, vy],
                 "angular_velocity": [wx, wy, wz] } ],
    "params": { "mu_s": 0.2 }        // config overrides for this shot only
  },
  "expectations": [
    { "quantity": "time_to_rolling", "expected": 0.1456, "tolerance": 1e-9, "unit": "s" }
  ],
  "invariants": [                    // properties that must hold, not just numbers
    { "quantity": "rebound_angle_independence", "statement": "...", "tolerance": 1e-9 }
  ],
  "known_deviations": [ "..." ]      // model limitations recorded on purpose
}
```

Conventions:

- `tolerance` is **absolute**, in the unit of the quantity. Use a tight tolerance for
  closed-form quantities (1e-9) and a looser one for comparisons against measured data
  (0.2°–0.5°).
- Positions are metres, velocities m/s, angles degrees in the fixture (radians internally).
- Every fixture is runnable without a table: `setup` is the complete initial state.
- `params` overrides the table config for that shot only; anything not listed uses the
  defaults in §13.2 of the document.

## The cushion fixture derives its contact height

`cushion_rebound.json` does not take a contact height as an input. It takes the **nose height**
(`nose_height`, the height above the bed at which the cushion acts on a ball resting on the
bed — the WPA specifies 63.5% of the ball diameter) and derives everything else, exactly as the
simulator is meant to:

```
eps   = h_nose - R                             # contact height above the ball's centre
theta = asin(eps / R)                          # tilt of the contact normal
```

For the WPA reference profile that gives `eps = 0.270 R` and `theta = 15.7°`, which is above
the vertical flip threshold of `0.139 R` (a nose height of 0.57 D), so the cushion presses the
ball into the slate and the slate's rebound makes it hop ~3.8 mm.

The `vertical_flip` block sweeps the nose height through the three reference heights Dr. Dave
discusses — 0.500 D (the cushion *launches* the ball, +0.49 m/s), 0.635 D (the WPA spec, presses
it in, −0.46 m/s) and 0.700 D (drives it down hard, −0.87 m/s) — plus the flip between them.
Reproducing all three is the check that the contact height really is the nose height.

The `rail_regimes` block records which surface a ball meets depending on its height: the nose
rubber, the rail structure above it, or nothing at all. The solver must branch on that rather
than extrapolating the rubber model upward.

## The throw fixture compares like with like

`cut_throw_sweep.json` used to carry one model row per cut angle, at a fixed 2 m/s, next to
three columns of measured throw labelled slow, medium and fast. That was a trap: the model's
row was at 2 m/s and the columns were measured at 1, 3 and 7 mph, so the model row was really
being compared against the *fast* column, and the residual looked like a small overshoot that
grew with the cut angle.

Every row is now at one of TP A.14's own speeds, so the model value and the measured value
beside it are the same shot, and the residual is what it says it is:

| cut | speed | model | measured | residual |
|---|---|---|---|---|
| 30° | 1 mph | 4.81° | 4.65° | +0.16° |
| 30° | 3 mph | 3.55° | 4.55° | −1.00° |
| 30° | 7 mph | 2.02° | 2.90° | −0.88° |

**The measured columns cannot be fitted by any friction law of the form `A + B·e^(−K·u)`**,
for any floor term and any decay constant, so they are not a calibration target: the implied
coefficients at a 30° cut are 0.0813, 0.0796 and 0.0507 at slip speeds of 0.224, 0.671 and
1.565 m/s, and for such a law the ratio of the successive differences is bounded below by
`(u₂−u₁)/(u₃−u₂) = 0.50`, while the measured ratio is 0.061. TP A.14's own fit is the
authority for the law; the residual is a recorded deviation, and the test pins it at 1.05°.

## Notes for the test author

- The fixtures state the *physics*, not the API. Wire them to whatever the simulation entry
  point turns out to be; the point is that the numbers do not move.
- `determinism_golden.json` must be run on every platform you ship. A mismatch between
  x86-64 and wasm is almost always a transcendental function that leaked into the core
  (doc §11.4), not an arithmetic difference.
  - The hash has been confirmed on **x86-64** and on **i686-unknown-linux-gnu** — a
    different word size and a different floating-point path (x87 against SSE2) — and it agrees
    bit for bit.
  - The **wasm32** baseline has *not* been established: there is no wasm toolchain here yet.
    Until it has, the golden value is a baseline for one platform and a specification for the
    others.
  - The transcendentals are covered structurally rather than by luck: the core calls the
    `libm` crate and nothing else, and `pool-core/tests/portability.rs` fails if a platform
    `sin`, `cos`, `exp`, `hypot` or `powf` appears anywhere in the core's source.
- `pocket_drop.json` deliberately leaves the rattle outcome to the implementation: once the
  jaw model is in place, add the observed outcome as a golden value so that a change in the
  jaw parameters shows up as a test failure rather than as a silently different game.
