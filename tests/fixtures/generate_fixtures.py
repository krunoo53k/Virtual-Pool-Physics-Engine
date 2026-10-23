#!/usr/bin/env python3
"""Generates tests/fixtures/*.json - acceptance criteria for the physics core.

Every expected value is computed here from the equations in docs/virtual-pool-physics.md
(or taken from a published measurement, which is marked with a "source"), so the fixtures
cannot drift away from the document.
"""
import json, math, os

R = 0.028575          # ball radius, m
D = 2*R                # ball diameter, m
M = 0.170             # ball mass, kg
G = 9.81
E_BALL = 0.95         # ball-ball COR
MU_BALL = 0.06        # ball-ball friction at the 1 m/s reference slip (TP A.14's own fit value)
MU_S = 0.20            # ball-cloth sliding friction
MU_CUSHION = 0.14      # cushion sliding friction
MU_R = 0.01            # ball-cloth rolling friction
NOSE_H = 0.0363          # nose (contact) height above the bed, m (63.5% of ball diameter)
NOSE_TIP_R = 0.00635     # nose tip radius, m (1/4") - shape only, not the impulse direction
EPS = NOSE_H - R         # derived contact height above the ball's centre, m
EPS_R = EPS/R            # ... in units of R
SIN_T = EPS/R            # sin(theta): tilt of the contact normal from the horizontal
COS_T = math.sqrt(1 - SIN_T**2)
I = (2/5)*M*R*R

OUT = os.path.dirname(os.path.abspath(__file__))


def write(name, data):
    path = os.path.join(OUT, name)
    with open(path, "w") as f:
        json.dump(data, f, indent=2)
        f.write("\n")
    print("wrote", path)


# ---------------------------------------------------------------- helpers
def add(a, b): return (a[0]+b[0], a[1]+b[1])
def sc(a, s): return (a[0]*s, a[1]*s)
def cr(w, r): return (-w*r[1], w*r[0])          # omega_z x r  (2D)
def mag(a): return math.hypot(a[0], a[1])
def ang(a): return math.atan2(a[1], a[0])
def rot(a, t): return (a[0]*math.cos(t)-a[1]*math.sin(t), a[0]*math.sin(t)+a[1]*math.cos(t))


def friction_at_speed(mu_ball, slip):
    """The ball-ball friction as a function of the sliding speed at the contact.

    Alciatore's TP A.14 fits Marlow's data with
    `mu(v) = 0.009951 + 0.108 * exp(-0.77 * v)`. The config's `mu_ball` is the
    coefficient at the reference slip speed of 1 m/s, so the law is normalised there
    (doc section 4.4.5).
    """
    slip = max(slip, 0.0)
    reference = 0.009951 + 0.108 * math.exp(-0.77)
    return mu_ball * (0.009951 + 0.108 * math.exp(-0.77 * slip)) / reference


def ball_ball(pA, pB, vA, vB, wA=0.0, wB=0.0, e=E_BALL, mu=MU_BALL):
    """Doc section 4.4.5: the impulse model in the tangent plane."""
    d = (pB[0]-pA[0], pB[1]-pA[1]); L = mag(d); n = (d[0]/L, d[1]/L); t = (-n[1], n[0])
    v_n = vA[0]*n[0]+vA[1]*n[1]
    rA = (R*n[0], R*n[1]); rB = (-R*n[0], -R*n[1])
    cA = add(vA, cr(wA, rA)); cB = add(vB, cr(wB, rB))
    # The slip is a vector in the tangent plane: horizontal along the rail and
    # vertical. The test balls here are spinless, so the vertical part is zero.
    u_t = (cA[0]-cB[0])*t[0] + (cA[1]-cB[1])*t[1]
    u_z = 0.0
    u = math.hypot(u_t, u_z)
    J_n = ((1+e)/2)*M*v_n
    mu_eff = friction_at_speed(mu, u)
    J_t = min(M*u/7, mu_eff*J_n) if u > 0.0 else 0.0
    # The friction always opposes the slip, so the impulse carries the sign of it.
    J_t = math.copysign(J_t, u_t) if u_t != 0.0 else 0.0
    vA2 = add(vA, add(sc(n, -J_n/M), sc(t, -J_t/M)))
    vB2 = add(vB, add(sc(n,  J_n/M), sc(t,  J_t/M)))
    wA2 = wA + (-rA[1]*(-J_t*t[0]) + rA[0]*(-J_t*t[1]))/I
    wB2 = wB + (-rB[1]*( J_t*t[0]) + rB[0]*( J_t*t[1]))/I
    return dict(vA=vA2, vB=vB2, wA=wA2, wB=wB2, Jn=J_n, Jt=J_t, u_t=u_t,
                n=n, t=t, sticking=(u > 0.0 and M*u/7 <= mu_eff*J_n))


