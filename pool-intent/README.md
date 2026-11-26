# pool-intent

The shared, Godot-free side of a shot. `pool-godot` is a `cdylib`, so the server
and the client's display-only preview cannot depend on it; this `rlib` is where
the numbers both sides use actually live.

* `direction` — aim in degrees to a unit direction with `libm` trig, mod-360
  normalised, and the shooter-right → core-negative-x contact sign flip;
* `input` — `shot_input`, including pocket-state recovery from a bare position,
  and the inclusive `CueTuning` bounds with a radial tip check;
* `outcome` — the index → ball-number mapping and typed events/outcome built
  from a trace, so output-side logic cannot drift between the bindings.

Everything here is plain `f64` in the core frame. No Godot types cross this
boundary, and no platform transcendental functions are used.
