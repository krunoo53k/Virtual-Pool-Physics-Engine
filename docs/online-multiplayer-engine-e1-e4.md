# E1–E4 — engine-side online multiplayer hand-off

This is the engine half of the shared contract. The client and the server
consume the same crates; no GDScript judge needs to survive parity.

## E1 — `pool-rules` (rlib)

A verbatim Rust port of the client's `EightBallRules`, `MatchJudge`,
`RulingApply` and `RackModel`:

* `judge` / `judge_and_announce(&Outcome, &ShotParams, &[ShotEvent], &MatchState)`;
* `Judgement` with the seven fields and `announcements()` yielding the exact
  `name:payload` words in announce order (game over suppresses `turn_changed`);
* the closed reason-word enum `scratch`, `ball_off_table`,
  `wrong_group_first_contact`, `no_contact`, `no_rail_after_contact`,
  `illegal_break`, `miscue`, `incomplete_shot`, `""` — in that priority;
* `_facts` reduction with strict `time > contact_t` for rail-after-contact,
  deduped `driven`, non-deduped `pots`, cue-off-table as a scratch, and
  `pocket-drop` as the only pot;
* `apply_ruling`, `RackState::{after_shot, rerack, respot_cue}` with the client's
  head spot in the core frame `[0.635, 0.6335, 0.028575]`.

The canonical fixtures live here at `tests/fixtures/rules/eight_ball_fixtures.json`
(copied byte-for-byte from the client), and `pool-rules/tests/fixtures.rs`
replays all 54 shots, asserting every Judgement field and the exact signals.

## E2 — `pool-intent` (rlib, not part of `pool-godot`)

* `cue_direction(level_aim_deg, elevation_deg)` — libm trig, aim normalised
  mod 360, core-frame unit vector; the contact sign flip lives here too;
* `shot_input(...)` — including pocket-state recovery for a ball handed over
  on a pocket shelf;
* `outcome_from_trace(...)` — the core index → ball-number mapping and typed
  events/outcome, so output-side logic cannot drift;
* `CueTuning` — speed `[0.15, 9.0]`, elevation `[0, 60]`, inclusive edges,
  radial `hypot ≤ R` wire tip check where `R` is the ball radius. A tip outside
  the *miscue limit* (`R·μ/√(1+μ²)`) but inside the ball is legal on the wire
  and becomes a core-reported foul; the two bounds are deliberately separate;
* `canary_hex()` — the three-shot startup canary.

`pool-core/tests/portability.rs` now scans `pool-intent/src` with the same
platform-transcendental ban it already applied to the core.

## E3 — `pool-godot` f64 entry points

All existing `f32`-facing `#[func]` methods are unchanged: `simulate`,
`simulate_with_rotations`, `sample`, `sample_rotation`, `get_final_state`, … .
New methods:

| Method | Input / output |
|---|---|
| `PoolCore.simulate_from_intent_f64` | six f64 scalars + flat core-frame `PackedFloat64Array` positions/numbers/rotations → `PoolShot` |
| `PoolCore.make_cue_direction_f64` | flat core-frame f64 triple |
| `PoolCore.preview_shot_json` | JSON → shared outcome + judge + announcements |
| `PoolCore.judge_json` / `PoolCore.outcome_from_trace_json` | the same request, narrowed to the judge / the outcome mapping |
| `PoolCore.get_build_info` | JSON: `core`/`rules`/`intent` revs from `compat.json`, `target`, `canary`, `model`; `git_rev` is the build HEAD and informational |
| `PoolShot.sample_rotation_f64` | flat core-frame f64 triples |
| `PoolShot.get_final_positions_f64` | flat core-frame f64 triples |
| `PoolShot.get_final_rotations_f64` | flat core-frame f64 triples from `trace.sample_rotation(duration)` — the final carry, not the incoming orientation |

The hotseat `f32` path has a sunset date; the two paths are intentionally
*different shots* for the same nominal parameters (the `f32` one quantises the
direction and the rack), which is why they keep separate pins. `frame::cue_direction`
keeps the legacy bit-for-bit behaviour for wrapped aims; only the online path
normalises the aim mod 360.

## E4 — pins

Unmoved: `c399229b25faae5a` (old break), `cf1ce48907739066` (fixture),
`1087065d38976a3e` (raised cue).

New, in `pool-godot/tests/determinism_v1.rs`:

| Pin | Value |
|---|---|
| Startup three-shot canary | `c8ee370c4195017f` |
| Three-shot final hashes | `51817b37b13c4587`, `9a1017d7cdcb2235`, `2b0dedb66711845a` |
| Three-shot path hashes | `0b00bcad82ab5257`, `d14b69b2264cefeb`, `c0c9b496bc03b9e0` |
| Three-shot carried rotations | `b4a68d2f572bb938`, `93d6bd0edd43cec1`, `e42750c204660cc4` |
| Random f64 bit-pattern sequence | `d9e504e33fb95cb4` |

The three-shot canary includes final state hashes, carried rotations and a
mid-shot path sample, so it catches the two things the state hash excludes or
cannot see: rotation carry (excluded by design, a rendering fact) and
path-shaping differences. The state hash was confirmed to include both pocket
state and off-table state, and to exclude `BallState.rotation`.

Model version for the handshake: **1.10** in source. The deployed `.so` that was
checked first reports **1.9**; rebuild before parity work (see E0 record). The
model version is an **opaque string** compared with exact equality — never sort
it or parse it as a float (`"1.10" < "1.9"` in both readings). The canary
definition (sequence, hashed fields, current value) is pinned in
[`online-multiplayer-e0.md`](online-multiplayer-e0.md#4-the-determinism-canary-definition-pinned-here).

Carried rotations do **not** influence physics: `BallState.rotation` is read
only when the path recorder is built. Excluding it from the state hash is safe;
the rotation/path pins protect the rendered hand-off, not the simulation.

Web scope: **out of v1**. `COMMIT` has no trajectory, so a client with no
physics cannot animate a shot from it. The planned v2 route is a standalone
`wasm32-unknown-unknown` module of `pool-core` + `pool-intent`, not gdext-web.

### Did the corpus catch 1.10? (experiment)

The original 72 shots of the corpus were replayed against the pre-change engine at `e2faf8a`
(model **1.9**) in a worktree at `/tmp/opencode/v19`, and against the current
1.10 engine, using the **same** exact-f64 direction conversion in both.
Result: **17 of 72 shot hashes differ**, 0 shots fault; replaying with the
current engine reproduced all 72 recorded hashes exactly. The differing shots
are rails-15; spin-02/-10/-12/-14/-15; jumps-04/-05/-06/-08; pockets-02/-14;
off_table-03/-04/-08/-09; carry-01. All 17 are among the 19 shots whose events
touch the shelf (`pocket-enter`, `pocket-drop` or `pocket-exit`).

Important methodological note: comparing the old bridge's `Vector3`-returning
`cue_direction` (f32 components) against the new f64 path changes **66 of 72**
hashes for a conversion reason, not a physics reason. The experiment must hold
the intent conversion fixed; that is exactly what `pool-intent` provides.

The original corpus has no `HungUp` shot and only two `pocket-exit` shots, so
coverage was extended (shelf hand-off, jaw rattle, hung-up, miscue, near-touching
and faulted shots) before treating the corpus as the safety net.

### Corpus/model-version guard

`.github/scripts/check_corpus_guard.py` compares the hashes of the shots listed
in `tests/fixtures/corpus_manifest.json`'s `baseline_hashes` with the current
corpus. An existing hash change requires a model bump **and cannot be waived by
a justification**; a model bump without a hash change requires a written
justification in the manifest. Adding shots is
coverage, not a physics change: it only requires updating the manifest's
`corpus_sha256`. The guard was tested by tampering with a shot hash (fails) and
by adding a new shot (only the file digest needs updating). The corpus is self-generated: it detects change, not correctness.

## Dependencies for v1

The workspace crates are path dependencies with matching version requirements
while in development. For release, the three repos pin each other by **git
revision**, not crates.io: `pool-core` → `pool-intent` → `pool-rules` →
`pool-godot` → server rev → client extension rebuild.

`compat.json` at the workspace root is the single source of truth: protocol and
model strings, the three git revs, the canary and the corpus SHA-256. Engine CI
checks it against this workspace and the regenerated corpus, and checks the
canary against the built helper; the client additionally compares its pinned
copy with the extension's runtime `get_build_info()`. The canary and the corpus
catch a wrong pin even when a version string happens to match.

The canonical event digest is `pool_intent::events_digest(&[SimEvent], &[i32])`
(kind word, time bits, ball/rail number, other number, detail byte, in event
order). The corpus, the canary and the server's `COMMIT` all use this one recipe.