def cushion3(v, w, sin_t=None, e=0.75, mu=0.14, z_c=None):
    """Doc section 5.1/5.3, in 3D. v, w are 3-vectors; the rail normal is +x.

    sin_t defaults to the value derived from the nose profile geometry
    (line of centres), which is how the simulator is meant to work. z_c is
    the ball's centre height at the contact; the default (R) is a ball
    resting on the bed.
    """
    if z_c is None:
        z_c = R
    if sin_t is None:
        sin_t = SIN_T
    th = math.asin(sin_t); ct = math.cos(th)
    # local frame: x = towards the rail, y = along the rail, z = up
    y = (0.0, 1.0, 0.0)
    # contact point relative to the ball's centre: R*sin(theta) above the centre,
    # on the inclined contact face
    rc = (R*ct, 0.0, R*math.sin(th))
    tangent = (math.sin(th), 0.0, -ct)
    nhat = (-ct, 0.0, -math.sin(th))

    def cross3(a, b):
        return (a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0])
    def dot3(a, b):
        return a[0]*b[0]+a[1]*b[1]+a[2]*b[2]
    def add3(a, b):
        return (a[0]+b[0], a[1]+b[1], a[2]+b[2])
    def sc3(a, s):
        return (a[0]*s, a[1]*s, a[2]*s)

    vc = add3(v, cross3(w, rc))
    s_x = dot3(vc, tangent)
    s_y = dot3(vc, y)
    slip = math.hypot(s_x, s_y)
    v_n = -dot3(v, nhat)
    Jn = (1+e)*M*v_n
    Jt = min((2/7)*M*slip, mu*Jn)
    sd = add3(sc3(tangent, s_x/slip), sc3(y, s_y/slip))
    imp = add3(sc3(nhat, Jn), sc3(sd, -Jt))
    v2 = add3(v, sc3(imp, 1/M))
    w2 = add3(w, sc3(cross3(rc, imp), 1/I))
    return dict(v=v2, w=w2, Jn=Jn, Jt=Jt, slip=slip,
                sticking=((2/7)*M*slip <= mu*Jn))


# ---------------------------------------------------------------- 1. slide -> roll
def fixture_slide():
    v0, w0 = 1.0, 0.0
    t = 2*v0/(7*MU_S*G)
    v = v0 - MU_S*G*t
    w = (5*MU_S*G/(2*R))*t
    x = v0*t - 0.5*MU_S*G*t*t
    return {
        "id": "slide_to_roll",
        "title": "Sliding ball reaches the rolling condition exactly at t = 2*u0/(7*mu_s*g)",
        "purpose": "Pins the sign of the sliding-state angular velocity (doc section 3.3). "
                   "A minus sign in the omega equation makes u grow instead of decaying, "
                   "which this fixture catches immediately.",
        "sources": ["doc section 3.3", "Kiefl 2020 (sign error in the published version)"],
        "setup": {
            "balls": [{"id": "b0", "position": [0.0, 0.0], "velocity": [1.0, 0.0],
                       "angular_velocity": [0.0, 0.0, 0.0]}],
            "params": {"mu_s": MU_S, "mu_r": MU_R},
        },
        "expectations": [
            {"quantity": "time_to_rolling", "expected": t, "tolerance": 1e-9,
             "unit": "s", "notes": "2*u0/(7*mu_s*g)"},
            {"quantity": "velocity_at_transition", "expected": v, "tolerance": 1e-9,
             "unit": "m/s"},
            {"quantity": "omega_y_at_transition", "expected": w, "tolerance": 1e-6,
             "unit": "rad/s", "notes": "must be POSITIVE (spin up towards forward roll)"},
            {"quantity": "x_at_transition", "expected": x, "tolerance": 1e-9, "unit": "m"},
            {"quantity": "rolling_condition_residual", "expected": 0.0, "tolerance": 1e-6,
             "unit": "rad/s", "notes": "omega_y - v/R must vanish at the transition"},
        ],
    }


