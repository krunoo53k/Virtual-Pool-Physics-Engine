# E0 — version fact check, web spike and platform matrix

This is the engine-side fact-check record for the online-multiplayer work. It is
written down because the hand-maintained version strings have already drifted
once: the protocol contract needs a canary as well as a version.

Sections 3–5 were **written after E1–E4** so they could record the outcome of
the spike, the corpus and the pins. Section 1 and 2 are the original E0 work,
done before the port.

## 1. The version fact check (done first)

| Source | Model version | Evidence |
|---|---|---|
| Game README (`Open-Virtual-Pool/README.md:30`) | `1.8` | Printed description of `PoolCore.get_model_version()`. |
| Deployed extension (`Open-Virtual-Pool/bin/libpool_godot.so`) | **`1.9`** | Runtime check: `godot --headless --path Open-Virtual-Pool res://scenes/smoke.tscn` prints `model version: 1.9`; the binary contains the literal `1.9`. SHA-256 `f89a792bc852936f65651e14403735f7b97066f2336f17ac5a630a0ae3144f24`. |
| Engine source at the start of this work (branch `fix/clippy-warnings`) | `1.9` | `pool-core/src/lib.rs:60`. |
| Engine source after `main` was pulled (commit `0352476`) | **`1.10`** | `pool-core/src/lib.rs:60`. Commit `4f7884c` changed pocket-shelf physics and bumped the model. |

The **exact model version of the deployed extension is `1.9`**. The README's
`1.8` is stale, and the deployed binary is itself one physics release behind
the source. The game keeps the library it loaded at startup. The deployed SHA
above is a record, **not a pin** — a rebuilt binary legitimately has a different
hash on every machine.

After pulling `main` and finishing E1–E4, this repo's extension was rebuilt from
a clean target directory:

```text
cargo clean && ./godot-project/build-extension.sh release
sha256 22fc2301cb24f91146e59d1755abc12c182d63b16a3718ce28a5d133053fb78e
godot --headless --path godot-project res://scenes/smoke_test.tscn
  -> model version: 1.10
  -> the hash matches the pinned value (c399229b25faae5a)
```

The rebuilt artifact contains all the new `#[func]` entry points, including
`get_build_info()` (git rev, model, rules, intent, target, canary). The client
repo's `bin/` copy was deliberately left alone: refreshing it is a **separate
commit**, first, because 1.10 changes pocket physics and the client's harness
pins (self-play outcomes, pocket fixtures, hotseat behaviour) may move. Mixing
that refresh into the C1 type refactor would make physics drift and refactor
bugs indistinguishable.

The single golden break hash **`c399229b25faae5a` did not move** across a
model change that altered physics. That is the concrete proof that one pin is
not enough; the canary and the corpus exist because of it. (`cf1ce48907739066`
fixture hash and `1087065d38976a3e` raised-cue hash also stayed put.)

### Version strings are opaque

`"1.10"` sorts **below** `"1.9"` as a string and below `1.9` as a float. The
protocol is safe because it compares model versions with **exact equality**,
never ordering. Treat the model version as an **opaque string**: do not sort,
compare numerically, or parse it as a float. If a numeric comparison is ever
needed, parse it as integer `(major, minor)`.

## 2. gdext / wasm32 feasibility spike

```text
# The obvious target does not build at all.
cargo build --target wasm32-unknown-unknown -p pool-godot
  -> godot-ffi/godot compile_error:
     "Wasm target requires opt-in via `experimental-wasm` Cargo feature"
     "Wasm targets not using Emscripten are not supported."

# With the opt-in features on the Emscripten target, all Rust crates compile;
# only the final link is missing the Emscripten toolchain.
cargo build --target wasm32-unknown-emscripten -p pool-godot \
  --features godot/experimental-wasm,godot/experimental-wasm-nothreads
  -> Compiles godot-ffi, godot-core, godot, pool-core, pool-godot ...
     error: linker `emcc` not found
```

The gdext book's web guide adds the rest: nightly rustc with `-Zbuild-std`,
`emsdk` 3.1.74, `api-custom` bindings, Emscripten side-module linker flags, and
a Godot export with **Extensions Support** (plus Thread Support and
cross-origin-isolation headers, or a second `nothreads` build). gdext calls the
whole path experimental.

The pure-Rust crates need none of that. Verified wasm32 evidence:

```text
cargo test -p pool-core -p pool-intent -p pool-rules --target wasm32-wasip1
  -> pool-core:    46 + 4 + 10 + 12 + 8 + 36 + 3 + 6 + 8 + 15 tests, all ok
  -> pool-intent:  10 unit + 2 corpus tests (81 shots), all ok
  -> pool-rules:   18 unit + 1 canonical-fixture test (54 shots), all ok
     (executed through scripts/wasi-test-runner.mjs, Node WASI, not just --no-run)

cargo check --target wasm32-unknown-unknown -p pool-core -p pool-intent -p pool-rules
  -> Finished (compile-only lane; no test runner on this target)
```

`wasm32-unknown-emscripten` is deliberately **not** a CI target: without
`emcc` it cannot link, and a linked build would still be experimental.

## 3. Platform-matrix decision (from the spike, not assumed)

