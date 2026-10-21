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
| `cushion_rebound.json` | the Han cushion impulse model and the vertical flip |
| `draw_stop_shot.json` | the draw/stop behaviour and the analytic reversal threshold |
| `pocket_drop.json` | the pocket state machine, the rattle and the drop threshold |
| `squirt_curve.json` | the squirt calibration curve (anchors still to be measured) |
| `determinism_golden.json` | bit-exact reproducibility of a full shot |

## Regenerating

```sh
python3 tests/fixtures/generate_fixtures.py
```

The generator computes the expected values from the same equations as the document, so the
expectations cannot drift away from the specification. Two files contain values that are
**filled in by hand** and must survive regeneration:

- `squirt_curve.json` — the `squirt_deg` anchors are `null` and marked `TO BE CALIBRATED`;
  fill them from Cross (2008) or from high-speed video of your table.
- `determinism_golden.json` — `final_state_hash` is `null`; the first implementation writes the
  hash of the final state there, and from then on it is the golden value.

The generator never overwrites those fields (it writes `null` for them), so edit them in the
JSON and keep the generator's `TO BE FILLED` markers in place.

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

## Notes for the test author

- The fixtures state the *physics*, not the API. Wire them to whatever the simulation entry
  point turns out to be; the point is that the numbers do not move.
- `determinism_golden.json` must be run on every platform you ship. A mismatch between
  x86-64 and wasm is almost always a transcendental function that leaked into the core
  (doc §11.4), not an arithmetic difference.
- `pocket_drop.json` deliberately leaves the rattle outcome to the implementation: once the
  jaw model is in place, add the observed outcome as a golden value so that a change in the
  jaw parameters shows up as a test failure rather than as a silently different game.