# ---------------------------------------------------------------- 2. 90 degree rule
def fixture_stun():
    v0 = 2.0
    # head-on
    h = ball_ball((0.0, 0.0), (0.3, 0.0), (v0, 0.0), (0.0, 0.0))
    headon = {
        "quantity": "cue_speed_after_head_on", "expected": mag(h["vA"]), "tolerance": 1e-9,
        "unit": "m/s", "notes": "((1-e)/2)*v = 0.025*v; the cue ball creeps, it does not stop dead"},
    # 30 degree cut
    a = math.radians(30)
    r = ball_ball((0.0, 0.0), (0.3*math.cos(a), 0.3*math.sin(a)), (v0, 0.0), (0.0, 0.0))
    sep = math.degrees(ang(r["vB"]) - ang(r["vA"]))
    throw = math.degrees(ang(r["vB"]) - a)
    return {
        "id": "stun_separation_90",
        "title": "90 degree rule: cue and object ball separate at ~90 degrees (30 degree cut)",
        "purpose": "Validates the normal components of the ball-ball collision together with "
                   "the tangential (throw) impulse.",
        "sources": ["doc sections 4.2 and 4.4.5",
                    "Kim 2024: separation angle 85-89 degrees for e in [0.92, 0.98]"],
        "setup": {
            "balls": [
                {"id": "cue", "position": [0.0, 0.0], "velocity": [v0, 0.0],
                 "angular_velocity": [0.0, 0.0, 0.0]},
                {"id": "object", "position": [0.3*math.cos(a), 0.3*math.sin(a)],
                 "velocity": [0.0, 0.0], "angular_velocity": [0.0, 0.0, 0.0]},
            ],
            "params": {"e_ball": E_BALL, "mu_ball": MU_BALL},
        },
        "expectations": [
            headon,
            {"quantity": "separation_angle", "expected": sep, "tolerance": 0.5, "unit": "deg",
             "notes": "85-89 deg expected; frictionless elastic would be exactly 90"},
            {"quantity": "cue_speed_after", "expected": mag(r["vA"]), "tolerance": 1e-9,
             "unit": "m/s"},
            {"quantity": "object_speed_after", "expected": mag(r["vB"]), "tolerance": 1e-9,
             "unit": "m/s"},
            {"quantity": "object_throw_angle", "expected": throw, "tolerance": 0.2, "unit": "deg",
             "notes": "object ball is thrown toward the cue ball's post-collision direction"},
            {"quantity": "momentum_residual", "expected": 0.0, "tolerance": 1e-9,
             "unit": "kg*m/s", "notes": "sum of final momenta minus initial"},
        ],
    }


# ---------------------------------------------------------------- 3. throw sweep
ALCIATORE_THROW = {   # measured object-ball throw, degrees (cut -> slow, medium, fast)
    10: (1.45, 1.45, 1.45),
    30: (4.65, 4.55, 2.90),
    40: (5.55, 4.10, 2.35),
    50: (5.40, 3.75, 1.95),
    60: (5.25, 3.50, 1.70),
}

# The speeds of TP A.14's own throw measurements: 1, 3 and 7 mph.
ALCIATORE_SPEEDS = {"slow": 0.4472, "medium": 1.3411, "fast": 3.1293}


def throw_at(cut, speed):
    """The model's object-ball throw at a cut angle and a cue ball speed."""
    a = math.radians(cut)
    r = ball_ball((0.0, 0.0), (0.3*math.cos(a), 0.3*math.sin(a)),
                  (speed, 0.0), (0.0, 0.0))
    return math.degrees(ang(r["vB"]) - a), r["sticking"]


