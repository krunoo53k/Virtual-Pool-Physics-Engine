# Virtual Pool — Physics Research Document
## Deterministic 8-Ball Pool Physics Model for Rust Backend

> **Version:** 1.8
> **Date:** 2026-10-23
> **Scope:** 8-Ball pool physics for a 3D deterministic simulation. Covers ball-cloth, ball-ball, ball-cushion, ball-air, ball-slate, ball-cue and ball-pocket interactions. Includes jump shots, ball-off-table behavior, and deterministic simulation architecture.
>
> **Revision note (1.1):** corrected the sliding-state spin sign (§3.3), the ball-ball normal and
> tangential impulse (§4.4.5), the cushion geometry and the mechanism of the "pop" (§5.1, §8.1),
> the cue-strike spin and squirt treatment (§6), the jump-shot criterion and drag figures (§7),
> and the determinism guidance (§11.4). Sources were re-checked against the primary papers.
>
> **Revision note (1.2):** added ball–pocket interactions (§9) and the validation fixtures
> (§14, with `tests/fixtures/`); added the tuning-parameter defaults and the calibration order
> (§13.3); corrected the cushion's vertical impulse analysis, which now accounts for the
> Coulomb cap on the friction impulse (§5.1, §8.1) — the flip threshold is `ε_c ≈ μ_cushion`,
> not the value implied by the sticking branch.
>
> **Revision note (1.3):** the cushion contact height is now **derived from the nose geometry**
> rather than being a tuning parameter (§5.1). The table config carries the nose apex height and
> the nose arc radius; the contact normal is the line of centres, so `ε` and `θ` follow. For the
> WPA reference profile this gives `ε = 0.039 R`, below the flip threshold, so the standard case
> is the cushion launching the ball into the familiar few-millimetre hop. The TOI specification
> was updated to match (§11.3).
>
> **Revision note (1.4) — corrects 1.3.** The 1.3 reading of "nose height" was wrong: it took
> the specified height to be the *apex* of the nose bulge, which put the contact at 29.7 mm and
> made the specified nose height have no effect on the rebound at all — and the WPA specifies
> 0.635 D precisely because the height changes the rebound. The specification and the
> manufacturers define the nose height as the height at which the cushion *makes contact*
> (36.3 mm), so `ε = h_nose - R = 0.270 R` and the contact normal is tilted 15.7°. The model
> now reproduces Dr. Dave's account of all three reference heights (0.5 D launches the ball,
> 0.635 D presses it into the slate, 0.7 D drives it down hard), which is the check the 1.3
> reading failed. The standard case is therefore "pressed into the slate, hop from the slate
> rebound" again. Also: the rail is now treated as three surfaces (nose rubber, rail structure
> above it, clear of the rail) with the band between the nose and the rail top named (§11.3),
> and a spurious `cos θ` was removed from the vertical-impulse formula.
>
> **Revision note (1.5) — the amendments that playing the game asked for.** Four points, all
> of them found by building the game on top of this model rather than by reading it:
>
> * **The spin decay stops at zero** (§3.2, §3.4). The decay is linear, and a linear decay has
>   to be told where zero is: it decays *towards* zero and stops there. A decay that simply
>   subtracted would carry the spin past zero into a backspin, and the backspin would then
>   grow instead of dying - so a ball never came to rest and every shot ran to the time limit.
> * **The nose's height is a knife edge** (§11.3). A ball on the bed touches the nose at
>   *exactly* the nose's height, so the test that rejects a contact below the nose sits on
>   that boundary; with an ulp of tolerance, a ball whose height came from a single-precision
>   number was rejected and went *through* the rail. The tolerance is a length - a micron.
> * **The squirt comes from the sideways offset** (§6.1), not from the whole impact
>   parameter: a draw or a follow shot goes straight down the aim line.
> * **The render time is clamped to the shot** (§11.2): a frame drawn a little past the end
>   of the shot gets the end of the shot, not the origin of the table.
>
> **Revision note (1.6) — the corrections a review of the document asked for.** The
> derivations were re-checked and they hold; the errors were in the statements around them.
>
> * **The cushion's normal impulse is not skipped when the slip is zero** (§5.3). The code
>   returned early on `slip <= 0.0`, which dropped the normal impulse and let the ball
>   through the rail. The normal impulse is applied whenever the ball is approaching the
>   rail; only the tangential part is skipped when there is no slip.
> * **The cushion's impulse formulas assume a ball on the bed** (§5.1). The contact speed
>   `c` and the slip `s_x` are written for `v_z = 0`; an airborne ball at the rail needs the
>   `v_z` terms, and the core carries them.
> * **The spurious `cos θ` is gone from the formula *and* from the comment** (§5.3): the
>   outgoing vertical speed is `(1 + e)·v_n·(μ·cos θ − sin θ)`.
> * **A plane is a plane** (§11.3): the contact condition against the inclined nose face
>   is linear in position, so the time of impact is a quadratic for an airborne ball too.
>   Only a curved surface (the rounded nose tip) or a ball–ball contact gives a quartic.
> * **Air drag is dropped** (§7): it is 0.04 m/s² against 9.81, and a quadratic drag law
>   would take the closed-form ballistic arc away from the event-driven core.
> * **The handedness of the cue strike is stated once, in the input model** (§6.2, §12.3):
>   `+ŷ` is the shooter's *left*, and a positive `contact_offset_x` is a strike on the left.
> * **The cue strike's offset case is written out** (§6.2): the centre-hit formula is the
>   *longitudinal* response; an offset hit also has a *transverse* response, in which the
>   ball's effective mass is `m/(1 + 5b²/2R²)` — `m/1.625 = 0.615·m` at `b = 0.5R` (1.8
>   corrected the report of this: the 1.625 is the divisor, not the multiplier). The full
>   cue mass belongs to the longitudinal response and the ~12 g effective end mass to the
>   transverse one. The two are not in conflict; the document now says which is which.
> * **The friction between two balls is two-dimensional** (§4.4.5): the slip at a
>   ball–ball contact has a vertical component, the friction acts along the slip in the
>   tangent plane, and that vertical component is what carries the vertical effects of
>   §4.5 — the same treatment the cushion gets in §5.1.
> * **The friction between two balls falls with the sliding speed** (§4.4.5, §14.3):
>   TP A.14 fits `μ(v_rel) = 0.009951 + 0.108·e^(−0.77·v_rel)`, which is why the measured
>   throw is larger on slow shots. That is a cheaper way to close the throw gap than
>   coupling the cloth friction into the impulse, as §14.3 used to say was the only way.
> * **The pocket's facing angles are marked as a reading, not a specification** (§9.1): the
>   WPA gives the cut angles but no drawing and no reference lines, so the derivation from
>   the cut angle to the facing angle is stated as an assumption and exposed in the table
>   config. The WPA's side-pocket cut angle is 104° (+1); the 103° (±2°) in §2.3 is the
>   BCA's.
> * **The stale references are fixed** (§2.2, §5.2, §9.1, §9.2, §12.4), and the time limit
>   is no longer called a determinism requirement (§12.4).
> * **The Zeno guard is given a number** (§8.2): the slate bounce settles below
>   `v_z = 0.02 m/s`, and the 30 s cap reports an unsettled shot rather than truncating it
>   silently.
> * **The three-height check is a fit, not a validation** (§5.1, §8.1): the hop magnitudes
>   depend on treating the cushion impulse and the slate rebound as sequential, and
>   Mathavan treats the table contact as simultaneous. The 3.8 mm hop is a model output
>   until `cushion_rebound` carries measured video.
>
> **Revision note (1.7) — what a second review of v1.6 asked for.** Four of the points
> were about the honesty of the document rather than about its arithmetic.
>
> * **The break-speed lift is an extrapolation, and it says so** (§4.5). TP A.14's friction
>   law is fitted at 1–7 mph and a break slips at 8–10 m/s, past the end of the data; at a
>   10 m/s slip the law gives μ ≈ 0.008 and a lift of 0.08 m/s, where Kim's constant-μ
>   estimate gives 0.5 m/s. The two cannot both be right, the model is probably the one
>   that is low, and no published measurement of the cue ball's hop after a break-speed
>   collision was found. The fixture records one as `TO BE CALIBRATED` (§14).
> * **The throw fixture compares like with like** (§14.1). It used to carry one model row
>   per cut angle, at a fixed 2 m/s, beside three columns measured at 1, 3 and 7 mph — so
>   the model row was really being compared against the *fast* column, and the residual
>   looked like an overshoot growing with the cut angle. Every row is now at one of TP
>   A.14's own speeds.
> * **The measured throw columns are not a calibration target, and the document now
>   proves it** (§14.1, §14.3). The three columns at a 30° cut cannot be fitted by *any*
>   law of the form `A + B·e^(−K·u)`, for any floor term and any decay constant: the
>   implied coefficients are 0.0813, 0.0796 and 0.0507 at slip speeds of 0.224, 0.671 and
>   1.565 m/s, and for such a law the ratio of the successive differences is bounded
>   below by `(u₂−u₁)/(u₃−u₂) = 0.50`, while the measured ratio is 0.061. There is
>   nothing to fit, so the residual is a recorded deviation and no calibration is layered
>   on top of it. TP A.14's own fit is the authority.
> * **`hypot` is not arithmetic** (§11.4). It is not correctly rounded and not specified by
>   IEEE-754, and the core used the platform's to decide whether a ball is moving — so
>   two platforms could disagree in the last bit about whether a ball has stopped, and
>   the event sequence with it. The core calls `libm::hypot` now, and a test reads the
>   core's own source and fails if any platform transcendental appears in it.
> * **The bed is snapped at the input boundary** (§3.1). A ball at rest is on the bed, so
>   its centre is at exactly one radius, and that is where the `f32` positions of a game
>   stop being allowed to say otherwise. The bed tolerance is also checked against the
>   slate's settle threshold (§8.2): a ball with a vertical speed far below the threshold
>   is absorbed on its first bounce, not turned into a chain of near-zero events.
> * **What the fixtures do not check is now written down** (§14.5): the cushion hop, the
>   break-speed lift, the jaw parameters and the squirt curve are the four things the
>   model has not been checked against reality on.
> * **The golden hash is confirmed on two targets** (§14.4): x86-64 and
>   i686-unknown-linux-gnu — a different word size and a different floating-point path —
>   agree bit for bit. The wasm32 baseline has *not* been established: there is no wasm
>   toolchain here, and the document says so rather than implying a portability that has
>   not been demonstrated.
>
> **Revision note (1.8) — what a third review of v1.7 asked for.** The throw residual
> shrank because the model was wrong, not because the data was.
>
> * **The ball friction law is used unscaled** (§4.4.5, §13.2, §14.3). The config shipped
>   `μ_ball = 0.05` "at a 1 m/s slip", 16.6% below TP A.14's own fit value there
>   (0.05996), and §4.4.5 normalised the law onto it — so the whole law was rescaled by
>   0.83, which manufactured about 0.7° of the throw residual that was then recorded as
>   untouchable. The config carries the fit's own value (0.06) now, and against the
>   measured throw the model sits at most 0.5° below, worst at the fast speeds.
> * **The "no law can fit the measured columns" argument is retracted** (§14.1, §14.3). It
>   turned measured throw into implied friction with `μ = tan(throw)`, which holds only in
>   the sliding regime; the slow column at a 30° cut sits at the 4.83° sticking limit,
>   where the throw does not depend on the friction coefficient at all, so it yields only
>   a *lower* bound — which TP A.14's fit (0.101 there) clears. With the sticking branch
>   accounted for, the law reproduces all three columns to within half a degree, and the
>   conclusion that the columns were not measured under one set of conditions does not
>   follow.
> * **The cue impulse depends on the impact parameter** (§6.2). It responds against the
>   ball's effective mass at the contact point, `m_eff = m/(1 + 5b²/2R²)`, so
>   `v_ball = (1 + e)·V/(1 + m/M + 5b²/2R²)` — a tip at half a radius leaves the ball 31%
>   slower than a centre hit, which is why firm draw is hard. §6.2 had claimed the
>   impulse does not depend on `b`, and reported `m_eff` as `1.625·m` at `b = 0.5R`,
>   inverting its own formula: 1.625 is the divisor, and `m_eff = 0.615·m`. The 1.6 note
>   carried the same slip and is corrected where it reports what 1.6 established.
> * **The pocket facing defaults to the reading that matches a real jaw** (§9.1). The
>   default derivation is now the rail-line reading — 7° off the centreline at a corner,
>   14° at a side pocket — instead of the throat reading, whose 38° a side would be a 76°
>   V that no real jaw has. The side pocket's 14° replaces a 13° that was the BCA's 103°
>   run through the rule rather than the WPA's 104°. The facing angle stays a config
>   parameter, so a measured table can still be matched directly.
> * **Three smaller corrections** (§4.4.5, §5.1). The component-wise sticking branch
>   carries its `v_z` term (it was written for a ball on the bed); the slip `u_t` of §4.4.5
>   is labelled along the horizontal tangent, not "along the rail" — there is no rail in a
>   ball–ball contact; and §4.5's worked lift example uses the current μ.
> * **The i686 confirmation says less than v1.7 claimed** (§14.4). Rust's
>   `i686-unknown-linux-gnu` baseline includes SSE2, so that second target proves
>   *word-size* portability, not a different floating-point path — x87's 80-bit
>   intermediates are the `i586` target, which has not been run. A genuinely different
>   arithmetic path is `aarch64` under qemu, close to the shipping targets; wasm is strict
>   IEEE-754 like SSE2 and is not a different arithmetic path either.
> * **The fixed-timestep determinism claim is scoped to the stepped architecture**
>   (§11.1). §12.4 already said a fixed timestep does not give determinism in the
>   event-driven core; §11.1 now says which architecture its bullets describe.
> * **The portability scan is recursive** (§11.4). It covers every `.rs` file under
>   `src/`, at any depth, so a module moved into a subdirectory cannot slip out of the
>   check that made the ban list — which includes `powi`, `atan2`, `asin`, `acos`, `tan`,
>   `ln` and `cbrt` — apply to everything the core runs.

---

## Table of Contents