| Target | v1 status | Evidence / what runs |
|---|---|---|
| Desktop Linux/macOS/Windows (x86_64, aarch64) | **Supported** | Full extension, both hotseat f32 and online f64 paths, corpus and canary; x86_64 corpus run green. |
| Mobile Android / iOS (aarch64) | **Corpus verified on aarch64; native playback needs the gdext mobile build** | aarch64 corpus replayed under `qemu-aarch64-static` with a locally unpacked cross toolchain: 2/2 corpus tests, 1/1 rules fixture, 4/4 core determinism tests passed. The runner and artifacts live in `/tmp/opencode`, not in the repo. CI uses `cross` for the same job. |
| wasm32 in CI (wasm32-wasip1; compile also on wasm32-unknown-unknown) | **Supported determinism lane** | `pool-core`, `pool-rules`, `pool-intent` and the 81-shot corpus **all ran** under Node WASI (counts in §2). |
| Web browser | **Out of scope for v1** | See §4. |

**Web scope.** A `COMMIT` message carries a hash and a final rack, **no
trajectory**. A browser client with no physics can therefore only teleport balls
to the final rack; it cannot animate a shot. Claiming "web uses `COMMIT`
snapshots for prediction" was wrong. For v1, browser play is out of scope; the
`wss://` lobby note and client C5 acceptance are moot until that changes.

The v2 direction is **not** gdext-web. `pool-core` and `pool-intent` already
build on plain `wasm32-unknown-unknown`, so a standalone wasm module of the
shared pipeline is the natural route: the client predicts locally from the six
wire scalars with the same f64 code as the server, and the protocol either
carries a path or has the client reconstruct it from the confirmed params. That
avoids the experimental Emscripten side-module toolchain entirely.

## 4. The determinism canary (definition pinned here)

`HELLO` carries the hash of a fixed three-shot sequence, computed on both sides
from the same shared code (`pool-intent::canary_hex()`):

1. the engine smoke rack (cue at `(0.25, 0.5625, r)`, 15 object balls in a 4×4
   grid with one missing), fresh rotations;
2. three strokes, in order:
   `(speed, aim_deg, raise_deg, tip_right, tip_up)` =
   `(8.0, 90.0, 0.0, 0.0, 0.0)`,
   `(6.5, 105.0, 0.0, 0.006, 0.004)`,
   `(7.5, 78.0, 8.0, -0.005, -0.003)`;
3. after each stroke: potted/off-table balls are dropped, survivors keep the
   final rotation from `trace.sample_rotation(duration)`, and a scratched cue
   returns to the head spot with zero rotation;
4. hashed, in order, per stroke: the final state hash, the canonical event
   digest (`pool_intent::events_digest`), every component of the final carried
   rotations as raw f64 bits, and every component of the path sampled at half
   the shot duration.

Current value: **`c8ee370c4195017f`**. It covers exactly what the state hash
cannot see (rotation carry) and what it cannot detect for untouched shots
(event differences and path shaping).

## 5. Corpus design and its limits

The corpus lives in `tests/fixtures/corpus.json`, generated by
`pool-cli corpus`, replayed by `pool-intent/tests/corpus.rs`. It is a
self-play sequence: shot *n* starts from shot *n−1*'s final balls, carrying the
final rotations of every ball ([§11.2's hand-off](../docs/virtual-pool-physics.md#112-two-architectures-stepped-vs-event-driven)),
with potted/off-table balls removed and a scratched cue re-spotted. Every shot
stores the exact f64 parameters (decimal **and** hex bit patterns), the rack,
the full event sequence plus a canonical digest, the final rotations of every
ball, the final hash and the duration.

`pool-cli corpus` uses no RNG and no wall clock. The current file holds **81
shots** (schema `pool-corpus/2`): rails 16, spin 16, jumps 12, pockets 15,
off_table 10, carry 3, shelf 2, jaw 2, hung_up 1, miscue 1, edge 2, fault 1.
The shelf/jaw/hung-up shots place a ball past the mouth or drop line, rattle in
the jaws or stop on the shelf; `miscue-01` uses a tip outside the miscue disc;
`edge-01`/`edge-02` are touching and near-touching balls; `fault-01` lowers
`max_events` to 8 and reaches `GamePhase::Faulted`. Each record stores the six
intent scalars in decimal **and** as hex f64 bit patterns, a canonical digest of
the complete event sequence (kind, time bits, ball/rail number, detail), and
the final rotations of every ball, bit-exact. All 81 replay on x86_64, aarch64
and wasm32-wasip1. SHA-256 of the file is recorded in
`tests/fixtures/corpus_manifest.json`.

**Do carried rotations influence physics? No.** `BallState.rotation` is read
only when the path recorder is created; the simulation never steps it. Excluding
it from the state hash is therefore safe. A rotation drift is display-only, and
the rotation/path pins protect the renderer hand-off and the path, not the
simulation.

**Godot float parsing, checked.** A headless probe on Godot 4.7 showed
`JSON.parse_string` is correctly rounded for the probed values (`0.1234567890123456789`,
`-1.2345678901234567e-300`, `9007199254740993.0`, `0.028575` all compare equal
to the corresponding GDScript literals). But `str(float)` prints only about 14
significant digits. A client must therefore not re-serialise corpus or wire
values with `str()`; the corpus stores hex bit patterns next to the decimals so
a parser error cannot masquerade as a physics divergence.

**The corpus is self-generated**: it proves *change*, not *correctness*. A wrong
but stable physics model produces a stable corpus. The guard in
`.github/scripts/check_corpus_guard.py` closes the version-drift half of that:
if an existing shot's hash changes, `MODEL_VERSION` must change; if the model
changes without a hash change, a written justification is required in
`tests/fixtures/corpus_manifest.json`.

The 1.9 → 1.10 detection experiment is recorded in
`docs/online-multiplayer-engine-e1-e4.md`.