def fixture_throw():
    v0 = 2.0
    rows = []
    # Every row is at one of TP A.14's own speeds, so that the model and the
    # measurement it is compared against are the same shot. A model row at one
    # speed next to a measured column from another speed is not a comparison.
    for cut in (10, 30, 40, 50, 60):
        measured = ALCIATORE_THROW.get(cut)
        for (name, speed) in ALCIATORE_SPEEDS.items():
            throw, sticking = throw_at(cut, speed)
            index = ("slow", "medium", "fast").index(name)
            rows.append({
                "cut_angle": cut,
                "cue_speed": speed,
                "measured_speed": name,
                "model_throw_deg": round(throw, 4),
                "regime": "sticking" if sticking else "sliding",
                "measured_throw_deg": measured[index] if measured else None,
            })
    # The model's own sweep at a fixed speed, as a regression value.
    sweep = []
    for cut in (10, 20, 30, 40, 50, 60):
        throw, sticking = throw_at(cut, v0)
        sweep.append({
            "cut_angle": cut,
            "cue_speed": v0,
            "model_throw_deg": round(throw, 4),
            "regime": "sticking" if sticking else "sliding",
        })
    return {
        "id": "cut_throw_sweep",
        "title": "Object-ball throw versus cut angle and shot speed",
        "purpose": "Validates the 1/7 sticking constant, the Coulomb cap on the tangential "
                   "impulse and the speed-dependent friction coefficient. Every row is at one "
                   "of TP A.14's own speeds, so that the model and the measurement beside it "
                   "are the same shot. Also records the size of the residual against the "
                   "measurement (see known_deviations).",
        "sources": ["Alciatore, 'The Amazing World of Billiards Physics' (2007), TP A-14 "
                    "throw data", "doc section 4.4.5"],
        "setup": {
            "balls": [
                {"id": "cue", "position": [0.0, 0.0], "velocity": [v0, 0.0],
                 "angular_velocity": [0.0, 0.0, 0.0]},
                {"id": "object", "position": [0.3, 0.0], "velocity": [0.0, 0.0],
                 "angular_velocity": [0.0, 0.0, 0.0]},
            ],
            "params": {"e_ball": E_BALL, "mu_ball": MU_BALL,
                       "note": "object ball placed along the cut-angle bisector per row; the "
                               "cue ball is spinless, so the slip is purely horizontal and "
                               "the slip speed is the cue ball's speed times sin(cut)"},
        },
        "expectations": rows,
        "model_sweep_at_2ms": sweep,
        "known_deviations": [
            "The model is below the measured throw by at most 0.5 degrees over the whole "
            "sweep (the worst row is a 30 degree cut at 7 mph, at 0.48 degrees), and the "
            "shortfall grows with the shot speed at a fixed cut angle. In the sticking "
            "regime (10 degrees) it agrees with the measurement to 0.03 degrees.",
            "An earlier version of this fixture recorded a degree-wide offset and claimed "
            "the three columns at a 30 degree cut could not be fitted by any law of the "
            "form A + B exp(-K u). That claim turned the measured throw into an implied "
            "friction coefficient with mu = tan(throw), which is valid only in the sliding "
            "regime: the slow column (4.65 degrees) sits at the 4.83 degree sticking "
            "limit, where the throw does not depend on the friction coefficient at all, so "
            "it gives only a LOWER bound on it. Taking the sticking branch into account, "
            "TP A.14's own law reproduces all three columns to within half a degree (doc "
            "section 14.3).",
            "The measured throw depends on the cue ball's spin as well as its speed, and the "
            "published figures do not carry every shot condition (doc section 14.3).",
        ],
    }