1. [Overview & Design Goals](#1-overview--design-goals)
2. [Standard Equipment Specifications](#2-standard-equipment-specifications)
3. [Ball-Cloth Interactions](#3-ball-cloth-interactions)
4. [Ball-Ball Collisions](#4-ball-ball-collisions)
5. [Ball-Cushion Collisions](#5-ball-cushion-collisions)
6. [Ball-Cue Interactions](#6-ball-cue-interactions)
7. [Jump Shots & Ball-Air Interactions](#7-jump-shots--ball-air-interactions)
8. [Ball-Slate Interactions](#8-ball-slate-interactions)
9. [Ball-Pocket Interactions](#9-ball-pocket-interactions)
10. [Balls Leaving the Table](#10-balls-leaving-the-table)
11. [Deterministic Simulation Architecture](#11-deterministic-simulation-architecture)
12. [State Representation & Input Model](#12-state-representation--input-model)
13. [Summary of Recommended Model](#13-summary-of-recommended-model)
14. [Validation Fixtures](#14-validation-fixtures)
15. [References](#15-references)

---

## 1. Overview & Design Goals

### 1.1 Goals

- **Realistic but doable:** Model all physically significant phenomena in pocket billiards using analytical/semi-analytical equations. No need for full finite-element soft-body simulation.
- **Deterministic:** Same inputs → same outputs, regardless of frame rate or platform.
- **3D:** Must support jump shots, ball elevation, ball-slate bounces, and balls leaving the table.
- **8-Ball focused:** Six pockets, 16 balls (cue + 15 object), standard 9×4.5 ft table.
- **Rust backend:** Physics computed in Rust; inputs are ball positions, cue speed, cue direction
  (unit vector), and the contact offset on the cue ball (english).

### 1.2 Core Physics Domains

The physics of pool decomposes into five interaction types (Kiefl, 2020):

1. **Ball-cloth:** How balls slide, roll, spin, and decelerate on the table surface.
2. **Ball-ball:** How balls collide with each other.
3. **Ball-cushion:** How balls bounce off rails.
4. **Ball-air:** How balls behave when airborne (jump shots, ball-off-table).
5. **Ball-slate:** How balls bounce when they hit the table surface after being popped into the air.

Each domain has well-studied analytical models.

---

## 2. Standard Equipment Specifications

### 2.1 Balls (WPA / BCA Standard)

| Property | Value | Source |
|---|---|---|
| Diameter | 2.25 in (57.15 mm) | Dr. Dave Pool Info, BCA |
| Radius (R) | 1.125 in (28.575 mm) | Derived |
| Mass | 5.5–6 oz (156–170 g); use 6 oz (170 g) | WPA/BCA; Dr. Dave Pool Info |
| Moment of Inertia | I = (2/5) mR² (solid sphere) | Standard physics, Dr. Dave |
| Ball-ball Coefficient of Restitution (e) | 0.92–0.98 (typical ~0.95) | Dr. Dave Pool Info; arXiv 2402.13258 |
| Ball-ball friction coefficient (μ_ball) | 0.03–0.08 | Dr. Dave Pool Info; arXiv 2402.13258 |
| Material | Aramith phenolic resin (HD) | Aramith specifications |

### 2.2 Table (WPA Tournament Standard — 9×4.5 ft)

| Property | Value | Source |
|---|---|---|
| Playing surface (between cushions) | 100" × 50" (2540 × 1270 mm) | WPA, BCA |
| Overall table size | 9 ft × 4.5 ft | WPA |
| Table bed height | 29¼"–31" from floor | WPA |
| Cushion profile | K-66 (BCA standard) | Wikipedia (Billiard table) |
| Cushion nose height | 63.5% of ball diameter ≈ 36.3 mm (1 7/16") | WPA/BCA; Dr. Dave Pool Info |
| Cushion base | 1 3/16" (30 mm) | BCA |
| Cushion contact point above ball centre (ε) | **derived, not specified**: `ε = h_nose − R` = 0.270 R for the WPA profile (§5.1). Han's 0.1–0.2 R is the *carom* range, for carom cushions whose noses sit lower — it is not the pool value | §5.1; Han 2005 (carom) |

### 2.3 Pockets (WPA/BCA)

| Property | Value | Source |
|---|---|---|
| Corner pocket mouth | 4.5"–4.625" (114.3–117.5 mm) | WPA Equipment Specs |
| Side pocket mouth | 5.0"–5.125" (127–130.2 mm) | WPA Equipment Specs |
| Corner pocket cut angle | 142° ± 1° | WPA Equipment Specs; BCA |
| Side pocket cut angle | 104° ± 1° (WPA); 103° ± 2° (BCA) | WPA Equipment Specs; BCA |
| Vertical pocket angle (back draft) | 12°–15° | WPA |
| Corner pocket shelf | 1"–2¼" (WPA); 1⅝"–1⅞" (BCA) | WPA Equipment Specs; BCA |
| Side pocket shelf | 0"–3/8" | WPA Equipment Specs; BCA |
| Pocket radius (slate) | 1/8"–1/4" drop point | BCA |

> **The cut angle is what the WPA specifies; the facing angle is what the model uses.**
> The WPA's wording is: "The angle must be the same on both sides of a pocket entrance.
> The cut angles of the rubber cushion and its wood backing (rail liner) for both sides of
> the corner pocket entrance must be 142 degrees (+1)." It gives no drawing and does not
> say which lines the angle is measured between, so the step from the cut angle to the
> facing angle is a *reading*, not a specification — see §9.1. The table uses the WPA's
> numbers; the BCA's differ slightly on the side pocket and on the corner shelf, and both
> are compliant tables.

### 2.4 Cloth / Cushion Properties

| Property | Value | Source |
|---|---|---|
| Sliding friction (ball-cloth) μ_s | 0.15–0.40 (typical: 0.2) | Dr. Dave Pool Info |
| Rolling friction (ball-cloth) μ_r | 0.005–0.015 | Dr. Dave Pool Info |
| Spinning friction μ_sp | — (phenomenological) | Kiefl, 2020 |
| Spin deceleration rate | 5–15 rad/s² | Dr. Dave Pool Info |
| Cushion coefficient of restitution (e_cushion) | 0.6–0.9 | Dr. Dave Pool Info |
| Cushion sliding friction (μ_cushion) | 0.14 (measured) | Mathavan et al., 2010 |
| Table (slate) coefficient of restitution (e_table) | 0.5–0.7 | Dr. Dave Pool Info |
| Cushion rubber thickness | 1/16"–1/4" (1.6–6.4 mm) | WPA |

**Notation:** `μ_s` always means the **sliding (kinetic)** ball-cloth coefficient. Where static
friction is meant it is written `μ_stat` — the two are not interchangeable (§4.5).

**Mapping the measured spin decay to `μ_sp`:** the equations of §3 use the phenomenological
coefficient `μ_sp`, while the table above lists the measured *decay rate*. They are related by
`dω_z/dt = 5·μ_sp·g/(2R)`, so

```
μ_sp = (2R / (5g)) · (spin decay rate)  →  0.006 … 0.017  for 5–15 rad/s²
```

### 2.5 Cue Stick Properties

| Property | Value | Source |
|---|---|---|
| Cue-ball friction coefficient (μ_cue) | ~0.6 (well-chalked) | Dr. Dave Pool Info |
| Cue-ball COR (leather tip) | 0.71–0.75 | Dr. Dave Pool Info |
| Cue-ball COR (phenolic tip) | 0.81–0.87 | Dr. Dave Pool Info |
| Cue mass | ~16 oz (454 g) typical | General knowledge |
| Cue length | ≥ 40" (1016 mm) | BCA |

---

## 3. Ball-Cloth Interactions

This domain covers how balls move on the table surface. A ball can be in one of four motion states: **stationary, spinning, rolling, or sliding**. The state depends on the relationship between linear velocity **v** and angular velocity **ω**.

### 3.1 State Definition

The key quantity is the **relative velocity** at the point of contact (PoC) between ball and cloth:

```
u(t) = v(t) + R × (k̂ × ω(t))
```

where:
- `v(t)` = linear velocity of the ball's center of mass
- `ω(t)` = angular velocity vector
- `R` = ball radius
- `k̂` = unit vector pointing upward (perpendicular to the table)
- `×` = cross product

The ball is:
- **Rolling** when `u(t) = 0` (no relative motion at PoC)
- **Sliding** when `u(t) ≠ 0`
- **Spinning** when `v(t) = 0` but `ω(t) ≠ 0` (ball rotates in place)
- **Stationary** when `v(t) = 0` and `ω(t) = 0`

> **The bed is at exactly one radius.** A ball that rests on the cloth has its centre at
> `z = R`, and that is a *known* quantity, not a measurement. A game that keeps its balls in
> `f32` puts the centre a fraction of a nanometre below that — and a ball that is a hair
> below the bed and a hair above the ground is in no state at all: it cannot land, because
> the landing time of a ball whose centre is *below* the bed does not exist, and it falls
> through the table and off the world. The core snaps a ball that is within a micron of the
> bed onto it. It is the same knife edge as the nose height of §11.3, one domain over.

### 3.2 Rolling State

**Condition:** `u(t) = 0`, i.e., `v(t) = -R × (k̂ × ω(t))`

In rolling, the ball decelerates due to rolling friction:

```
v(t) = v₀ - μ_r · g · t · v̂₀
```

where `v̂₀` is the unit vector of initial velocity direction.

**Displacement:**
```
r(t) = r₀ + v₀ · t - ½ · μ_r · g · t² · v̂₀
```

**Angular velocity:**
```
ω_xy(t) = k̂ × v(t) / R       (in-plane angular velocity)
ω_z(t)  = ω₀z - (5 · μ_sp · g) / (2R) · t   (spin decay)
```

> **The spin decay stops at zero.** The formula is linear, and a linear decay
> has to be told where zero is: it decays *towards* zero and stops there, so
> `ω_z(t) = sign(ω₀z) · max(0, |ω₀z| - (5 μ_sp g / 2R)·t)`. Without that, a ball
> whose spin reached zero would go on past it into a backspin, and a backspin
> would then *grow* instead of dying - which also means the ball never comes to
> rest, because the state it reaches depends on a spin that never ends.
>
> The spin of a ball that is rolling dies as it rolls, and the segment that
> follows starts with no spin to decay.

**Validity:** `0 ≤ t ≤ |v₀| / (μ_r · g)` — until the ball stops.

**Coordinate system (table frame):**
```
v_x(t) = v₀·cos(φ) - μ_r·g·cos(φ)·t
v_y(t) = v₀·sin(φ) - μ_r·g·sin(φ)·t
ω_x(t) = -|v(t)|/R · sin(φ)
ω_y(t) =  |v(t)|/R · cos(φ)
ω_z(t) = ω₀z - (5·μ_sp·g)/(2R)·t
```

where `φ` is the angle of motion relative to the table's x-axis.

### 3.3 Sliding State

**Condition:** `u(t) ≠ 0`

Sliding is the most complex state because the friction force opposes the *relative velocity* `u(t)`, not just the linear velocity. This creates curved trajectories when spin is present.

**Relative velocity evolution:**
```
u(t) = (u₀ - 7/2 · μ_s · g · t) · û₀
```

where `u₀ = |u(0)|` and `û₀` is the unit vector of initial relative velocity.

**The transition from sliding to rolling occurs when:**
```
t_slide_to_roll = 2/7 · u₀ / (μ_s · g)
```

**Displacement (ball-local frame, initial motion along +x):**
```
r(t) = v₀ · t - ½ · μ_s · g · t² · û₀
```

**Velocity:**
```
v(t) = v₀ - μ_s · g · t · û₀
```

**Angular velocity (in-plane):**
```
ω_xy(t) = ω₀xy + (5 · μ_s · g) / (2R) · t · (k̂ × û₀)
```

> **Sign check (this sign is easy to get wrong).** The friction force on a sliding ball is
> `F = -μ_s·m·g·û₀`, applied at the contact point `r = -R·k̂`. The resulting torque is
> `τ = r × F = R·μ_s·m·g·(k̂ × û₀)`, so `ω̇ = τ/I = +(5·μ_s·g)/(2R)·(k̂ × û₀)`. For a ball sliding
> along +x with no spin (`û₀ = x̂`) this gives `ω_y` growing **positive**, i.e. the ball spins up
> towards forward roll — exactly the rolling condition of §3.2 (`ω_y = +v/R`). A minus sign here
> would spin the ball up *backwards* and would also contradict the `u(t)` equation above
> (`u_x = v_x - R·ω_y` would grow instead of decaying to zero). *(The published Kiefl page carries
> this sign error; the equations here are checked against the torque balance.)*
>
> **Unit test:** integrate the sliding state and assert that `u` reaches zero at exactly
> `t = 2·u₀/(7·μ_s·g)`, and that `ω` at that instant satisfies the rolling condition of §3.2.

**Spin decay (z-component):**
```
ω_z(t) = ω₀z - (5 · μ_sp · g) / (2R) · t
```

> The slip direction `û₀` is constant while the ball slides on a flat bed: `u̇ = -(7/2)·μ_s·g·û₀`
> is anti-parallel to `u`, so `u` neither grows nor rotates. Curved *paths* still occur because
> the component of `v` perpendicular to `û₀` shrinks faster than the component along `û₀`.

**Transition to rolling:** When `t = 2/7 · u₀ / (μ_s · g)`, the ball enters the rolling state. The remaining equations use rolling friction `μ_r` instead of `μ_s`.

### 3.4 Spinning State

**Condition:** `v(t) = 0` but `ω(t) ≠ 0`

The ball rotates in place (typically around the z-axis). A phenomenological friction term causes spin decay:

```
ω_z(t) = ω₀z - (5 · μ_sp · g) / (2R) · t
```

The ball stops spinning when `ω_z(t) = 0`, i.e., at `t = (2R · ω₀z) / (5 · μ_sp · g)`.

> As in §3.2, the decay acts towards zero and stops there: the spin of a ball
> that has died stays dead, and a backspin dies rather than growing.

### 3.5 State Transitions

```
                    ┌─────────────────────────────────┐
                    │         SLIDING STATE           │
                    │  u(t) = v(t) + R(k̂ × ω(t)) ≠ 0 │
                    │  Deceleration: μ_s · g          │
                    └──────────────┬──────────────────┘
                                   │ t = 2/7 · u₀/(μ_s·g)
                                   │
                    ┌──────────────▼──────────────────┐
                    │         ROLLING STATE           │
                    │  u(t) = 0                       │
                    │  Deceleration: μ_r · g          │
                    └──────────────┬──────────────────┘
                                   │ |v(t)| → 0
                                   │
                    ┌──────────────▼──────────────────┐
                    │       STATIONARY STATE          │
                    │  v = 0, ω = 0                   │
                    └─────────────────────────────────┘

                    ┌─────────────────────────────────┐
                    │         SPINNING STATE          │
                    │  v = 0, ω_z ≠ 0                │
                    │  ω_z decay: μ_sp · g            │
                    └──────────────┬──────────────────┘
                                   │ ω_z → 0
                                   │
                    ┌──────────────▼──────────────────┐
                    │       STATIONARY STATE          │
                    └─────────────────────────────────┘
```

**Key insight:** Every cue stroke and every ball-ball collision produces a sliding ball. The ball transitions to rolling after a short sliding phase (typically 0.1–0.5 seconds, depending on speed and spin).

---

## 4. Ball-Ball Collisions

### 4.1 Model Choice: Elastic, Instantaneous, with Friction

Based on the research (arXiv 2402.13258, Wallace & Schroeder 1988, Peskin 2020), the best practical model is:

- **Nearly elastic:** COR e ≈ 0.95 (range 0.92–0.98)
- **Nearly instantaneous:** Contact time ≈ 250–300 μs
- **Low friction:** μ_ball ≈ 0.03–0.08

The collision time is so short that gravity and table friction are negligible *during* the collision. However, the table's role *during* collision is significant because the normal force can change dramatically due to friction between the balls.

### 4.2 Frictionless Elastic Collision (Baseline)

For two identical balls (mass m, radius R), with ball A moving at velocity `v₀` striking stationary ball B:

**Line of centers:** Unit vector `n̂` from center of A to center of B at contact.

**Angle of cut:** `α` = angle between `v₀` and `n̂`.

**Post-collision velocities (frictionless, elastic):**
```
v_A = v₀ · sin(α) · t̂_A     (perpendicular to line of centers)
v_B = v₀ · cos(α) · n̂        (along line of centers)
```

where `t̂_A` is the tangent direction perpendicular to `n̂`.

**Key result:** The two balls emerge at 90° to each other (for perfectly elastic, frictionless collision). With e ≈ 0.95, the angle is ~85°–89°.

### 4.3 Both Balls Moving

Transform to the frame of ball B:
```
v'_A = v_A - v_B    (relative velocity before collision)
v'_B = 0
```

Resolve collision using the stationary-ball formulas, then transform back:
```
v_A(final) = v'_A(final) + v_B(initial)
v_B(final) = v'_B(final) + v_B(initial)
```

### 4.4 With Friction: Throw Effects

When friction between balls is included, two phenomena occur:

#### 4.4.1 Cut-Induced Throw (CIT)

Due to the cut angle, there is relative sliding between the ball surfaces at the contact point. Friction during this sliding deflects the object ball slightly off the line of centers.

- CIT throws the object ball in the direction of the cue ball's post-collision motion.
- Maximum throw occurs at cut angles of ~30°–35° (half-ball hit) (Dr. Dave Pool Info, TP A-14).
- Typical throw: ~1.5° at a 10° cut, rising to ~3°–5.5° at 30°–40° cuts on slow shots
  (Dr. Dave Pool Info, TP A-14).

#### 4.4.2 Spin-Induced Throw (SIT) / Gear Effect

English (sidespin) creates additional relative sliding at the contact point:

- Right english → object ball thrown to the left.
- Left english → object ball thrown to the right.
- "Gearing" english (outside english that exactly cancels sliding) eliminates throw entirely — the cue ball "rolls" on the object ball like gears meshing.

#### 4.4.5 Mathematical Model with Friction

Resolve the collision in the frame of the line of centres `n̂` (from A's centre to B's centre)
and the **tangent plane** at the contact, which is spanned by the horizontal tangent `t̂` and
the vertical `ẑ`. Let `v_n = v_A·n̂` (positive while A approaches B).
Assume equal masses `m` and that B is initially at rest.

**Normal component (line of centres):**

For two equal masses, momentum conservation along `n̂` plus the restitution law give:

```
J_n = ((1 + e)/2) · m · v_n        (normal impulse, > 0, applied to B along n̂)

v_A,n′ = ((1 - e)/2) · v_n
v_B,n′ = ((1 + e)/2) · v_n
```

Sanity check: `v_A,n′ + v_B,n′ = v_n` (momentum conserved) and for a head-on hit
(`v_A,t = 0`) at `e = 0.95` the cue ball keeps only `0.025·v_n` and the object ball takes
`0.975·v_n`. The commonly written shortcut `v_A,n′ = -e·v_A,n` is **wrong** — it describes a
ball bouncing off an immovable wall, it does not conserve momentum for two balls, and it would
send the cue ball backwards at nearly full speed on a head-on shot.

**Tangential component (the tangent plane):**

Two balls touch at a *point*, not along a line, so the slip at the contact has two components:
the horizontal one along `t̂` and the vertical one along `ẑ`. The relative slip velocity at
the contact point (including the contribution of both balls' spin) is

```
u_t = (v_A + ω_A × r_A)·t̂ - (v_B + ω_B × r_B)·t̂      (slip along the horizontal tangent)
u_z = (v_A + ω_A × r_A)·ẑ - (v_B + ω_B × r_B)·ẑ      (slip in the vertical)
u   = sqrt(u_t² + u_z²)                              (the slip speed)
```

with `r_A = R·n̂` and `r_B = -R·n̂` the vectors from each centre to the contact point. An impulse
`J_t` applied at the contact changes the relative slip by `7·J_t/m` (each ball's
contact-point velocity changes by `(7/2)·J_t/m`, because `1/m + R²/I = 7/(2m)` for
`I = (2/5)mR²`). Hence the impulse that exactly stops the slip is

```
J_t(stick) = m · u / 7
```

and the Coulomb limit caps it at

```
|J_t| ≤ μ_ball(u) · J_n
```

so

```
J_t = min( m·u/7 , μ_ball(u)·J_n )
```

applied along the slip direction `û = (u_t·t̂ + u_z·ẑ)/u` as `−J_t·û` on A and `+J_t·û` on B
(the friction always opposes the relative slip), and then

```
v_A′ = v_A − (J_n·n̂ + J_t·û)/m        v_B′ = v_B + (J_n·n̂ + J_t·û)/m
ω_A′ = ω_A + (r_A × (−J_n·n̂ − J_t·û))/I
ω_B′ = ω_B + (r_B × ( J_n·n̂ + J_t·û))/I
```

> **The factor is 1/7, not 2/7.** `2/7` is the *ball-on-a-fixed-plane* constant (there the slip
> changes by `(7/2)·J/m`, not `7·J/m`). The 1/7 form is confirmed by Dr. Dave's published throw
> equation (TP A-14), whose sticking branch is `min(…, 1/7)·(v·sin φ - R·ω_z)`.
> The spin update is not optional: throw, gear effect and spin transfer are all consequences of
> it.

> **The friction between the balls is not constant.** A constant `μ_ball` cannot produce a
> throw that depends on the shot speed, and the measurements do (TP A-14): 2.9° at a 30° cut
> on a fast shot, 4.65° on a slow one. Alciatore fits Marlow's data with
>
> ```
> μ_ball(v_rel) = 0.009951 + 0.108·e^(−0.77·v_rel)      [v_rel in m/s]
> ```
>
> which is 0.118 at a standstill and 0.060 at `v_rel = 1 m/s`. The config's `μ_ball` is the
> value at that reference speed, so the law used here is
>
> ```
> μ_ball(u) = μ_ball · (0.009951 + 0.108·e^(−0.77·u)) / (0.009951 + 0.108·e^(−0.77))
> ```
>
> A slow shot slips slowly, so it gets a *larger* friction coefficient and a larger throw —
> which is the measured behaviour. The config's default is the fit's own 0.060, so the
> normalisation is the rounding of the config value and the law in force is TP A.14's fit,
> **unscaled**. (An earlier default of 0.05 rescaled the whole law by 0.83 and manufactured
> about 0.7° of the residual §14.3 used to record.) §14.3 records what is left over.

> **The slip is two-dimensional, and that is not a detail.** With the friction restricted to
> the horizontal tangent (the older form of this section) the vertical component of the slip
> is thrown away, and with it the vertical effects of §4.5: the cue ball is lifted and the
> object ball is pressed into the cloth because the *vertical* component of the friction
> impulse acts on them. For a rolling cue ball the contact point at the front of the ball
> moves *downwards* — it is the leading edge of a rolling sphere — so the friction lifts the
> cue ball and presses the object ball into the cloth; with backspin the roles swap. TP A-14
> works in the same two-dimensional tangent plane (its slip vector is
> `(v·sin φ − R·ω_z)·t̂ + (R·ω_x·cos φ)·ẑ`), and so does the cushion model of §5.1.


### 4.5 Table Effect During Collision (arXiv 2402.13258)

During collision, the impact force between balls is ~400× gravity. The friction force between balls can have a vertical component that:

1. **Lifts the cue ball** if the friction force exceeds gravity. The friction force between the
   balls is ~20× gravity, so the table's normal force on the cue ball can momentarily drop to
   zero and the ball leaves the cloth. The vertical impulse is `μ_ball · M · U_i`, so the
   vertical speed gained is `Δv_z = μ_ball · U_i` and the height reached is

   ```
   h = Δv_z² / (2g) = (μ_ball · U_i)² / (2g)
   ```

   For `U_i = 10 m/s`, `μ_ball = 0.06`: `h ≈ 1.8 cm` (observable).

   > The frequently copied form `h ≈ μ_ball·U_i²/(2g)` is a typo — it is off by a factor `μ_ball`
   > and gives 31 cm instead of 1.8 cm for the same inputs. The correct scaling is `(μ_ball·U_i)²`.

2. **Pushes the object ball down** into the cloth, activating large static friction.

3. The static friction coefficient between ball and cloth (`μ_stat`) is at least as large as the
   sliding coefficient (0.15–0.40) and is the value that actually applies during the collision:
   the cloth fibres cannot respond within 250 μs, so the full static friction is available.
   This is why the table plays such a large role in ball-ball collisions.

> **Where the vertical impulse comes from.** It is the *vertical component of the friction
> between the balls*, which is why the tangential impulse has to be treated in the whole
> tangent plane and not only along the rail (§4.4.5). The vertical slip `u_z` is
> `R·ω_x·cos φ` for a cue ball whose spin is about the axis perpendicular to the line of
> centres — the roll axis. For a **rolling** cue ball the contact point at the front of the
> ball moves downwards (it is the leading edge of a rolling sphere), so the friction acts
> upwards on the cue ball and downwards on the object ball: the cue ball hops and the object
> ball is pressed into the cloth. With backspin the two swap.
>
> The estimate above is the scale of that effect, not a separate mechanism: with a
> *constant* friction coefficient, the vertical impulse of the tangential model is
> `J_t·|u_z|/u = μ_ball·J_n/m ≈ μ_ball·U_i` for a square hit — which is exactly Kim's
> figure. The general model in §4.4.5 replaces the estimate and carries the sign, the
> cut angle and the speed dependence with it.
>
> The one place the two disagree is the size, and it is worth knowing about. TP A.14's
> friction coefficient *falls* with the sliding speed, so at break speed the model lifts
> the cue ball by about 0.08 m/s where Kim's constant-μ estimate gives 0.5 m/s — a hop
> of a third of a millimetre against his 1.3 cm. Both cannot be right, and which of them
> is right is a measurement rather than a calculation; it is on the calibration list
> (§13.3). What the model does settle is the sign, which Kim's estimate cannot: a stun
> shot has no vertical slip and no lift at all, and a rolling shot lifts the cue ball
> and presses the object ball into the cloth.
>
> **The break-speed number is an extrapolation, and it should be read as one.** The
> friction law is a fit to throw data at modest slip speeds — TP A.14's own speeds run
> from 1 to 7 mph — and a break shot's contact slips at 8–10 m/s, well past the end of
> the data. The tail of an exponential fit is the least trustworthy part of it: at a
> 10 m/s slip the law gives μ ≈ 0.008, a fifth of the coefficient the same fit gives at
> 1 m/s, and the resulting lift of 0.08 m/s is *probably* an under-prediction rather
> than a measurement. Kim's 1.3 cm is a calculation too, with a constant μ and the whole
> friction impulse treated as vertical; it is the number to check against, not the
> number to believe. **No published measurement of the cue ball's hop after a break-speed
> collision was found**, so the fixture records one as `TO BE CALIBRATED` (§14): the
> peak height of the cue ball after its first contact, from high-speed video, at a known
> speed. Until that exists, the break-speed lift is the model's least trustworthy number
> and nothing should be calibrated on it.

**Implementation recommendation:** Treat the table during the collision as (a) an extra
tangential friction contribution on the ball-ball contact, which is already captured by the
slip velocity `u` in §4.4.5 (spin is part of the slip, in both directions), and (b) the
vertical impulse that the vertical component of that same impulse produces — the cue ball's
share of it, `J_t·u_z/u` per unit mass, is its vertical velocity after the collision, and the
object ball's share is absorbed by the cloth, which is what "pushes the object ball down"
means. Do not apply an ad-hoc speed reduction to the object ball: the observed loss is a
consequence of the normal restitution `e` and of the throw mechanics, both of which the
collision model already produces.

### 4.6 Multi-Ball Collisions

Model as pairwise sequential collisions. Detect all ball pairs within distance `2R` and resolve collisions in order of contact time. For practical purposes, simultaneous multi-ball collisions are extremely rare in 8-ball and can be resolved sequentially without noticeable error.

---

## 5. Ball-Cushion Collisions

This is the **least accurately modeled** aspect of pool physics (Kiefl, 2020). Three models were found in the literature:

### 5.1 Recommended Model: Han (2005)

**Reference:** Inhwan Han, "Dynamics in Carom and Three Cushion Billiards," Journal of Mechanical
Science and Technology 19(4), 976–984 (2005). A corrected transcription of the equations, with
two typographical errors in the original identified, is given by Kiefl (2020) and his
[worksheet](https://ekiefl.github.io/images/pooltool/pooltool-theory/ball_cushion_inhwan_han.pdf).

**Assumptions:**
- Instantaneous collision (dt → 0)
- Negligible cushion deformation
- Single point of contact between ball and cushion

**Frame of reference:** The rail is perpendicular to the x-axis and `+x` points *toward* the rail
(away from the playing surface); `+z` is up; `ŷ` runs along the rail. The incoming velocity makes
an angle `φ` with the x-axis.

**Geometry — the nose height is the contact height:**

The cushion's nose height is the one physical input the model needs: the height above the bed at
which the cushion acts on a ball resting on the bed. The WPA specifies it as 63.5% of the ball
diameter (36.3 mm for a 2.25" ball), and the manufacturers describe it in exactly those terms —
"the point of the rubber cushion where the ball would make contact".

A ball resting on the bed has its centre at height `R`, so the contact sits `ε` above the
ball's centre and the contact normal is tilted from the horizontal by

```
ε = h_nose - R
θ = arcsin(ε/R)
```

For the WPA profile: `ε = 7.73 mm = 0.270 R`, `θ = 15.7°`. The contact surface near the nose is
then modelled as a plane inclined `θ` from the vertical, which is what makes a resting ball
contact at the specified height.

> `ε = 0.270 R` is above the 0.1–0.2 R that Han (2005) is usually run with, and it is *not* a
> correction of Han: his paper is about carom and three-cushion billiards, and those ε values
> are for carom cushions, whose noses sit lower. The WPA specification fixes the pool value at
> 0.635 D, and that is what this model uses. Do not "fix" 0.270 R back down into Han's range.

**Why this reading, and not the other one.** There are two ways to read "nose height":

- **As the contact height** (above). This is how the specification and the manufacturers use
  it, and Dr. Dave's explanation of *why* the WPA chose 0.635 D is built on it.
- **As the apex of a small-radius bulge.** On that reading a ball touches the rounded face well
  below the apex — for a 1/4" nose arc whose apex is at 36.3 mm the contact would land at
  29.7 mm, i.e. `ε = 0.039 R`, and the contact normal would be nearly horizontal.

The second reading is self-defeating: if the contact lands at 29.7 mm whatever the nose height
is, then the specified nose height has **no effect on the rebound at all** — and the WPA's
careful choice of 0.635 D, which exists precisely because the height changes the rebound, would
be meaningless. The first reading predicts the right behaviour at all three reference heights:

| Nose height | `ε` | `θ` | Model's vertical impulse (2 m/s square hit) | Dr. Dave on this height |
|---|---|---|---|---|
| 0.500 D (at the ball's centre) | 0 | 0° | **+0.49 m/s up** — the ball is launched, and the launch scales with speed | "rebounding balls would slide more and would **hop significantly** if they come into the cushion with topspin (especially at fast speed)" |
| **0.635 D (the WPA spec)** | +0.270 R | 15.7° | −0.46 m/s (pressed into the slate) → a 3.8 mm hop after the slate rebound | "a good compromise … good rebound performance **without too much ball hop or cloth wear**" |
| 0.700 D (centre of percussion) | +0.400 R | 23.6° | −0.87 m/s — driven down hard, 13.9 mm hop | "would tend to **drive a rebounding ball down into the table**, which would tend to slow the ball more, accelerate cloth wear … and cause the rebounding ball to hop" |

Three independent qualitative statements about three different heights, matched by one model.
That is the strongest available check on the cushion geometry, and it is why `h_nose` — a
measured, specified dimension — is the model's input and `ε` and `θ` are derived from it.

Note what this check does and does not do. It is a **qualitative** anchor: it pins the *sign* of
the vertical impulse and the *ordering* of the three heights, and it is genuinely discriminating
(the apex reading of v1.3 fails it outright). It does not pin the magnitude — the 3.8 mm hop the
model predicts at the spec height is a rigid-model number and still wants high-speed-video
calibration, at low speed as well as at break speed.

> **The three-height check is a fit, not a validation.** The reading of "nose height" was
> *chosen* because it reproduces the three heights, so the fact that it reproduces them is
> not independent evidence for it — it is the reason it was chosen. What the check does
> establish is that the alternative reading (v1.3) cannot be right: it fails on all three
> heights. A real validation has to come from a measurement of a *known* nose height: the
> rebound speed, the rebound angle and the hop height of a ball off a rail whose geometry has
> been measured. That is what the `cushion_rebound` fixture (§14) is for, and until it
> carries measured video the hop numbers in this section are model outputs.
>
> The hop magnitudes have a second, quieter dependency: they come from treating the cushion
> impulse and the slate rebound as *sequential* — the cushion presses the ball in, and then
> the slate throws it back. Mathavan et al. treat the table contact as simultaneous with the
> cushion contact, and the two treatments give different hops. The sequential treatment is
> the simpler one and is what this model does; it is an assumption, not a result.

**Per contact.** The contact height for a ball whose centre is at `z_c` is `z_c + R·sin θ`: the
contact moves up the face as the ball rises. The plane idealisation keeps the normal's tilt at
`θ`; the real nose profile is steepest at the tip and flattens upward, so the true tilt for a
lifted ball is *smaller* and the model over-drives it downward. What matters more is *which*
part of the rail the ball meets: see §11.3 for the three regimes (nose rubber, rail structure,
clear of the rail).

The **tangential** impulse is a separate story: it acts along the contact tangent
`t̂ = (sin θ, 0, -cos θ)`, and it opposes the slip — the ball's own contact-point surface
velocity `(v + ω × r_c)` sliding against a stationary cushion. For a rolling ball the surface
at the rail side of the ball moves *downwards* (it is the leading edge of a rolling sphere), so
the friction force on the ball points *upwards*, while the normal impulse points down. For a
rolling ball the Coulomb limit always binds (sticking would need
`s_x ≤ 3.5·μ_cushion·(1+e)·c`, but `s_x/c = (1 + sin θ)/cos θ ≥ 1 > 0.86`), so the net
vertical velocity is

```
v_z' = (1 + e_cushion)·v_n·(μ_cushion·cos θ - sin θ)
```

which changes sign at `tan θ = μ_cushion`, i.e. at an effective contact height of

```
ε_c/R = μ_cushion / sqrt(1 + μ_cushion²) ≈ 0.139 R   (μ_cushion = 0.14)
```

That threshold corresponds to a nose height of `1.139 R = 0.57 D` — well below the specified
0.635 D, so a standard cushion always presses the ball into the slate. (A cushion whose nose sat
at the ball's centre height would launch the ball instead; that is the 0.500 D row of the table
above.)

**Caveat — the hop scales as v².** `h = v_z'²/(2g)` with `v_z' ∝ v`, so a 5 m/s cushion hit
predicts ≈ 24 mm of hop against the ≈ 3.8 mm of a 2 m/s hit. Real cushion deformation limits
this: the rubber compresses, the contact is not rigid, and the ball is not launched as cleanly.
The rigid model therefore over-predicts the hop at break speeds. It is the right model to ship
(the geometry is sound and the low-speed behaviour is right), but the high-speed hop is where it
will want a calibration factor or a compliant-contact correction — see the `cushion_rebound`
fixture (§14), which pins both ends.

**Impulse model:**

```
c   = v_x(τ) · cos θ + v_z(τ) · sin θ                (normal approach speed, = v_n)
s_x = v_x(τ) · sin θ - v_z(τ) · cos θ + R·ω_y(τ)     (slip along the contact tangent)
s_y = v_y(τ) + R·ω_z(τ) · cos θ - R·ω_x(τ) · sin θ  (slip along the rail)

J_n = (1 + e_cushion) · m · c              (normal impulse, > 0 for an incoming ball)
```

> **The `v_z` terms matter for an airborne ball.** A ball resting on the bed has `v_z = 0`,
> and then `c = v_x·cos θ` and `s_x = v_x·sin θ + R·ω_y` — the form this section used to
> give. A ball that reaches the rail in the air does not: its vertical velocity enters both
> the normal approach speed and the slip, and §11.3 expects exactly that case. The core
> carries the general form.

The slip components are the contact point's velocity (`v + ω × r_c`) projected onto the tangent
plane of the contact, which is spanned by `t̂ = (sin θ, 0, -cos θ)` and `ŷ`:

- along `t̂`: `(v + ω × r_c)·t̂ = v_x·sin θ - v_z·cos θ + R·ω_y`
- along `ŷ`: `(v + ω × r_c)·ŷ = v_y + R·(ω_z·cos θ - ω_x·sin θ)`

> Han (2005) prints `s_y` with the opposite sign (which flips the `v_y` equation) and prints
> `s_x` with an extra `-v_y·cos θ` term that does not follow from the projection above. The two
> forms agree exactly for a perpendicular impact (`φ = 0`), which is the case most simulations
> are validated against; for oblique incidence use the projected forms given here.

The tangential impulse is either enough to stop the slip or is limited by Coulomb friction:

```
J_t(stick)  = (2/7) · m · sqrt(s_x² + s_y²)
J_t(slip)   = μ_cushion · J_n
use J_t(stick) if J_t(stick) ≤ J_t(slip), else J_t(slip)
```

(`2/7` here, not `1/7`: the cushion does not move, so the slip changes by `(7/2)·J_t/m` — this is
the ball-on-a-fixed-plane constant, unlike the ball-ball case of §4.4.5.)

**Post-collision state** (`t̂ = (sin θ, 0, -cos θ)` is the contact tangent in the x–z plane, and
`ŝ = (s_x·t̂ + s_y·ŷ)/|s|` is the unit slip direction):

```
r' = r                                            (instantaneous)

Δv = (1/m) · [ J_n·n̂ - J_t·ŝ ]

ω' = ω + (r_c × (J_n·n̂ - J_t·ŝ)) / I ,  I = (2/5)·m·R²
```

Written out component-wise, the sticking branch is:

```
v_x' = v_x - (1 + e)·c·cos θ - (2/7)·s_x·sin θ
v_y' = v_y - (2/7)·s_y
v_z' = v_z - (1 + e)·c·sin θ + (2/7)·s_x·cos θ
```

> **Erratum.** The `v_x` term is missing from the x-equation as printed in Han (2005) (and from
> Kiefl's transcription of it). The term is required: the same term is present in the z-equation,
> and without it the `θ = 0, e = 1` limit returns `-2·v_x` instead of `-v_x`, i.e. the ball
> rebounds at twice its incident speed, which violates energy conservation. The corrected form is
> used throughout this document. The z-equation above also carries its own `v_z` term, which Han
> elides by writing the model for a ball on the bed (`v_z = 0`); an airborne ball arriving at the
> rail needs it, and the slip `s_x` of the line above already carries `v_z·cos θ` for the same
> reason (§5.1's opening).

**Key effects the model captures:**
1. **Angle of reflection ≠ angle of incidence** (cushion friction and spin).
2. **English changes both the rebound angle and the rebound speed.** With heavy left/right spin
   a ball can rebound *back towards the side it came from* (Mathavan et al., 2010, Fig. 10–11;
   also Cross (2008) for tennis balls). The change in rebound angle is of the order of a few
   degrees at maximum english — not a fixed "degrees per unit english" constant.
3. **The vertical behaviour depends on the cushion height**: a tall cushion presses the ball into
   the slate (the slate's rebound is what makes it hop, §8.1); a low cushion launches it
   directly upward.

### 5.2 Alternative: Mathavan et al. (2010)

More realistic but requires solving differential equations numerically. Recommended only if you need maximum accuracy.

**Reference:** S. Mathavan, M.R. Jackson, R.M. Parkin, "A theoretical analysis of billiard ball dynamics under cushion impacts," Proceedings of the Institution of Mechanical Engineers, Part C 224(9), 1863-1873 (2010).

Useful concrete values from that paper (measured on a snooker table):

| Quantity | Value |
|---|---|
| Contact point height `h` | `7R/5` (so `sin θ = 2/5`, i.e. 0.4 R above the ball's centre) |
| Ball–cushion coefficient of restitution `e_e` | 0.98 |
| Ball–cushion sliding friction `μ_w` | 0.14 |
| Largest speed loss | at incident angles around 40° |
| Sidespin | can raise the rebound speed above the incident speed, and reverse the rebound side |

Note the difference in `e`: the *rail rubber* restitution measured by Mathavan (0.98) is much
higher than the effective ball–rail restitution of 0.6–0.9 quoted by Dr. Dave, which includes the
energy lost in the cloth contact that follows the cushion impact. Use the effective value unless
you are modelling the two contacts separately.

> **Do not port Mathavan's `h = 7R/5` straight across.** His measurements were made on a
> Riley-style *snooker* table, and 7R/5 = 40 mm for a pool ball — about 10% above the WPA pool
> nose height of 36.3 mm. For a K-66 pool table the effective contact height is the one the
> specification gives: `ε = h_nose − R` = 0.270 R, i.e. a 15.7° contact tilt (§5.1). Snooker
> values must be recalibrated for pool.

### 5.3 Simplified Model (Game-Level)

For a game-level implementation, a simplified model may suffice. The important points are that
the normal component is reversed and damped, that the tangential component is reduced by a
Coulomb-limited impulse that is computed from the *contact-point* slip (spin included), that the
spin is updated consistently, and that the ball is **not** given a fictitious upward kick:

```rust
// Simplified cushion collision (instantaneous, rigid cushion).
// n_rail: unit horizontal vector pointing from the playing surface towards the rail.
fn resolve_cushion_collision(ball: &mut Ball, n_rail: Vec3, e_cushion: f64,
                             mu_cushion: f64, nose: &NoseProfile) {
    // Local frame: x = towards the rail, y = along the rail, z = up.
    let x = n_rail;
    let y = Vec3::Z.cross(x);

    // The contact geometry comes from the specified nose height, not from a tuned epsilon.
    // The nose height is the height at which the cushion makes contact with a ball
    // resting on the bed, so the contact sits epsilon above the ball's centre.
    let epsilon = nose.height - ball.radius;
    let sin_t = epsilon / ball.radius;
    let cos_t = (1.0 - sin_t * sin_t).sqrt();

    // Contact point relative to the ball's centre, the contact tangent and the contact normal.
    // The contact face is inclined theta from the vertical, so the normal points away from
    // the rail and *downwards*; the friction impulse along the tangent can point the other
    // way (section 5.1).
    let r_c = x * (ball.radius * cos_t) + Vec3::Z * (ball.radius * sin_t);
    let tangent = x * sin_t - Vec3::Z * cos_t;
    let n_hat = -x * cos_t - Vec3::Z * sin_t;

    // Slip velocity of the contact point (the cushion is stationary).
    let v_contact = ball.velocity + ball.angular_velocity.cross(r_c);
    let s_x = v_contact.dot(tangent);
    let s_y = v_contact.dot(y);
    let slip = (s_x * s_x + s_y * s_y).sqrt();

    // Normal approach speed (positive while the ball is heading into the rail).
    let v_n = -ball.velocity.dot(n_hat);
    if v_n <= 0.0 {
        // The ball is not approaching the rail: there is no contact at all.
        return;
    }

    // Normal impulse, and a tangential impulse capped by Coulomb friction.
    let j_n = (1.0 + e_cushion) * ball.mass * v_n;

    // The tangential part needs a slip to work against. A ball whose contact point is
    // instantaneously at rest has no slip, and then there is no friction impulse - but
    // the normal impulse still stands, and skipping it would let the ball through the
    // rail. (Such a ball exists: a ball with exactly the backspin that cancels the
    // contact point's velocity at the rail.)
    let j_t = if slip > 0.0 {
        let j_t_stick = (2.0 / 7.0) * ball.mass * slip;
        j_t_stick.min(mu_cushion * j_n)
    } else {
        0.0
    };

    // The tangential impulse opposes the slip.
    let slip_dir = if slip > 0.0 {
        tangent * (s_x / slip) + y * (s_y / slip)
    } else {
        Vec3::ZERO
    };
    let impulse = j_n * n_hat - j_t * slip_dir;

    ball.velocity = ball.velocity + impulse / ball.mass;
    ball.angular_velocity =
        ball.angular_velocity + r_c.cross(impulse) / ball.moment_of_inertia;

    // The outgoing vertical velocity is whatever the impulse produced:
    //   v_z' = (1 + e) * v_n * (mu*cos(theta) - sin(theta))
    // If it is positive the ball leaves the cloth (section 8.1); if it is negative the slate
    // rebound handles it. Do not add an upward velocity by hand in this function.
}
```

```rust
/// The physical shape of a cushion nose. `height` is the height at which the cushion makes
/// contact with a ball resting on the bed - the WPA specifies 63.5% of the ball diameter,
/// 36.3 mm. Everything else in the cushion model is derived from it.
struct NoseProfile {
    height: f64,        // nose (contact) height above the bed, m
    tip_radius: f64,    // radius of the nose tip, m (6.35 mm = 1/4") - shape only
}
```

### 5.4 Cushion Geometry (K-66 Profile)

The K-66 cushion profile has:
- Nose height: 63.5% of the ball diameter ≈ 36.3 mm (1 7/16") above the playing surface
- Base width: 1 3/16" (30 mm)
- Radius at nose tip: ~1/4" (6.35 mm)

These three numbers are the *input* to the cushion model. The nose height is above the ball's
radius (R = 28.575 mm), so the cushion contacts the ball 7.73 mm above its centre and the
contact normal is tilted 15.7° from the horizontal (§5.1). The nose height is the height at
which the ball makes contact — that is what the specification measures and what the
manufacturers mean — so the tip radius is a *shape* parameter (it rounds the transition to the
slate and it draws the rail) rather than something that sets the impulse direction.

For this profile the cushion presses the ball into the slate and the ball hops a few
millimetres off it. See §8.1 for the magnitudes and for what changes with a taller cushion.

---

## 6. Ball-Cue Interactions

### 6.1 Cue Strike Model

The cue stick strikes the ball at a specific point, imparting both linear and angular velocity.

**Impact parameter (b):** Distance from ball center to the cue's line of travel (perpendicular offset).
The angle between the cue axis and the line from the ball's centre to the contact point is
`φ`, with `sin φ = b/R`. (Here `φ` is the cue impact angle; §5 uses `φ` for the cushion
incidence angle.)

**Squirt (deflection):** When the cue strikes off-center, the tip first *slides* on the ball's
surface; the friction force is not along the cue axis, so the ball leaves along the resultant of
the normal and friction forces, deflected from the intended direction by the "squirt angle" ψ.
This slip is the origin of squirt (Kim, 2021).

Kim's rigid-cue model gives

```
tan θ = μ_c · ( 1 + (γ⁻¹ - 1)·μ_c·tan φ / (1 + e*·G + μ_c·tan φ) )
ψ = φ - θ
```

where `μ_c` is the effective tip–ball friction coefficient, `e*` the tip COR (~0.75), and `G` and
`γ` are the tip geometry and mass-ratio factors of Kim's model.

**But the rigid-cue model badly over-predicts squirt.** Kim's own evaluation: with a cue/ball
mass ratio `M_c/M ≈ 3` and a strike at half the radius (`φ = 30°`), the rigid model predicts
**ψ ≈ 16°**, "very different from the actual squirt angle that appears in billiard games".

The reason is that a real cue is not rigid. Its vibration period is ~4 ms and the tip-to-ball
contact lasts ~2 ms, so the impulse never reaches the butt of the cue: only the last ~12 cm
responds — an effective **cue-end mass of roughly 12 g**, not the 454 g total cue mass. Modelling
the cue as a rigid body of the full mass is what makes the squirt far too large.

**Practical values and trends** (Cross, 2008; Kim, 2021):

- Squirt for a well-chalked tip is a few degrees: roughly **1°–5°** at maximum english, growing
  almost linearly with the impact parameter for `b < 0.5R` and then flattening.
- Squirt **increases** with the cue's effective end mass and **decreases** with a slicker tip
  (lower `μ_c`). It vanishes for a centre-ball hit.
- Therefore the cue mass that belongs in any squirt formula is the **effective end mass**
  (~12 g for a typical shaft), never the total cue mass.

**Implementation:** use a calibration curve, not Kim's flexible-cue model. Keep a table of
`(b/R → ψ)` pairs, nearly linear up to `b = 0.5R` and flattening beyond, with a single
`squirt_scale` multiplier in the config for global tuning. The effective end mass (~12 g) is
baked into the curve and need not be exposed unless cue customisation is wanted. The curve's
anchors are a calibration target, not a result: `squirt_curve` (§14) records where the
measured values from Cross (2008) or high-speed video go.

> The formula `ψ ≈ arctan((5/2)·(b/R)·(1/(1 + M·R²/I)))` sometimes quoted for squirt is wrong:
> with `M` taken as the total cue mass it predicts ≈9° at `b = 0.5R` and ≈17–18° at `b = R`, which
> contradicts both the measured 1°–5° and the doc's own "2°–5°" claim; and because its
> denominator grows with `M` it predicts that squirt *shrinks* with cue mass, which is the
> opposite of the real trend.

> **The squirt is driven by the *sideways* offset, not by the whole impact
> parameter.** The impact parameter `b` is the perpendicular distance from the
> cue's axis to the ball's centre, so a tip above or below the centre is an
> off-centre hit by that measure too — and looking the squirt curve up at `b`
> gives a draw or a follow shot a squirt of nearly 2°. It does not: the squirt
> comes from the tip *sliding around* the ball, and only a **sideways** offset
> makes it slide sideways. A draw or a follow shot goes straight down the aim
> line. So the curve is looked up at the sideways part of the contact, while
> the spin (`ω = (5/2)(b/R)(v/R)`) and the miscue limit keep using the whole
> impact parameter, because a tip above the centre does give topspin and does
> slip off at the same offset.

**References:** R. Cross, "Cue and ball deflection (or 'squirt') in billiards," Am. J. Phys. 76,
205 (2008). H.-C. Kim, "Motions of a billiard ball after a cue stroke," arXiv:2104.11232 /
New Physics: Sae Mulli 72, 208 (2022).

### 6.2 Post-Impact Ball State

Given a cue velocity `V_cue`, impact parameter `b`, and cue elevation angle `θ`:

**Linear velocity of the ball:**
```
v_ball = (1 + e_cue) · V_cue / (1 + m_ball/M_cue + 5b²/(2R²))
```
(the rigid-cue longitudinal response; `e_cue` is the tip COR, 0.71–0.75 leather /
0.81–0.87 phenolic. At `b = 0` this is the centre-hit form
`(1 + e_cue)·M_cue·V_cue/(m_ball + M_cue)` — for a 19 oz cue at 2 m/s, 1.45 m/s)

> **The impulse *does* depend on `b` — through the ball's effective mass.** The tip pushes the
> ball along the cue's axis; the cue is rigid in compression, so the mass on the cue's side is
> the cue's whole mass `M_cue`. The restitution acts on the relative speed at the contact
> point, which starts at zero however large `b` is — but the mass that *responds* at an offset
> contact point is not `m_ball`: it is the ball's **effective mass** there,
>
> ```
> 1/m_eff = 1/m_ball + b²/I     →     m_eff = m_ball / (1 + 5b²/2R²)
> ```
>
> because an off-centre impulse both translates and spins the ball. The impulse is
> `J = (1 + e_cue)·V_cue/(1/M_cue + 1/m_eff)`, and dividing by `m_ball` gives the formula
> above (Leckie & Greenspan's form). An off-centre hit is a *slower* hit: at `b = 0.5R`,
> `m_eff = m_ball/1.625 = 0.615·m_ball`, and for a 454 g cue and a 170 g ball the ball
> leaves 31% slower than on a centre hit — which is why firm draw is hard. The offset also
> goes into the *torque* (`ω = (5/2)(b/R)(v_ball/R)`, below) and into the *transverse*
> response, which is the squirt.
>
> The transverse response is a different direction and therefore a different pair of masses
> *on the cue's side*: the ball's `m_eff` is the same formula above, but the cue resists with
> its **effective end mass** of ~12 g, not the 454 g of the whole cue, because only the last
> ~12 cm of the shaft moves during the ~2 ms contact (§6.1).
>
> So the two rules do not contradict each other, and the document has to say which is which:
> **the total cue mass belongs to the longitudinal response — the ball's speed, together with
> the ball's own effective mass at the contact point — and the effective end mass belongs to
> the transverse one (the squirt).** A formula that mixes them — the total cue mass in a
> squirt expression, or the end mass in a speed expression — is wrong.

**Angular velocity (spin) from offset hit:**

For a horizontal stroke (θ = 0), taking the shot direction as +x̂, with +ŷ lateral and +ẑ up:
- **Topspin (follow):** Hit above center → `ω_y > 0` (this is the rolling condition of §3.2)
- **Backspin (draw):** Hit below center → `ω_y < 0`
- **Sidespin (english):** Hit the +ŷ side → `ω_z < 0` (clockwise seen from above);
  hit the −ŷ side → `ω_z > 0`

> **Which side is +ŷ?** The shooter's right hand is `direction × up`, so for a shot along +x̂
> the shooter's right is **−ŷ** and the shooter's **left** is **+ŷ**. Read that way, the line
> above says: *left english* → `ω_z < 0`, *right english* → `ω_z > 0` — which is what §4.4.2
> means by "right english", and the sign the implementation follows. The core measures the
> contact offset along `up × direction` (§6.1), which is that same +ŷ, so a **positive**
> `contact_offset_x` is a strike on the shooter's **left**.

The spin magnitude for a tip that grips the ball (no slip at the end of contact) follows from the
torque of the impulse about the ball's centre, `τ = r_c × J`, with `|r_c| = b` and
`J ≈ m_ball·v_ball`:

```
ω = (b · J) / I = (5/2) · b · v_ball / R² = (5/2) · (b/R) · (v_ball/R)
```

Equivalently, in terms of the slip between the tip and the ball surface (Kim, 2021):

```
ω = (F · t / I) × (offset vector)
```

where `F · t` is the impulse delivered by the cue.

> The commonly copied form `ω = b·V_cue/R²` is missing the factor 5/2 and uses the *cue* speed
> instead of the *ball* speed; it under-predicts the spin by more than a factor of two for a
> typical cue.

### 6.3 Miscue Condition

To avoid miscue (the cue slipping off the ball), the impact parameter must satisfy:
```
b ≤ R · μ_cue / sqrt(1 + μ_cue²)
```
(equivalently `μ_static ≥ tan φ = (b/R)/sqrt(1 - (b/R)²)`, Kim, 2021.)

For μ_cue ≈ 0.6: `b ≤ 0.51 · R` — you can hit up to ~51% of the ball's radius from center
before miscuing.

---

## 7. Jump Shots & Ball-Air Interactions

### 7.1 Jump Shot Physics

A jump shot is executed by striking the cue ball **below center** with the cue elevated at an angle θ above horizontal.

**Mechanism (this is the part most often got wrong):** the elevated cue does *not* launch the
ball upwards. It drives the ball **downwards into the slate**, and it is the slate's reaction
that launches the ball. The cue ball is compressed against the cloth and rebounds off it, so a
jump shot is really a ball–slate bounce with a horizontal velocity component.

**The relevant criterion is an impulse, not a force.** Comparing a force to `m·g`
("`F_vertical ≥ m_ball·g`") is meaningless here, because the cue strike is an impulse of a few
milliseconds: the ball is pressed into the cloth for far less time than it takes gravity to pull
it down. What matters is the downward velocity the ball carries into the slate:

```
v_z↓ = (1 + e_cue) · M_cue · V_cue / (m_ball + M_cue) · sin(θ)    (downward)
```

and the resulting hop, after the slate's restitution (§7.2):

```
h_jump = (e_table · |v_z↓|)² / (2g)
```

For `v_z↓ = 1 m/s` and `e_table = 0.6`: `h_jump ≈ 1.8 cm`; for `v_z↓ = 2 m/s`: `h_jump ≈ 7 cm`.

**From Regis Petit's "Billiards: Theory of the Game":**
- The cue must be elevated (typically 10°–45°)
- The contact point on the ball must be below center
- The ball-cloth impact during the jump creates a reactive force from the slate

**Post-jump trajectory (ball in air):**

While airborne, the ball is subject to:
1. **Gravity:** `a_z = -g`
2. **Air drag:** `F_drag = -½ · ρ_air · C_d · A · |v| · v` — *not modelled, see below*
3. **Magnus effect (if spinning):** `F_Magnus ∝ ω × v` — *not modelled*

**Air drag parameters** (for the size of the effect, not for a model):
- Air density ρ_air ≈ 1.225 kg/m³
- Drag coefficient for sphere C_d ≈ 0.47
- Cross-sectional area A = π · R² ≈ 0.00256 m²

For a jump shot at 3 m/s with topspin of 500 rpm:
- Drag deceleration: `½·ρ·C_d·A·v²/m ≈ 0.04 m/s²` — negligible next to gravity
  (9.81 m/s²), even over the longest plausible jump
- Magnus effect: also negligible *in practice*. The relevant parameter is the spin ratio
  `S = ωR/v` (≈0.5 for 500 rpm at 3 m/s), not the Reynolds number; the induced force is
  modest and a jump lasts only ~0.1–0.3 s, so the resulting Δv is a few cm/s.

**Recommendation:** **Model gravity only.** Air drag and the Magnus effect are *minor* for
jump shots in pool — 0.04 m/s² against 9.81 m/s² — and modelling drag would cost more than it
buys:

- A drag law quadratic in `v` has no closed-form trajectory. The event-driven core advances
  every ball *analytically* between events (§11.2), and the air state is one of the four
  closed-form states of §3. With drag, the air state would have to be integrated, and the
  whole point of the architecture — that a ball's position at any `t` is known without
  stepping — would be gone for the one state that can go on for a third of a second.
- At 0.04 m/s² the drag changes the height of a 3 m/s jump by well under a millimetre.

So the drag and Magnus numbers above are quoted to show that they do not matter, not as a
model to implement. The same goes for a ball in the air for longer than a jump shot: the
only air state the model has is the ballistic arc of §3.

### 7.2 Ball Landing (Ball-Slate Interaction)

When the ball lands back on the table:

1. The ball impacts the slate with a vertical velocity `v_z`.
2. The slate applies a normal force, reversing `v_z` with coefficient `e_table` (0.5–0.7):
   ```
   v_z(final) = -e_table · v_z(initial)
   ```
3. The ball typically bounces 1–3 times before settling, with each bounce losing energy.
4. Horizontal velocity is largely preserved (with minor friction loss during contact).

**Implementation:** When a ball's z-position returns to near 0 after being airborne, apply the COR to the z-velocity and check if the ball should continue bouncing or settle.

---

## 8. Ball-Slate Interactions

### 8.1 Ball Popped by Cushion

A cushion hit can make the ball hop. Two vertical effects act at the contact (§5.1): the
cushion's **normal** impulse presses the ball into the slate, and the cushion's **friction**
impulse lifts it. Which one wins is set by the contact height, with the flip threshold at

```
ε_c/R = μ_cushion / sqrt(1 + μ_cushion²) ≈ 0.139 R   (μ_cushion = 0.14)
```

which corresponds to a nose height of 0.57 D — below the specified 0.635 D, so a standard
cushion always presses the ball in. For a rolling ball the Coulomb limit always binds, so the
outgoing vertical velocity is

```
v_z' = (1 + e_cushion)·v_n·(μ_cushion·cos θ - sin θ)
```

For a 2 m/s square cushion hit:

| Nose height | `ε` | `v_z'` | What happens |
|---|---|---|---|
| 0.500 D (at the ball's centre) | 0 | **+0.49 m/s** (up) | the cushion launches the ball; it rises ≈ 12 mm, and the launch scales with speed |
| **0.635 D (the WPA spec)** | **0.270 R** | **−0.46 m/s** (down) | pressed into the slate; the slate rebound (§7.2) gives +0.27 m/s, i.e. a **3.8 mm hop** |
| 0.700 D (centre of percussion) | 0.400 R | −0.87 m/s (down) | driven into the slate hard; a 13.9 mm hop after the rebound |

So the standard case is the middle row: the cushion presses the ball into the slate, and the
slate's rebound is what makes it hop. The "the rails pop the ball into the air" complaint
corresponds to a low cushion — one whose nose sits at or below the ball's centre height — and
that is a mounting problem, not a physics feature.

**The hop scales as v².** A 5 m/s cushion hit predicts ≈ 24 mm against the ≈ 3.8 mm of a 2 m/s
hit, but real cushion deformation limits how cleanly the ball is launched. The rigid model
over-predicts the high-speed hop; treat it as the calibration point it is.

**If the ball was pressed into the slate**, the rebound is

```
v_z↑ = e_table · |v_z↓|
h_hop = v_z↑² / (2g)
```

> The formula `v_z ≈ (h_cushion - R)/R · |v_n| · sin(φ)` does not correspond to any published
> model; it is dimensionally plausible but has no derivation behind it. Use the impulse
> treatment above instead.

**Implementation:** the cushion handler returns the outgoing `v_z` from the impulse model and
nothing else. If `v_z < 0` the ball is pressed into the slate: apply the slate rebound (§7.2)
to it. If `v_z > 0` the ball leaves the cloth as an airborne ball and simply flies its
ballistic arc until it lands (§7.2). Do not add an upward velocity by hand in the cushion
handler.

### 8.2 Ball-Slate Bounce

When any ball hits the slate (from cushion pop, jump landing, or ball-off-table return):

```
v_z(after) = -e_table · v_z(before)
```

where `e_table ≈ 0.5–0.7`.

If `|v_z(after)|` is small (below a threshold), the ball settles on the table and `v_z = 0`.

> **The threshold is what stops the bounce sequence, and it has to be a number.** A slate
> bounce multiplies the vertical speed by `e_table` on every hit, so `v_z` after `n` bounces
> is `v_z·e_table^n`. With `e_table = 0.6` that sequence never reaches zero: it has
> *infinitely many* events in a finite time. This is the classic Zeno behaviour of a
> restitution model, and it is not a modelling subtlety — it is an infinite event loop, and
> an event-driven core will sit in it until it hits a limit.
>
> The cure is the settle threshold, and it is a **length-and-speed threshold, not a bounce
> count**: when the rebound's vertical speed falls below `v_settle = 0.02 m/s`, the ball is
> treated as settled, `v_z` is set to zero and the ball is back on the bed. The number is
> arbitrary within a wide band — 0.02 m/s is a hop of 20 μm, which nothing can see — but it
> has to exist, it has to be in the config, and it has to be *the same* on every run, because
> the event sequence is part of the deterministic state (§11.4).
>
> A second guard sits behind it: the event loop has a hard limit on the number of events it
> will resolve (§12.4). A Zeno sequence that the threshold somehow missed would then be cut
> off rather than run forever — but the threshold is the real cure, and the event limit is
> the net under it.

---

## 9. Ball-Pocket Interactions

A pocket is not a hole in the slate. It is a short run of rail (the two **jaws**, or
**facings**), a cut edge in the slate (the **lip**), a shelf of slate behind that lip, and a drop.
A ball is pocketed only if it survives all four: it must pass between the jaw tips, cross the
lip, cross the shelf without rattling back out, and pass the drop point. Any one of those can
fail, which is why pocketing is the hardest thing in pool to model.

### 9.1 Geometry

Per pocket (dimensions in §2.3):

| Element | Corner pocket | Side pocket |
|---|---|---|
| Mouth (jaw tip to jaw tip) | 4.5"–4.625" | 5.0"–5.125" |
| Cut angle (the specified angle) | 142° ± 1° | 104° ± 1° |
| Facing angle from the pocket centreline (the default reading, below) | 7° | 14° |
| Slate cut angle at the lip (from vertical) | 12°–15° | 12°–15° |
| Shelf depth behind the mouth | 1"–2¼" | 0"–3/8" |
| Drop point radius | 1/8"–1/4" | 1/8"–1/4" |

> **The facing angle is a reading of the cut angle, and it is an assumption.** The WPA
> specifies the *cut angle* — "the cut angles of the rubber cushion and its wood backing
> (rail liner) for both sides of the corner pocket entrance must be 142 degrees (+1)" — and
> it is a per-side angle: the same on both sides of the entrance. But the specification is
> text: it has no drawing, and it does not say which two lines the cut angle is measured
> between. Two readings are possible, and the model has to pick one:
>
> * **The default: the cut angle is the angle between a facing and its own rail line.** The
>   facing is then `180° − cut` from that rail line, measured into the pocket; subtract that
>   from the angle between the rail and the pocket's centreline — 45° at a corner, 90° at a
>   side — and each facing lies **7°** off a corner's centreline (`45° − (180° − 142°)`) and
>   **14°** off a side pocket's (`90° − (180° − 104°)`). The included funnel angles are then
>   14° and 28°, which is what real jaws look like. (Earlier versions printed 13° for the
>   side pocket here; that was the BCA's 103° run through this rule, not the WPA's 104°.)
> * **The rejected reading: the cut angle is the angle at which the two facings meet at the
>   throat.** Then each facing is `(180° − cut)/2` from the centreline — 19° for the corner,
>   but *38°* for the side pocket: a 76° V, wide enough for a ball to cross the mouth
>   anywhere without converging on either jaw. Real side-pocket jaws do not look like that,
>   so this reading is not the default. Nothing in the WPA text settles it; the default is
>   the one that matches a table you can look at, until someone measures one.
>
> The normal of a facing is rotated by the facing angle away from the perpendicular to the
> centreline, and that is the normal used for the jaw collision. The consequence for the
> implementation is that **the facing angle has to be a config parameter, not a derived
> constant**: the table config carries the cut angle, the facing angle is derived from it by
> the default rule above, and a table that has been measured can be matched by overriding the
> facing angle directly. The `pocket_drop` fixture (§14) checks the resulting rattle
> behaviour.

Everything here is table-dependent. Pockets vary between manufacturers, and many tables are
"shimmed" to make the openings smaller. Treat the numbers above as the WPA reference
configuration and expose them in the table config so a specific table can be matched.

### 9.2 The state machine

Model the pocket as a small state machine attached to the ball:

```
ON_TABLE ──(centre crosses the mouth line)──► ENTERING
   ▲                                            │
   │                                            ├─(centre crosses the drop line)──► POCKETED
   │                                            │
   └────(centre crosses back over the mouth)────┘   ("rattle out")
```

- **ON_TABLE** — normal ball-cloth physics (§3).
- **ENTERING** — the ball's centre is between the mouth line and the drop line. It is off the
  cloth and sliding on slate. It may still hit a jaw.
- **POCKETED** — the centre has crossed the drop line; the ball is in the cabinet.

Three distinct failure modes have to be represented, and they are the whole reason the pocket
needs its own model:

1. **Rattle out** — the ball hits a jaw, is deflected back out of the mouth, and stays on the
   table. This is the most common pocket failure and is the dominant effect of the jaw angles.
2. **Hung up** — the ball crosses the mouth but runs out of speed on the shelf before reaching
   the drop. It must not be counted as pocketed.
3. **Rattled in** — the ball hits a jaw, drops onto the shelf and still makes it into the
   pocket. Perfectly normal, and the reason a "bad" angle can still go in.

### 9.3 Jaw collisions

The facings behave like short pieces of cushion: they are struck with the same impulsive model
as §5, with the pocket's own restitution and friction, and with the facing's own normal
(§9.1). Two differences from a rail:

- **Lower restitution.** The rubber is short and unrestrained, so the ball loses more energy.
- **Higher friction.** The ball is rubbing along the facing's edge and the slate.

There are no published measurements for `e_jaw` and `μ_jaw`; start from `e_jaw = 0.60` and
`μ_jaw = 0.30` (against `e_cushion = 0.75` and `μ_cushion = 0.14` for a rail) and calibrate
against the rattle behaviour the fixture in §14 checks.

Because the jaw collision is an ordinary instantaneous impulse, it slots straight into the
event-driven loop: the TOI test is a swept sphere against a line segment, and the resolution is
the §5 impulse with the jaw's parameters. Nothing special is needed beyond the geometry.

### 9.4 The shelf and the drop

Once the ball is past the lip it is sliding on slate, not cloth, so the friction model changes:

- `μ_slate` (slate is slipperier than cloth; use ≈ 0.1–0.2) and gravity along the shelf.
- The shelf is effectively horizontal, so a slow ball simply runs out of speed on it.
- The cut edge at the back of the shelf is angled 12°–15° from vertical, which stops a ball from
  riding back up over the lip and is what makes the pocket accept balls that arrive at a poor
  angle.

**Capture condition:** the ball is pocketed when its *centre* crosses the drop line (the slate's
cut edge). Using the centre rather than the edge of the ball is what makes a ball that is
partly over the drop still count as pocketed, which is the real behaviour.

**Speed threshold:** a ball that crosses the mouth with too little speed to reach the drop will
stop on the shelf. The threshold follows from the shelf length and `μ_slate`:

```
v_min_to_drop = sqrt(2 · μ_slate · g · shelf_depth)
```

For a 1.75" (44 mm) corner shelf and `μ_slate = 0.15`: `v_min ≈ 0.36 m/s`. Balls slower than
that hang up. Note that this is a *hanging* condition, not a rattle — the ball stays in the
pocket area, which under 8-ball rules is a ball in hand, not a lost ball.

### 9.5 Effective pocket size

The pocket is not a target of fixed size. Dr. Dave's measurements (Pocket Effective Size and
Center) show that the effective centre of a pocket shifts with the entry angle and speed: for a
corner pocket the ideal aim point moves by up to ~0.6" (and ~0.3" for a side pocket) between a
slow, shallow entry and a fast, square one.

That shift is not an extra parameter — it falls out of the jaw geometry and the friction model.
If the simulated pocket does not reproduce it, the jaw restitution or friction is wrong. This is
exactly what the `pocket_drop` fixture (§14) checks.

### 9.6 Parameters to expose

| Parameter | Default | Notes |
|---|---|---|
| `e_jaw` | 0.60 | jaw restitution (lower than a rail) |
| `μ_jaw` | 0.30 | jaw friction |
| `μ_slate` | 0.15 | friction on the pocket shelf |
| `shelf_depth_corner` | 1.75" | WPA range 1"–2¼" |
| `shelf_depth_side` | 0.25" | WPA range 0"–3/8" |
| `pocket_angle` | 13.5° | slate cut angle from vertical |
| `mouth_corner` | 4.56" | jaw tip to jaw tip (WPA 4.5"–4.625") |
| `mouth_side` | 5.06" | jaw tip to jaw tip (WPA 5"–5.125") |
| `corner_cut_angle_deg` | 142° | the specified horizontal pocket cut angle (WPA ±1°) |
| `side_cut_angle_deg` | 104° | the specified horizontal pocket cut angle (WPA ±1°) |
| `corner_facing_angle_deg` | 0 (derived) | the facing's angle from the centreline — set it for a measured table |
| `side_facing_angle_deg` | 0 (derived) | the facing's angle from the centreline — set it for a measured table |

### 9.7 Implementation notes

- The pocket is a **region**, not a trigger volume. Do not pocket the ball when its centre
  merely enters the mouth: that would swallow every rattle.
- The jaw segments are part of the table's boundary geometry, so the TOI solver (§11.3) needs
  to handle "cushion segment" and "pocket region" as different boundary types.
- Rattling is deterministic by construction: each jaw impact is an event with an explicit time,
  and equal-time events are broken by ball index (§11.4).
- A ball that ends up on the shelf (hung up) must remain in the simulation as a stopped ball
  until the rules resolve it (ball in hand), not be silently deleted.

---

## 10. Balls Leaving the Table

### 10.1 Ball Going Off the Table

When a ball is hit hard enough (or jumped) to leave the playing surface:

**Detection:** Check if the ball's (x, y) position is outside the table boundaries (accounting for ball radius).

**Behavior options:**

1. **Ball drops and stays off the table** (common in casual play):
   - Remove from simulation
   - Mark as "ball off table"
   - In 8-ball rules, this is a foul; the opponent gets ball-in-hand

2. **Ball bounces back onto the table** (realistic physics):
   - The ball is in free fall
   - It may bounce off the floor, side rails, or table apron
   - Eventually lands back on the table (or not)
   - Requires 3D physics with the floor and table structure

**Recommendation:** For a game, option 1 is simpler and sufficient. For maximum realism, implement option 2 with a simple 3D physics model including floor and table geometry.

### 10.2 Minimum Speed to Leave the Table

A ball travelling at speed `v` across a 100" (2.54 m) table takes approximately:
```
t = 2.54 / v seconds
```

With rolling friction `μ_r ≈ 0.01` and `g = 9.81`:
```
deceleration = μ_r · g ≈ 0.098 m/s²
```

The distance a rolling ball covers before stopping is `d = v₀² / (2·μ_r·g)`:

| Initial speed | Distance before stopping |
|---|---|
| 3 m/s | ≈ 46 m |
| 5 m/s | ≈ 128 m |

So a ball is nowhere near being stopped by friction within the length of a table. In practice it
hits a cushion or a pocket long before friction stops it.

**Minimum speed to reach the far end of the table** (i.e. to travel the 2.54 m of playing
surface): solving `d = v₀²/(2·μ_r·g)` for `v₀` gives

```
v_min = sqrt(2 · μ_r · g · d) ≈ 0.71 m/s
```

— a very gentle shot is already enough to cross the table.

**Minimum speed to actually leave the table** is a different question, because the ball has to
clear the rail (≈50 mm above the cloth), not merely reach it. Two routes exist:

1. **The ball is already airborne.** A jump shot (§7.1) or the vertical impulse of a
   ball-ball collision (§4.5) can lift the ball, and an airborne ball that strikes the rail
   above the nose height can sail over it. The rail's top edge — not the cushion nose — is what
   the ball has to clear, and it is ~15 mm higher than the nose.
2. **The ball hops off the cushion.** On a correctly mounted cushion the simple model predicts
   only a millimetre-scale hop (§8.1), so this is not a realistic route; it becomes one on a low
   or worn cushion, where the cushion's friction throws the ball upward instead.

Because route 1 depends on the ball's height at the rail and route 2 on the cushion condition,
there is no single "minimum speed" to quote. Balls leaving the table is a rare event in real
play, which is why §10.1 recommends simply removing the ball and calling the shot a foul.

---

## 11. Deterministic Simulation Architecture

### 11.1 Fixed Timestep Simulation

**References:**
- Gaffer on Games, "Fix Your Timestep!" (2004) — https://gafferongames.com/post/fix_your_timestep/
- Gaffer on Games, "Snapshot Interpolation" (2014) — https://gafferongames.com/post/snapshot_interpolation/

**Core principle:** Decouple simulation rate from rendering rate.

```
time_accumulator = 0.0
FIXED_DT = 1.0 / 60.0  // 60 Hz simulation

loop:
    frame_start_time = get_time()
    delta_time = frame_start_time - previous_time
    previous_time = frame_start_time
    
    // Clamp to prevent spiral of death
    if delta_time > 0.25:
        delta_time = 0.25
    
    time_accumulator += delta_time
    
    // Update simulation in fixed steps
    while time_accumulator >= FIXED_DT:
        update_physics(FIXED_DT)
        time_accumulator -= FIXED_DT
```

**Why fixed timestep?**
- Same inputs → same outputs (determinism), independent of the render frame rate
- No accumulation of floating-point drift across variable-length frames
- Essential for replay systems and network lockstep

> **This determinism is the *stepped* architecture's, and only that one's.** The bullets
> above describe the fixed-step loop. The core is event-driven (§11.2, §12.4), and there a
> fixed timestep is not what makes two runs agree — the closed forms are, together with the
> total order the tie-break rule imposes on simultaneous events. "Same inputs → same
> outputs" holds in both architectures; only the mechanism is different (§12.4).

**What a fixed timestep does *not* do:** it does not prevent tunnelling. A fixed step makes
tunnelling *consistent* across frame rates, but if a ball moves further than a diameter in one
step, it will pass straight through another ball or through a cushion. At 240 Hz a 13 m/s break
shot moves `13/240 = 5.4 cm` per step — 94% of a ball diameter (5.715 cm) — and a cushion is only
1.6–6.4 mm of rubber. Discrete "distance < 2R" checks will miss those collisions.

Two ways out:

1. **Continuous (swept) collision detection** — solve for the time of impact within the step
   rather than testing the end-of-step position (§11.3).
2. **Event-driven simulation** — do not step at all; solve for the next event time (§11.2).

### 11.2 Two Architectures: Stepped vs Event-Driven

Section 3 gives *closed-form* trajectories for each motion state (slide / roll / spin / air).
That is an event-driven description of the world, not a stepped one: between events a ball's
state is known analytically for any `t`, so the simulation never needs to integrate anything.

**Event-driven (recommended core, as in pooltool):**

```
loop:
    for each ball: compute the time until its next event
                   (state change, ball-ball hit, cushion hit, pocket, rest)
    take the earliest event
    advance every ball analytically to that time
    apply the event (instantaneous impulse, or a state switch)
```

| | Stepped integration | Event-driven |
|---|---|---|
| Collision ordering | approximate; simultaneous events are fuzzy | exact by construction |
| Tunnelling | possible unless CCD is added | impossible |
| Step-size error | yes | none (closed form) |
| Cost of a long shot | many steps | a handful of events |
| Renderer | samples positions each frame | gets a piecewise-analytic path to interpolate |
| Implementation | simple | needs TOI solvers (§11.3) |

The "Renderer" row matters for the Godot side: with an event-driven core you hand the renderer a
list of segments ("ball 7 slides in a straight line until t=1.83 s, then rolls until t=4.1 s")
and it can interpolate the exact position at any render time, including sub-step times, with no
accumulated interpolation error.

> **The render time is clamped to the shot.** A renderer's frame is not the
> shot's duration: the last frame of a game loop is at the duration plus a
> fraction of a frame, so the render time can be a little past the end of the
> shot. The path has nothing to interpolate at that time, and a lookup that
> misses every segment would answer "the ball is at the origin of the table".
> The time is therefore clamped to the shot: a time past the end gives the
> end, and a time before the start gives the start. The balls are where the
> shot left them, which is what a renderer wants on its last frame.

**Recommendation:** use the event-driven core for the shot simulation (it makes §3's closed-form
equations the single source of truth and removes tunnelling), and keep a fixed-timestep loop only
for continuous/ambient motion (ball-in-hand placement, cue stick animation, UI).

### 11.3 Collision Detection Strategy

**Broad phase:** Spatial partitioning (grid or BVH) to find potentially colliding pairs.

**Narrow phase:** Time-of-impact (TOI) tests rather than overlap tests:
- **Ball-ball:** solve for the first `t` at which the centres are `2R` apart (§ below)
- **Ball-cushion:** solve for the first `t` at which the ball surface meets the cushion. The
  boundary is the rail profile, and the rail has to be treated as **three different surfaces**
  depending on how high the ball hits it:

  1. **The nose rubber** — the ball's contact height is at or below the nose height
     (`h_nose`, 36.3 mm). This is the overwhelmingly common case, and it is the cushion model
     of §5.1: a swept sphere against the inclined contact face, with the normal tilted `θ` from
     the horizontal and `ε = h_nose - R`. The face is a *plane* inclined `θ` from the vertical,
     so the TOI is the usual swept-sphere-against-a-plane test: `x_c(t)` is quadratic in `t`
     (rolling friction) and the contact condition is linear in it, so the TOI is a
     **quadratic**.

     > **The nose's height is a knife edge, and the test has to know it.** A ball that rests on
     > the bed touches the nose at *exactly* the nose's height — that is what "63.5% of the
     > diameter" means — so a test that rejects a contact below the nose is comparing against a
     > boundary that the common case sits exactly on. With a tolerance of an ulp, a ball whose
     > height came from a single-precision number (a game that keeps its balls in `f32` is a
     > fraction of a nanometre off) is rejected, and the ball goes *through* the rail instead of
     > bouncing off it. The tolerance has to be a length — a micron is far below anything that
     > matters physically and far above the noise in the numbers.
  2. **The rail structure** — the ball's lowest point is above the nose but below the rail's
     top edge, i.e. its centre is between `h_nose + R` = 64.9 mm and roughly `h_nose + 15 mm +
     R` = 80 mm. Here the ball strikes the *upper* structure — the rubber's back and the rail's
     wood face — which is a different surface with a different restitution (wood is stiffer and
     less bouncy than K-66 rubber) and a different normal. Model it as its own boundary type
     with its own `e` and `μ`; do **not** extrapolate the rubber model up into it.
  3. **Clear of the rail** — the ball's centre is above ~80 mm, so its lowest point is above
     the rail's top edge and it sails over the table (§10.2).

  Note that 64.9 mm is "the ball clears the **nose**", not "the ball clears the **rail**" — the
  rail's top edge is ~15 mm higher than the nose, and the band in between is regime 2. The
  solver must branch on the regime rather than run one model through all three.

  For an airborne ball in regime 1, `z_c(t)` is quadratic as well as `x_c(t)`, and the
  contact condition against the inclined face is **still a quadratic**. The reason is that
  the face is a *plane*: the contact condition is the signed distance to the plane, which is
  linear in the ball's position, and the position is quadratic in `t`. A quartic would need
  a *curved* surface — the rounded nose tip of §5.4, if the model ever used the real profile
  instead of the inclined plane — or a ball–ball contact (§ below). The nose tip radius is a
  shape parameter and the model does not use it, so the time of impact in regime 1 is a
  quadratic for a ball on the bed and for a ball in the air alike.

  > **An airborne ball has to be branched on the regime too.** The three regimes are
  > decided by the height of the ball *at the moment of the contact*, and for a ball in
  > the air that height is a function of `t`. So the solver has to find where the ball
  > crosses the rail line and then ask which of the three surfaces it meets there: the
  > nose rubber if it is low enough, the rail structure if it is in the band, and over
  > the rail if it is above the top. A ball that goes over is *off the table* (§10.2);
  > a ball on the bed that is past the rail has gone *through* it, because the cushion
  > is in the way and could not have been passed.
- **Ball-pocket:** point-in-circle/polygon test, with the pocket's cut angle (§2.3)
- **Ball-off-table:** boundary check on the ball's centre against the rail line

**Ball-ball collision time (constant velocity):**
```
Let dp = p₂ - p₁ (position difference)
Let dv = v₂ - v₁ (velocity difference)

Solve: |dp + dv · t|² = (2R)²
→ |dv|² · t² + 2(dp · dv) · t + |dp|² - 4R² = 0

t = (-2(dp·dv) - sqrt(4(dp·dv)² - 4|dv|²(|dp|² - 4R²))) / (2|dv|²)
```

Pick the smallest positive `t`.

> **This quadratic is only valid for straight-line motion.** Under cloth friction the positions
> are quadratic in `t` (`p(t) = p₀ + v₀·t + ½·a·t²`), so the time-of-impact condition
>
> ```
> |Δp₀ + Δv·t + ½·Δa·t²|² = (2R)²
> ```
>
> is a **quartic** in `t`, not a quadratic. In an event-driven core, where each segment's
> trajectory is known in closed form, solve the quartic (or use conservative advancement /
> a swept-sphere test) instead of assuming constant velocity. With a stepped integrator the
> usual workaround is to solve the quadratic against the *start-of-step* positions and accept the
> error.

### 11.4 Determinism in Rust

**Precision is an accuracy decision, not a determinism decision.** IEEE-754 defines `+`, `-`,
`*`, `/` and `sqrt` exactly, and Rust maps them to correctly-rounded hardware instructions on
every target (x86-64 SSE, ARM, WASM). So `f32` vs `f64` does not change whether two runs agree;
both are bit-exact for these operations. Choose `f64` for accuracy (a 30 s shot at 240 Hz is
7200 steps of accumulation across 16 balls) and be consistent about it everywhere.

The real determinism risks are:

1. **Platform libm.** `sin`, `cos`, `powf`, `exp`, `ln` call the platform's libm, which differs
   between glibc, musl, macOS and different library versions — *and* between the same platform
   on different hardware with different SIMD paths. Section 3's rotation matrices need `sin`/`cos`,
   so this is not avoidable by choosing a different architecture. Use the [`libm`](https://crates.io/crates/libm)
   crate (a pure-Rust, musl-derived implementation) instead of `f64::sin`/`f64::cos`, or supply
   your own polynomial approximations. Anything computed once at startup (cue direction, table
   geometry) should be computed *outside* the deterministic core and passed in as data.

   > **`hypot` belongs on the list too.** It looks like arithmetic — "the length of a
   > vector" — but it is not correctly rounded and it is not specified by IEEE-754: the
   > platform is free to compute it any way it likes, and glibc and musl do not agree in
   > the last bit. The core uses `hypot` to decide whether a ball is moving, so a
   > platform that disagrees by an ulp can decide that a ball has stopped when another
   > platform says it is still rolling — and then the event sequence, and the hash, are
   > different. `libm::hypot` is the same code everywhere, so the core calls that one.

   > **The rule is enforced, not just written down.** A test in the core reads the
   > core's own source and fails if a platform transcendental — `sin`, `cos`, `exp`,
   > `hypot`, `powf`, anything of that family — appears anywhere outside a test module.
   > A leak is found on the machine that wrote it, rather than as a hash mismatch on
   > the machine that did not.
2. **`HashMap` iteration order.** Rust's `HashMap` uses a per-process random seed, so iteration
   order changes between runs of the same binary. Never iterate a `HashMap` to drive simulation
   order; use a `Vec`, a `BTreeMap`, or an explicitly sorted list.
3. **Parallel reductions.** `rayon`'s `reduce`/`sum` over floats may combine partial results in a
   different order on different runs, which changes the last bits. Reduce sequentially, or use a
   fixed-order tree reduction.
4. **Unstable sorts and partial comparisons.** `sort_unstable` is fine for reproducibility only
   if the comparator is a total order; `partial_cmp` returns `None` for `NaN` and makes the
   order input-dependent. Sort by `(key, ball_index)` so ties are broken deterministically.
5. **Simultaneous events.** Two collisions at the same instant must be ordered by an explicit
   rule — e.g. by ball index — or two runs can diverge.
6. **NaN / Inf.** Guard `sqrt` of negative arguments, zero-length normals and division by zero;
   a single `NaN` silently poisons every later comparison and breaks reproducibility.
7. **FMA contraction and fast-math.** Rust does not contract `a*b+c` into an FMA by default, but
   enabling `+fma`/`fast-math` for one target changes results. Keep the flags uniform across
   every target in the release profile.

> `min`/`max` instead of branches is a branchless-code performance tip, not a determinism rule —
> both forms are deterministic. Keep it if you like it; it is not load-bearing here.

**Lockstep networking (if multiplayer):**
- Each client runs the same simulation
- Inputs are synchronized across clients
- Client n waits for input from all other clients before advancing to step n+1
- Only 2–4 players recommended (Gaffer on Games)

**Snapshot interpolation (alternative to lockstep):**
- Server runs simulation at fixed rate
- Clients receive snapshots and interpolate between them
- More bandwidth, less waiting

---

## 12. State Representation & Input Model

### 12.1 Ball State (per ball)

```rust
struct BallState {
    // Position (3D)
    position: Vec3,      // (x, y, z) in table coordinates
    
    // Linear velocity (3D)
    velocity: Vec3,      // (vx, vy, vz) in m/s
    
    // Angular velocity (3D)
    angular_velocity: Vec3, // (wx, wy, wz) in rad/s
    
    // Derived: motion state
    motion_state: MotionState, // SLIDING, ROLLING, SPINNING, STATIONARY, AIRBORNE
    
    // Physical properties
    mass: f64,           // 0.170 kg (6 oz, standard)
    radius: f64,         // 0.028575 m (standard)
    moment_of_inertia: f64, // (2/5) * m * R²
    
    // Game properties
    number: u8,          // 0 = cue ball, 1-15 = object balls
    is_pocketed: bool,
    is_off_table: bool,
}
```

All simulation state is `f64`. The precision choice is an accuracy decision — it does not affect
determinism (§11.4) — but it must be consistent across the whole codebase.

### 12.2 Game State

```rust
struct GameState {
    balls: [BallState; 16],
    cue_stick: CueStickState,
    turn: u8,            // current player
    phase: GamePhase,    // aiming, shooting, settling, etc.
    foul: bool,
    game_over: bool,
    winner: Option<u8>,
}
```

### 12.3 Input Model

The input to the physics simulation is:

```rust
struct ShotInput {
    // Cue stick state at moment of strike
    cue_speed: f64,            // m/s
    // Direction of the cue axis as a unit vector (x, y, z) in table coordinates.
    // The elevation for jump shots is baked into this vector.
    cue_direction: Vec3,       // pre-normalized by the caller
    
    // Contact point on cue ball (offset from center).
    //
    // `contact_offset_x` is measured along `up x direction`, which is the +y side of
    // the frame of section 6.2 - and +y is the shooter's LEFT (the shooter's right is
    // `direction x up`). So a POSITIVE `contact_offset_x` is a strike on the shooter's
    // left, and a negative one is a strike on the shooter's right.
    contact_offset_x: f64,     // -R to +R (left/right english)
    // `contact_offset_y` is measured along `up`: positive is above the centre (topspin
    // or follow), negative is below it (backspin or draw).
    contact_offset_y: f64,     // -R to +R (top/bottom spin)
    
    // Current state of all balls at moment of strike
    ball_states: [BallState; 16],
}
```

> **The sign convention is part of the input.** `contact_offset_x` and `contact_offset_y` are
> *offsets from the centre of the cue ball*, in the units of §6.2, and the axes are the
> cue's own axes: `+y` is `up × direction` (the shooter's left) and `+z` is `up`. Reading
> `contact_offset_x` as "the shooter's right" inverts the english, and nothing else in the
> shot looks broken — the ball simply goes the other way round.

**Why a direction vector and not angles:** expressing the aim as degrees (or radians) forces the
deterministic core to call `sin`/`cos` to convert it, and those calls go to the platform libm,
which is the single largest source of cross-platform divergence (§11.4). Passing a
pre-normalized unit vector moves the trigonometry into the UI layer, where a one-bit difference
does not matter, and keeps the simulation core free of transcendental functions.

The same rule applies to any other angle that reaches the core (table geometry, pocket angles):
normalize once outside the deterministic boundary and pass vectors/coordinates.

**From this input, the physics computes:**
1. Initial velocity and angular velocity of the cue ball
2. Time evolution of all balls until they settle (all stationary)
3. Final state of all balls

### 12.4 Simulation Loop

**Event-driven (recommended, see §11.2):**

```rust
fn simulate_shot(input: ShotInput) -> GameState {
    // 1. Apply cue strike to cue ball
    let mut balls = apply_cue_strike(&input);
    
    // 2. Event loop: advance to the next event, resolve it, repeat
    let mut t = 0.0;
    while t < MAX_SIM_TIME && !all_balls_settled(&balls) {
        // For each ball, find the time of its next event:
        //   - motion-state change (slide -> roll, roll -> rest)
        //   - ball-ball contact, cushion contact, pocket, off-table
        // Take the earliest one; ties are broken by ball index (deterministic).
        let (dt, event) = next_event(&balls, t);
        
        // Advance every ball analytically to t + dt (closed form, section 3)
        advance_all(&mut balls, dt);
        t += dt;
        
        // Apply the event
        match event {
            Event::BallBall(a, b) => resolve_ball_ball(&mut balls, a, b),
            Event::Cushion(b)     => resolve_cushion(&mut balls, b),
            Event::Pocket(b)      => resolve_pocket(&mut balls, b),
            Event::OffTable(b)    => resolve_off_table(&mut balls, b),
            Event::StateChange(b) => switch_motion_state(&mut balls, b),
        }
    }
    
    GameState { balls, .. }
}
```

**Stepped alternative** (if the event-driven core is too much work for a first version):

```rust
fn simulate_shot(input: ShotInput) -> GameState {
    let mut balls = apply_cue_strike(&input);
    let fixed_dt = 1.0 / 240.0;
    let mut accumulator = 0.0;
    
    while !all_balls_settled(&balls) && accumulator < MAX_SIM_TIME {
        accumulator += fixed_dt;
        integrate_step(&mut balls, fixed_dt);
        
        resolve_ball_ball_collisions(&mut balls);
        resolve_cushion_collisions(&mut balls);
        resolve_pocket_collisions(&mut balls);
        resolve_ball_off_table(&mut balls);
        resolve_ball_slate_bounces(&mut balls);
    }
    
    GameState { balls, .. }
}
```

**Choosing the timestep:** the timestep is a property of the *stepped* architecture, not of
determinism. With the event-driven core there is no timestep at all, and the fixed timestep
that §11.1 describes is not needed for determinism either — determinism comes from the
closed-form segments and the explicit tie-break rule (§11.4). What a *stepped* integrator
does need is a step size that is fixed (so that the sequence of operations does not depend
on the frame rate) and small enough for spatial accuracy. 240 Hz is *not* justified by the
ball-ball contact time: collisions are modelled as instantaneous, so the 250 μs contact
duration never enters the integrator; what the timestep actually has to be small enough for
is *spatial* accuracy — that no ball may move far enough in one step to pass through another
ball or a cushion. At 240 Hz a 13 m/s break shot covers 5.4 cm per step, essentially a full
ball diameter, so discrete overlap tests will miss collisions at break speed.

So:
- with **event-driven** simulation, no timestep constraint applies at all;
- with a **stepped** integrator, use a *fixed* timestep and either continuous collision
  detection (swept TOI within each step, §11.3) or raise the rate — 1000 Hz gives 1.3 cm
  per step at 13 m/s, which is below a ball diameter but still not comfortable for a fast
  grazing hit.

**The time limit is a guard, not a physics parameter.** `MAX_SIM_TIME` (30 s) and the event
limit behind it exist so that a shot cannot run for ever — a Zeno sequence that the settle
threshold of §8.2 somehow missed, or a ball that never comes to rest. When the limit is
reached the shot has **not** finished, and the core says so: the state it returns is
reported as *settling*, not *settled*, and the balls are where they were at the limit. That
matters for the rules — a shot that is still settling is a shot that is still going, and
the game must not call it.

A limit that fires on a *legitimate* shot is a limit that is set too low, not a shot that
should be cut short. A ball rolling at the slowest speed the model admits takes
`v/(μ_r·g)` seconds to stop, which at `μ_r = 0.01` is about 10 s for a ball that is still
moving at 1 m/s — and the shot that contains it is not over. The 30 s limit is comfortably
above that; it is the Zeno case it is there for, not the slow roll.

---

## 13. Summary of Recommended Model

### 13.1 Recommended Models by Domain

| Domain | Recommended Model | Complexity | Accuracy |
|---|---|---|---|
| Ball-cloth | Analytical (Kiefl 2020, Section I) | Low | High |
| Ball-ball | Elastic + friction impulse (Kim 2024; Dr. Dave TP A-14) | Medium | High |
| Ball-cushion | Han (2005) or simplified game model | Low-Medium | Medium-High |
| Ball-cue | Impulse-based with squirt correction (Kim 2021) | Medium | High |
| Jump shots | Gravity only, closed form (drag and Magnus neglected, §7) | Low | Medium-High |
| Ball-slate | COR bounce (e_table = 0.5–0.7) | Low | Medium |
| Determinism | f64, event-driven core, platform-independent libm, lockstep | Medium | N/A |

### 13.2 Key Parameters (Summary)

| Parameter | Value |
|---|---|
| Ball radius R | 0.028575 m |
| Ball mass m | 0.170 kg (6 oz) |
| Ball COR (ball-ball) e | 0.95 |
| Ball-ball friction μ_ball | 0.06 at a 1 m/s slip — TP A.14's own fit value there, so the law in force is the published one **unscaled** (the law is speed-dependent, §4.4.5) |
| Sliding friction μ_s | 0.20 |
| Rolling friction μ_r | 0.01 |
| Cushion COR e_cushion | 0.75 |
| Cushion nose (contact) height h_nose | 36.3 mm (63.5% of ball dia) |
| → derived contact height ε | 0.270 R (= 7.73 mm) — an output, not an input (§5.1) |
| → derived contact tilt θ | 15.7° |
| Cushion nose tip radius | 6.35 mm (1/4") — shape only, not the impulse direction |
| Cushion sliding friction μ_cushion | 0.14 |
| Table COR e_table | 0.60 |
| Cue-ball friction μ_cue | 0.60 |
| Spin decay μ_sp | 0.0117 (a 10 rad/s² decay rate) |
| Pocket jaw COR e_jaw | 0.60 |
| Pocket jaw friction μ_jaw | 0.30 |
| Pocket shelf friction μ_slate | 0.15 |
| Pocket cut angle (corner / side) | 142° / 104° (WPA ±1°) |
| Slate settle speed | 0.02 m/s (the Zeno guard, §8.2) |
| Simulation timestep | none required (event-driven); 1/240–1/1000 s if stepped |
| Max simulation time | 30 seconds |
| Gravity g | 9.81 m/s² |
| Numeric type | f64 throughout |

### 13.3 Tuning Parameters and Calibration Order

Every value in the table above is a *starting point*. Some are well determined by measurement
(the friction coefficients, the CORs, the pocket angles) and some are not (the cushion contact
height, the jaw restitution, the squirt curve). Put the second group in the table config so it
can be fitted, and calibrate in this order — each step depends on the previous one:

| Order | Parameter | How to fit it | Observable |
|---|---|---|---|
| 1 | `μ_s`, `μ_r` | Dr. Dave's constants; confirm with a rolling ball's stopping distance | how far a 2 m/s ball rolls |
| 2 | `μ_sp` | watch a spinning ball decay | 5–15 rad/s² |
| 3 | `h_nose` (cushion nose height) | slow-motion video of a cushion hit | the hop height and the rebound angle |
| 4 | `e_cushion` | rebound speed off a cushion | speed loss on a 45° incidence |
| 5 | `e_jaw`, `μ_jaw`, `μ_slate` | pocket outcomes on a real table | which shots rattle and which hang up |
| 6 | squirt curve | high-speed video of off-centre hits | the deflection angle vs impact parameter |
| 7 | `e_ball`, `μ_ball` | separation angle and throw | 85°–89° separation, 1.45° throw at a 10° cut; at a 30° cut the throw runs from 2.9° (fast) to 4.65° (slow) |

Two defaults deserve an explicit note:

- **`ε` is derived, not tuned.** The table config carries the nose height
  (`h_nose = 36.3 mm`), and the contact height `ε = h_nose - R = 0.270 R` and the contact tilt
  `θ = 15.7°` follow from it (§5.1). Nothing to drift out of sync with the physics, and the
  model's predictions reproduce Dr. Dave's qualitative account of the rebound at 0.5 D, 0.635 D
  and 0.7 D. If the hop is missing or too big, the nose height is wrong, not a magic number.
- **`e_cushion = 0.75`** is the effective ball–rail restitution (Dr. Dave's 0.6–0.9), which
  already includes the energy lost in the cloth contact after the rail. Mathavan's 0.98 is the
  rail rubber alone; do not use it here unless the two contacts are modelled separately.

### 13.4 What to Skip (for a game)

- **Full soft-body cushion physics** (Mathavan 2010) — overkill for a game
- **Magnus effect in air** — negligible for pool ball sizes/speeds
- **Multi-ball simultaneous collisions** — resolve pairwise, ordered by (event time, ball index)
- **Cue flexion** — treat the cue as rigid (small effect on the ball state), but note that the
  *effective cue-end mass* (~12 g, not 454 g) is what matters for squirt
- **Cloth nap anisotropy** — negligible for game-level play
- **Temperature/humidity effects** — can be added as optional modifiers later

---

## 14. Validation Fixtures

The equations in this document are only worth as much as the tests that pin them down.
`tests/fixtures/` holds the acceptance criteria for the physics core: each file is a shot
specification plus the expected outcome, derived either from an independent source (measured
data or a published result) or from a closed-form limit of the model itself.

The fixtures are generated by `tests/fixtures/generate_fixtures.py`, which computes the
expectations from the same equations as this document and records the measured values they are
to be compared against. Regenerating them must never change a number that has been filled in
by hand (`squirt_curve`, `determinism_golden`) — those are marked `TO BE FILLED` / `TO BE
CALIBRATED` and are edited in place, not overwritten.

### 14.1 The fixtures

| Fixture | What it pins down | Independent source |
|---|---|---|
| `slide_to_roll` | the **sign** of the sliding-state angular velocity | closed form (§3.3) |
| `stun_separation_90` | the 90° rule, the normal components, the throw impulse | Kim 2024 (85°–89° separation) |
| `cut_throw_sweep` | the 1/7 sticking constant and the Coulomb cap | Alciatore TP A-14 measured throw |
| `cushion_rebound` | the Han cushion impulse model | Mathavan et al. 2010 invariants |
| `draw_stop_shot` | the draw/stop behaviour and the reversal threshold | closed form (§3.3) |
| `pocket_drop` | the pocket state machine, the rattle, the drop threshold | Dr. Dave, Pocket Effective Size and Center |
| `squirt_curve` | the squirt calibration curve | Cross 2008 (to be transcribed) |
| `determinism_golden` | bit-exact reproducibility of a full shot | golden value from the first implementation |

Two of them are worth calling out because they are the ones that catch real bugs:

- **`slide_to_roll`** is the regression test for the §3.3 sign error. It asserts that the slip
  reaches zero at exactly `t = 2·u₀/(7·μ_s·g)` *and* that `ω_y` is positive at that instant.
  A sign flip in the angular velocity breaks both.
- **`cut_throw_sweep`** compares the model against measured throw, at TP A.14's own three
  speeds (1, 3 and 7 mph), so that the model and the measurement beside it are the same
  shot. The two regimes have closed forms. In the *sticking* regime (cut angles below
  ≈ 18.8°) the throw is `atan(2·tan(cut)/(7·(1+e)))`, which gives 1.48° at a 10° cut against
  a measured 1.45° — agreement to 0.03°. In the *sliding* regime the Coulomb cap makes the
  throw `atan(μ_ball(u))`, where `u` is the slip speed, and because `μ_ball` falls with the
  slip speed (§4.4.5) the throw is speed-dependent. There the model sits up to 0.5° *below*
  the measurement, and the shortfall grows with the shot speed — small, and a slope, not
  the degree-wide offset earlier versions recorded.

> **The residual is small, and the argument that it could not be closed was wrong.** With
> the friction law used unscaled (`μ_ball = 0.06`, §13.2), the three measured columns at a
> 30° cut come out at 4.83° (the sticking limit; measured 4.65°), 4.26° (measured 4.55°)
> and 2.42° (measured 2.90°) — residuals of +0.2°, −0.3° and −0.5°. v1.7 claimed no law of
> the form `A + B·e^(−K·u)` could fit those columns and recorded a 0.6°–1.0° offset at every
> speed; both rested on a wrong premise, and §14.3 retracts them. The premise was the step
> from a measured throw to an implied friction coefficient, `μ = tan(throw)`, which holds
> only in the *sliding* regime: the slow column sits at the 4.83° sticking limit, where the
> throw does not depend on μ at all, so it yields only a *lower* bound — which TP A.14's
> fit clears. The rest of the old residual was the config: `μ_ball` shipped 17% below the
> fit it called the authority (§4.4.5).

### 14.2 Schema

```jsonc
{
  "id": "slide_to_roll",
  "title": "...",
  "purpose": "what bug this fixture is here to catch",
  "sources": ["..."],                 // where the expected values come from
  "setup": {
    "balls": [ { "id": "b0", "position": [x, y], "velocity": [vx, vy],
                 "angular_velocity": [wx, wy, wz] } ],
    "params": { "mu_s": 0.2 }         // config overrides for this shot only
  },
  "expectations": [
    { "quantity": "time_to_rolling", "expected": 0.1456, "tolerance": 1e-9, "unit": "s" }
  ],
  "invariants": [                     // properties that must hold, not just numbers
    { "quantity": "rebound_angle_independence", "statement": "...", "tolerance": 1e-9 }
  ],
  "known_deviations": [ "..." ]       // model limitations the fixture records on purpose
}
```

Conventions:

- `tolerance` is **absolute**, in the unit of the quantity. Use a tight tolerance for closed-form
  quantities (1e-9) and a looser one for measured comparisons (0.2°–0.5°).
- Positions are in metres, velocities in m/s, angles in degrees in the fixture (radians internally).
- Every fixture must be runnable without a table: the setup is the complete initial state.

### 14.3 Known deviations

The `cut_throw_sweep` fixture records the one place where the model is knowingly wrong.

**What the model does now.** The tangential impulse is capped at `μ_ball(u)·J_n`, and
`μ_ball` is not a constant: it is TP A.14's fit to Marlow's data,
`μ(v_rel) = 0.009951 + 0.108·e^(−0.77·v_rel)` (§4.4.5), used **unscaled** — the config's
default `μ_ball = 0.06` is the fit's own value at the 1 m/s reference slip. A slow shot
slips slowly, so it gets a larger friction coefficient and a larger throw — which is the
measured behaviour, and the reason the measured throw is 4.65° at a 30° cut on a slow shot
against 2.90° on a fast one.

**What is left.** A residual of at most half a degree, and it is not uniform. At the 30°
cut the model gives 4.83° (sticking), 4.26° and 2.42° against the measured 4.65°, 4.55°
and 2.90° — +0.2°, −0.3°, −0.5°. At 40°–60° cuts the shortfall stays between 0.03° and
0.48° and is again largest at the fast speeds. The speed dependence of the throw is
modelled correctly (the shape of the curve follows the slip speed); what is missing is a
few tenths of a degree at the fast end.

**What was claimed here in v1.7, and why it was wrong.** v1.7 recorded a 0.6°–1.0° offset
"at every speed and every cut angle" and argued it could not be fitted away: the three
measured columns at a 30° cut, it said, cannot be fitted by *any* law of the form
`A + B·e^(−K·u)` — their implied coefficients are 0.0813, 0.0796 and 0.0507 at slip speeds
of 0.224, 0.671 and 1.565 m/s, and the ratio bound says the measured ratio (0.061) cannot
reach the legal minimum (0.50). The ratio bound is correct for convex laws of that form,
but the implied coefficients were obtained from `μ = tan(throw)`, and that relation holds
only in the *sliding* regime. The slow column (4.65°) sits at the sticking limit
`atan(2·tan 30°/(7·1.95)) = 4.83°`: it is saturated, the throw there does not depend on μ
at all, and the column yields only a *lower* bound on the coefficient — which TP A.14's
fit (0.101 at that slip speed) clears. With the sticking branch taken into account, the
law reproduces all three columns to within half a degree, so the conclusion that "the
columns were not measured under one set of conditions" does not follow.

The rest of the v1.7 residual was self-inflicted: the config shipped `μ_ball = 0.05`
"at 1 m/s" — 16.6% below the fit's own 0.05996 — and §4.4.5 rescaled the whole law by
0.83 to match it, which alone accounted for about 0.7° in the sliding columns (with the
scaled law the model sat 1.0° and 0.9° below the middle and fast columns at the 30° cut,
while the slow column was already *above* the measurement — so even then the offset was
not "at every speed"). The config now carries the fit's own value.

**What used to be said here before that, and why it changed.** The 1.6 text said that
closing the gap meant coupling the ball-cloth friction into the ball-ball impulse, which
the 250 μs contact makes awkward. That was written before the speed dependence in TP A.14
was taken into account: with the friction coefficient itself a function of the slip speed
(and the config not silently rescaling it), the gap is half a degree. Coupling the cloth
friction into the impulse is still the remaining route to the last fraction of a degree,
and it is still awkward — but it is now a half-degree question, and it wants a measurement
(high-speed video of a cut throw) rather than a correction layered on the model.

### 14.4 Determinism fixture

`determinism_golden` specifies a full 16-ball break shot and a hash of the final state. The
first implementation writes the hash into the fixture; from then on it is the golden value. The
hash must be computed over the raw `f64` state — positions, velocities and angular velocities —
and must not include wall-clock time, hash-map iteration order or thread scheduling.

Run it on every platform you ship. A mismatch between x86-64 and wasm is almost always a
transcendental function that leaked into the core (§11.4), not an arithmetic difference.

**Where the golden hash has been confirmed.** The hash is currently an **x86-64** value, and
it has been confirmed on a second target: `i686-unknown-linux-gnu`, which agrees bit for
bit. That second target proves *word-size* portability — 32-bit against 64-bit — and
nothing more: Rust's `i686-unknown-linux-gnu` baseline includes SSE2, so it runs the same
64-bit floating-point instructions as x86-64 does. (x87's 80-bit intermediates are the
`i586-unknown-linux-gnu` target, which has not been run.) A genuinely different
floating-point path is `aarch64` — close to the shipping targets, and runnable under qemu —
and that is the next confirmation to make. `wasm32` is *not* a different arithmetic path
either: wasm floating point is strict IEEE-754, like SSE2, so its risk is the
transcendental one below rather than an arithmetic one. Neither target checks the
*transcendentals*, because both run the same `libm`; for that, the guarantee is structural —
the core calls `libm` and nothing else, and a test enforces it.

**The wasm baseline has not been established.** There is no wasm toolchain in the
environment this document was written in, so the hash has not been run on
`wasm32-unknown-unknown`. Until it has, the golden value is a baseline for one platform and
a *specification* for the others: when the wasm build exists, the first thing to do with it
is run this fixture, and the hash it produces is either the same number (and the model is
portable) or a different one (and the leak that §11.4 warns about is in the core, and the
portability test will find it).

### 14.5 What the fixtures do not check

The fixtures are the model's own equations, checked against each other and against whatever
published data exists. They are **self-consistency, not validation**: a fixture that the model
itself generated proves that the code implements this document and that the document's closed
forms agree with one another. It says nothing about whether the physics is right.

Four things are known to be unvalidated, and they are the ones to measure next:

| What | Why it is open | What would close it |
|---|---|---|
| The cushion hop (3.8 mm at the WPA nose height) | the model treats the cushion impulse and the slate rebound as *sequential*, and Mathavan treats the table contact as simultaneous; the hop scales with the difference | high-speed video of a ball off a rail whose nose height has been measured (`cushion_rebound`, `measured_hop`) |
| The break-speed cue ball lift | TP A.14's friction law is fitted at 1–7 mph and a break slips at 8–10 m/s, past the end of the data; Kim's estimate says 13 mm where the model says 0.3 mm (§4.5) | the cue ball's peak height after its first contact, from video, at a known speed (`cushion_rebound`, `measured_break_hop`) |
| The jaw parameters and the facing angles | the WPA specifies the pocket's *cut* angles and gives no drawing, so the facing angle is a reading (§9.1); and no published measurement of a jaw's restitution or friction was found | a table, and a record of which shots rattle and which hang up (`pocket_drop`) |
| The squirt curve | Cross (2008) has not been transcribed and the anchors are `null` | Cross's data, or high-speed video of off-centre hits (`squirt_curve`) |

Everything else in this document is either a closed form — which the fixtures check against
each other — a published result — which the fixtures check against the measurement — or an
assumption that is written down as an assumption.

---

## 15. References

1. **Kiefl, Evan (2020).** "The physics of pool/billiards." https://ekiefl.github.io/2020/04/24/pooltool-theory/
   - Comprehensive modern treatment of all five interaction domains
   - Includes full analytical equations for ball-cloth, ball-ball, ball-cushion, ball-air, ball-slate
   - Errata worksheet for Han (2005): https://ekiefl.github.io/images/pooltool/pooltool-theory/ball_cushion_inhwan_han.pdf
   - Note: the sliding-state angular velocity equation on that page carries a sign error (§3.3)

2. **Kim, Hyeong-Chan (2024).** "Collision of two spinning billiard balls and the role of table."
   arXiv:2402.13258; published in *New Physics: Sae Mulli* **74**(6), 621 (2024).
   - Modern research on ball-ball collisions with spin and table friction
   - Experimental values for COR (0.92–0.98) and friction (0.03–0.08); contact time 250–300 μs
   - Source of the vertical hop estimate `h = (μ·U)²/(2g)` (§4.5)

3. **Kim, Hyeong-Chan (2021/2022).** "Motions of a billiard ball after a cue stroke."
   arXiv:2104.11232; published in *New Physics: Sae Mulli* **72**, 208 (2022).
   - Complete theoretical analysis of the cue-ball collision
   - Squirt angle derivation, the rigid-cue vs. flexible-cue distinction, the effective cue-end mass
   - Miscue condition `μ_static ≥ tan φ = (b/R)/√(1-(b/R)²)`

4. **Han, Inhwan (2005).** "Dynamics in Carom and Three Cushion Billiards." Journal of Mechanical
   Science and Technology **19**(4), 976–984.
   - Analytical ball-cushion model (instantaneous, point contact, ε/θ geometry)

5. **Mathavan, S., Jackson, M.R. & Parkin, R.M. (2010).** "A theoretical analysis of billiard ball dynamics under cushion impacts." Proceedings of the Institution of Mechanical Engineers, Part C **224**(9), 1863–1873.
   - Most complete ball-cushion model (requires numerical solution)
   - Contact height h = 7R/5, cushion COR 0.98, sliding friction 0.14

6. **Dr. Dave Pool Info (drdavepoolinfo.com).** "Pool Physics Property Constants."
   https://drdavepoolinfo.com/faq/physics/physical-properties/ and
   https://drdavepoolinfo.com/faq/table/cushion-nose-height/
   - Authoritative reference for all physical property values
   - Also the source of the throw equation in TP A-14 and the throw data used in §4.4

7. **Alciatore, D.G. (2005, rev. 2025).** "TP A.14 — The effects of cut angle, speed, and spin
   on object ball throw." https://drdavepoolinfo.com/technical_proofs/new/TP_A-14.pdf
   - The source of the 1/7 sticking constant, of the two-dimensional slip vector used in
     §4.4.5, and of the measured throw data in §14.1
   - Also the source of the speed-dependent friction law
     `μ(v_rel) = 0.009951 + 0.108·e^(−0.77·v_rel)`, fitted to Marlow's data

8. **Alciatore, D.G. (2005).** "TP A.17 — The effect of the amount of friction on throw."
   https://billiards.colostate.edu/technical_proofs/new/TP_A-17.pdf
   - The throw equation of TP A.14 written out with the friction-speed relation, and the
     scale factor used to show the effect of the friction on the throw

9. **Cross, R. (2008).** "Cue and ball deflection (or 'squirt') in billiards." Am. J. Phys. 76, 205.
   - Experimental and theoretical analysis of squirt

10. **Petit, Régis (2004).** "Billiards: Theory of the Game." Chiron.
    - Complete 3D vector treatment of billiard physics
    - Covers horizontal and inclined cue strokes, jump shots

11. **Alciatore, David (Dr. Dave).** "The Amazing World of Billiards Physics," *Scientific American*
    (2007). https://billiards.colostate.edu/physics_articles/Alciatore_SCIAM_article_posted_version.pdf
    - Spin-induced throw, cut-induced throw, jump shot mechanics
    - Published throw equation (TP A-14), including the 1/7 sticking constant

12. **Gaffer on Games.** "Fix Your Timestep!" (2004).
    https://gafferongames.com/post/fix_your_timestep/
    - Industry standard for fixed-timestep simulation

13. **Gaffer on Games.** "Snapshot Interpolation" (2014).
    https://gafferongames.com/post/snapshot_interpolation/
    - Industry standard for client-side interpolation of server snapshots

14. **WPA Equipment Specifications.** https://wpapool.com/wp-content/uploads/2024/01/RECOMMENDED-EQUIPMENT-SPECIFICATIONS.pdf
    - Official tournament table, ball, and pocket dimensions
    - Section 9 gives the pocket openings, the vertical pocket angle and the *horizontal
      pocket cut angles* (142° for the corner pocket, 104° for the side pocket, each +1°),
      with no drawing and no definition of the reference lines — see §9.1

15. **BCA Equipment Specifications.** https://cdn.ymaws.com/bca-pool.com/resource/resmgr/imported/BCAEquipmentSpecifications_2008.pdf
    - Additional equipment specifications: its side-pocket cut angle is 103° ± 2°, and its
      corner shelf range is narrower than the WPA's

16. **Aramith General Specifications.** https://aramith.com/general-specifications/
    - Ball material and physical properties

17. **Nierhoff, T. et al. (2012).** "Strategic play for a pool-playing robot." IEEE Workshop on Advanced Robotics and its Social Impacts.
    - Robotic billiards implementation experience

18. **Wallace, R.E. & Schroeder, M.C. (1988).** "Analysis of billiard ball collisions in two dimensions." Am. J. Phys. 56, 815.
    - Classic paper on billiard ball collision physics

19. **Coriolis, G. (1835/2005).** "Théorie mathématique des effets du jeu de billard." English translation by D. Nadler.
    - Historical foundation of billiard physics (1835)

20. **Peskin, C.S. (2020).** "Collision of Billiard Balls in 3D with Spin and Friction." https://math.nyu.edu/~peskin/Collision_of_Billiard_Balls_in_3D_with_Spin_and_Friction.pdf
    - 3D collision model with spin and friction

---

*Document generated 2026-10-21. Physics values come from peer-reviewed papers (Han 2005;
Mathavan et al. 2010; Cross 2008; Wallace & Schroeder 1988; Kim 2021, 2024 — all published in
refereed journals, with the arXiv numbers given for the two Kim papers), official WPA/BCA
equipment specifications, and practitioner references (Dr. Dave's site, Kiefl's blog). The
practitioner references are not peer-reviewed and are used for empirical constants and for the
cushion model, where no simple peer-reviewed treatment exists.*