# ---------------------------------------------------------------- 4. cushion rebound
def fixture_cushion():
    """Incidence angle is measured from the rail normal: 0 deg is a square hit."""
    rows = []
    for inc in (10, 20, 30, 40, 50, 60, 70, 80, 90):
        v0 = 2.0
        th = math.radians(inc)
        v = (v0*math.cos(th), v0*math.sin(th), 0.0)
        w = (-v[1]/R, v[0]/R, 0.0)        # rolling: omega = (k x v)/R
        r = cushion3(v, w, EPS_R)
        # rebound angle measured from the direction away from the rail (-x)
        beta = math.degrees(math.atan2(abs(r["v"][1]), -r["v"][0]))
        rows.append({
            "incidence_angle": inc,
            "rebound_speed": round(math.hypot(r["v"][0], r["v"][1]), 6),
            "rebound_angle": round(beta, 4),
            "rebound_vz": round(r["v"][2], 6),
            "sticking": r["sticking"],
        })
    # vertical flip: the same square hit at different effective contact heights
    flip = []
    v0 = 2.0
    v = (v0, 0.0, 0.0); w = (0.0, v0/R, 0.0)
    e_table = 0.60
    # nose heights as fractions of the ball diameter; the WPA spec is 0.635 D
    for label, frac_d in (("0.500 D (at the ball's centre)", 0.500),
                          ("0.570 D (the vertical flip)",
                           0.5*(1 + MU_CUSHION/math.sqrt(1+MU_CUSHION**2))),
                          ("0.635 D (the WPA spec)", 0.635),
                          ("0.700 D (centre of percussion)", 0.700)):
        eps_R = frac_d*D/R - 1.0
        r = cushion3(v, w, eps_R)
        vz = r["v"][2]
        # if the ball is pressed into the slate it rebounds with e_table; if it is launched
        # upward it simply flies its ballistic arc
        up = vz if vz > 0 else e_table*abs(vz)
        flip.append({
            "nose_height_over_D": frac_d,
            "eps_over_R": round(eps_R, 6),
            "rebound_vz": round(vz, 6),
            "direction": "up" if vz > 0 else "down",
            "hop_height_mm": round((up**2)/(2*G)*1000, 3),
        })
    # a ball that is not resting on the bed when it reaches the rail: the contact
    # geometry, and therefore the sign of the vertical impulse, changes
    regimes = []
    v0 = 2.0
    rail_top = NOSE_H + 0.015          # rail top edge, ~15 mm above the nose
    for label, z_c in (("resting on the bed", R),
                       ("lifted 20 mm (popped by a ball-ball collision)", R + 0.020),
                       ("lifted 40 mm", R + 0.040)):
        lowest = z_c - R
        if lowest > rail_top:
            regime = "clear of the rail"
        elif lowest > NOSE_H:
            regime = "rail structure"
        else:
            regime = "nose rubber"
        regimes.append({
            "case": label,
            "ball_centre_height_mm": round(z_c*1000, 3),
            "lowest_point_mm": round(lowest*1000, 3),
            "regime": regime,
        })

    return {
        "id": "cushion_rebound",
        "title": "Cushion rebound for a rolling, spinless ball",
        "purpose": "Validates the Han impulse model (doc section 5.1). The incidence angle is "
                   "measured from the rail normal, so 0 deg is a square hit. The second block "
                   "pins the vertical flip between the normal impulse (into the slate) and the "
                   "friction impulse (upward), which is what produces the hop.",
        "sources": ["doc sections 5.1 and 8.1", "Mathavan et al. 2010, Figs. 8-10"],        "setup": {
            "balls": [{"id": "b0", "position": [0.5, 0.5],
                       "velocity": [2.0, 0.0], "angular_velocity": [0.0, 2.0/R, 0.0]}],
            "params": {"nose_height": NOSE_H, "nose_tip_radius": NOSE_TIP_R,
                       "e_cushion": 0.75, "mu_cushion": MU_CUSHION,
                       "derived_eps_over_R": EPS_R,
                       "derived_tilt_deg": math.degrees(math.asin(SIN_T))},
            "note": "the incoming velocity makes `incidence_angle` with the rail normal (+x); "
                    "the ball is rolling, so omega = (k x v)/R",
        },
        "expectations": rows,
        "vertical_flip": flip,
        "rail_regimes": regimes,
        "measured_hop": {
            "quantity": "hop_height_mm",
            "expected": None,
            "tolerance": 0.5,
            "unit": "mm",
            "notes": "TO BE CALIBRATED: the hop height of a ball off the rail, measured from "
                     "high-speed video, at a known nose height. The 3.8 mm the model predicts "
                     "at the WPA spec height is a model output and not a measurement: it "
                     "depends on treating the cushion impulse and the slate rebound as "
                     "sequential, and Mathavan treats the table contact as simultaneous "
                     "(doc section 5.1). Until this is measured, the hop numbers in the "
                     "document are the model talking to itself.",
        },
        "measured_break_hop": {
            "quantity": "cue_ball_hop_height_mm",
            "expected": None,
            "tolerance": 1.0,
            "unit": "mm",
            "notes": "TO BE CALIBRATED: the peak height of the cue ball after its first "
                     "contact in a break, from high-speed video, at a known speed. No "
                     "published measurement was found. The model gives about 0.3 mm at a "
                     "10 m/s contact, and that number is an extrapolation of TP A.14's "
                     "friction law past the end of its data (doc section 4.5): Kim's "
                     "constant-friction estimate gives 13 mm for the same collision, and "
                     "the two cannot both be right.",
        },
        "invariants": [
            {"quantity": "rebound_angle_independence",
             "statement": "the rebound angle is identical for incident speeds of 1, 2 and 4 m/s "
                          "at the same incidence angle (Mathavan Fig. 8)",
             "tolerance": 1e-9, "unit": "deg"},
            {"quantity": "max_speed_loss_near_40deg",
             "statement": "the speed loss is largest for incidence angles around 40 deg "
                          "(Mathavan, section 4)",
             "tolerance": 0.02, "unit": "m/s"},
            {"quantity": "vertical_flip_threshold",
             "statement": "the outgoing vertical velocity changes sign at "
                          "eps_c/R = mu_cushion/sqrt(1 + mu_cushion^2) = 0.139 for "
                          "mu_cushion = 0.14; above it the ball is pressed into the slate, "
                          "below it the ball is launched upward. The WPA reference profile "
                          "derives eps = 0.270 R and must therefore press the ball into "
                          "the slate",
             "tolerance": 1e-9, "unit": "m/s"},
            {"quantity": "eps_derived_from_nose_height",
             "statement": "the contact height comes from the specified nose height: "
                          "eps = h_nose - R, so it is an output of the table spec rather than "
                          "a tuned input",
             "tolerance": 1e-9, "unit": "m"},
            {"quantity": "eps_independent_of_approach_angle",
             "statement": "eps depends only on the nose height: the contact normal has no "
                          "component along the rail, so a 40 degree approach gives the same "
                          "eps and the same normal tilt as a square hit",
             "tolerance": 1e-9, "unit": "m"},
            {"quantity": "nose_height_changes_the_rebound",
             "statement": "the specified nose height is what sets the vertical impulse: at "
                          "0.500 D the cushion launches the ball upward (+0.49 m/s at 2 m/s) "
                          "and at 0.635 D it presses the ball into the slate (-0.44 m/s). "
                          "Reproducing both is the check that the contact height is the "
                          "nose height",
             "tolerance": 1e-9, "unit": "m/s"},
            {"quantity": "rail_regimes_are_distinct",
             "statement": "the solver must branch on the regime: nose rubber below "
                          "h_nose, rail structure between h_nose and the rail top (~15 mm "
                          "higher), and no contact above the rail top. The rubber model must "
                          "not be extrapolated into the rail structure",
             "tolerance": 0},
        ],
    }


# ---------------------------------------------------------------- 5. draw / stop
def fixture_draw():
    v_after = 0.025*2.0                     # cue ball speed after a head-on collision, e = 0.95
    cases = []
    for name, omega_y in (("follows_forward", -2.0), ("strong_draw", -100.0)):
        u0 = v_after - R*omega_y
        t_slide = 2*u0/(7*MU_S*G)
        v_end = v_after - MU_S*G*t_slide
        cases.append({
            "case": name,
            "omega_y_at_impact": omega_y,
            "velocity_at_impact": v_after,
            "slip_at_impact": u0,
            "time_sliding_ends": t_slide,
            "velocity_when_sliding_ends": v_end,
            "direction": "backward" if v_end < 0 else "forward",
        })
    # the analytic reversal threshold: the ball stops within the sliding phase iff
    # u0 > 3.5 * v, i.e. R*|omega_y| > 2.5 * v
    threshold = 2.5*v_after
    return {
        "id": "draw_stop_shot",
        "title": "Draw shot: the cue ball reverses only when the backspin dominates",
        "purpose": "Pins the draw/stop behaviour that follows from the sliding-state equations, "
                   "including the analytic reversal threshold.",
        "sources": ["doc section 3.3"],
        "setup": {
            "balls": [
                {"id": "cue", "position": [0.0, 0.0], "velocity": [0.05, 0.0],
                 "angular_velocity": [0.0, -100.0, 0.0]},
                {"id": "object", "position": [0.3, 0.0], "velocity": [0.0, 0.0],
                 "angular_velocity": [0.0, 0.0, 0.0]},
            ],
            "params": {"mu_s": MU_S, "e_ball": E_BALL},
            "note": "the cue ball state is the state immediately after the head-on collision; "
                    "it then evolves on the cloth",
        },
        "expectations": cases,
        "invariants": [
            {"quantity": "reversal_threshold_R_omega_y",
             "expected": threshold, "tolerance": 1e-9, "unit": "m/s",
             "statement": "the cue ball reverses iff R*|omega_y| > 2.5 * v_at_impact "
                          "(derived from u0 > 3.5*v in the sliding phase)"},
        ],
    }


# ---------------------------------------------------------------- 6. pocket
def fixture_pocket():
    mu_slate = 0.15
    shelf_corner = 1.75*0.0254
    v_min = math.sqrt(2*mu_slate*G*shelf_corner)
    return {
        "id": "pocket_drop",
        "title": "Pocket capture conditions",
        "purpose": "Validates the pocket state machine (doc section 9): the drop threshold, "
                   "the rattle-out case and the minimum speed to reach the drop.",
        "sources": ["doc section 9", "Dr. Dave, 'Pocket Effective Size and Center'"],
        "setup": {
            "balls": [{"id": "b0", "position": [1.0, 1.0], "velocity": [1.5, 0.0],
                       "angular_velocity": [0.0, 1.5/R, 0.0]}],
            "params": {"mu_slate": mu_slate, "shelf_depth_corner": shelf_corner,
                       "e_jaw": 0.60, "mu_jaw": 0.30},
            "note": "the ball is placed on the centre line of a corner pocket and rolls "
                    "straight in at 1.5 m/s",
        },
        "expectations": [
            {"quantity": "outcome", "expected": "POCKETED", "tolerance": 0,
             "notes": "a ball entering on the centre line must drop"},
            {"quantity": "minimum_speed_to_reach_drop", "expected": v_min,
             "tolerance": 1e-9, "unit": "m/s",
             "notes": "sqrt(2*mu_slate*g*shelf_depth); slower balls hang up on the shelf"},
        ],
        "invariants": [
            {"quantity": "no_capture_at_the_mouth",
             "statement": "a ball whose centre merely crosses the mouth line is NOT pocketed; "
                          "the centre must cross the drop line"},
            {"quantity": "effective_center_shift",
             "statement": "the ideal aim point of a corner pocket shifts with entry angle and "
                          "speed by up to ~0.6 inch (Dr. Dave)",
             "tolerance": 0.15, "unit": "in"},
            {"quantity": "rattle_determinism",
             "statement": "a ball that rattles in the jaws must produce the same outcome on "
                          "every run and on every platform (no wall-clock, no hash ordering)",
             "tolerance": 0},
        ],
    }


# ---------------------------------------------------------------- 7. squirt
def fixture_squirt():
    return {
        "id": "squirt_curve",
        "title": "Squirt angle versus impact parameter (calibration target)",
        "purpose": "The squirt model is a calibration curve, not a closed form. This fixture "
                   "fixes the shape and the anchors; the amplitude is a config parameter that "
                   "playtesting or measurement sets.",
        "sources": ["Cross 2008 (experimental data, to be transcribed)",
                    "Kim 2021: squirt increases almost linearly for b < 0.5R"],
        "setup": {
            "balls": [{"id": "cue", "position": [0.0, 0.0], "velocity": [0.0, 0.0],
                       "angular_velocity": [0.0, 0.0, 0.0]}],
            "params": {"cue_speed": 2.0, "effective_end_mass": 0.012},
            "note": "the cue ball is struck at a range of impact parameters b; the squirt "
                    "angle is the deflection of the resulting velocity from the cue axis",
        },
        "expectations": [
            {"b_over_R": 0.0, "squirt_deg": 0.0, "tolerance": 1e-9,
             "notes": "a centre-ball hit has no squirt"},
            {"b_over_R": 0.1, "squirt_deg": None, "tolerance": 0.3,
             "notes": "TO BE CALIBRATED from Cross 2008 or high-speed video"},
            {"b_over_R": 0.2, "squirt_deg": None, "tolerance": 0.3,
             "notes": "TO BE CALIBRATED"},
            {"b_over_R": 0.3, "squirt_deg": None, "tolerance": 0.3,
             "notes": "TO BE CALIBRATED"},
            {"b_over_R": 0.4, "squirt_deg": None, "tolerance": 0.3,
             "notes": "TO BE CALIBRATED"},
            {"b_over_R": 0.5, "squirt_deg": None, "tolerance": 0.3,
             "notes": "TO BE CALIBRATED; roughly 1-5 deg at maximum english"},
        ],
        "invariants": [
            {"quantity": "squirt_monotonic", "statement": "squirt increases monotonically with "
             "the impact parameter and vanishes at b = 0"},
            {"quantity": "squirt_shape", "statement": "the curve is nearly linear up to "
             "b = 0.5R and flattens beyond (Kim 2021)"},
            {"quantity": "squirt_direction", "statement": "the ball squirts AWAY from the side "
             "of the ball that was struck"},
        ],
    }


# ---------------------------------------------------------------- 8. determinism
def fixture_determinism():
    # The golden hash is written by hand, by the first implementation, and it must
    # survive a regeneration (doc section 14). The generator never overwrites it: it
    # reads the value that is already in the file and writes it back.
    golden = None
    notes = None
    path = os.path.join(OUT, "determinism_golden.json")
    if os.path.exists(path):
        with open(path) as handle:
            existing = json.load(handle)
        for entry in existing.get("expectations", []):
            if entry.get("quantity") == "final_state_hash":
                golden = entry.get("expected")
                notes = entry.get("notes")
    return {
        "id": "determinism_golden",
        "title": "Golden-shot determinism check",
        "purpose": "The whole point of the architecture: identical inputs must produce "
                   "identical state, bit for bit, on every run and every platform.",
        "sources": ["doc section 11.4"],
        "setup": {
            # The foot spot of a 9 ft table is 25" (0.635 m) from the foot rail and on
            # the centre line of the width, so the break starts at (0.635, 0.635) with
            # the rack around it. (The rack used to be placed at y = 1.270, which is
            # the far rail: the balls were off the table and the shot was not a
            # break at all.)
            "balls": [
                {"id": "cue", "position": [0.635, 0.635], "velocity": [0.0, 0.0],
                 "angular_velocity": [0.0, 0.0, 0.0]},
            ] + [
                {"id": f"obj{n}", "position": [0.635 + 0.06*(n % 4), 0.635 + 0.06*(n // 4)],
                 "velocity": [0.0, 0.0], "angular_velocity": [0.0, 0.0, 0.0]}
                for n in range(1, 16)
            ],
            "shot": {"cue_speed": 8.0, "cue_direction": [0.0, 1.0],
                     "contact_offset": [0.0, 0.0]},
            "params": {"max_sim_time": 30.0},
        },
        "expectations": [
            {"quantity": "final_state_hash", "expected": golden, "tolerance": 0,
             "notes": notes or ("TO BE FILLED: the first implementation writes the hash of "
                                "the final state here, and it becomes the golden value")},
            {"quantity": "hash_stability", "statement": "the hash must be identical across "
             "repeated runs, across thread counts, and across x86-64 / ARM / wasm targets",
             "tolerance": 0},
            {"quantity": "event_order_stability", "statement": "the sequence of resolved events "
             "must be identical between runs; equal-time events are broken by ball index",
             "tolerance": 0},
        ],
        "notes": [
            "The hash must be computed over the raw f64 state (position, velocity, angular "
            "velocity) - not over a formatted or rounded representation.",
            "Do not include wall-clock time, iteration order of any hash map, or thread "
            "scheduling in the hashed state.",
        ],
    }


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    write("slide_to_roll.json", fixture_slide())
    write("stun_separation_90.json", fixture_stun())
    write("cut_throw_sweep.json", fixture_throw())
    write("cushion_rebound.json", fixture_cushion())
    write("draw_stop_shot.json", fixture_draw())
    write("pocket_drop.json", fixture_pocket())
    write("squirt_curve.json", fixture_squirt())
    write("determinism_golden.json", fixture_determinism())
