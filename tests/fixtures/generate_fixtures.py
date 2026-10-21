#!/usr/bin/env python3
"""Generates tests/fixtures/*.json - acceptance criteria for the physics core.

Every expected value is computed here from the equations in docs/virtual-pool-physics.md
(or taken from a published measurement, which is marked with a "source"), so the fixtures
cannot drift away from the document.
"""
import json, math, os

R = 0.028575          # ball radius, m
M = 0.170             # ball mass, kg
G = 9.81
E_BALL = 0.95         # ball-ball COR
MU_BALL = 0.05        # ball-ball friction
MU_S = 0.20            # ball-cloth sliding friction
MU_CUSHION = 0.14      # cushion sliding friction
MU_R = 0.01            # ball-cloth rolling friction
NOSE_APEX = 0.0363       # nose apex above the bed, m (63.5% of ball diameter)
NOSE_ARC_R = 0.00635     # nose tip radius, m (1/4")
DELTA = NOSE_APEX - NOSE_ARC_R - R      # vertical offset: ball centre -> nose arc centre
EPS_R = (R * DELTA/(R + NOSE_ARC_R))/R  # derived contact height above the ball centre, in R
SIN_T = DELTA/(R + NOSE_ARC_R)          # tilt of the contact normal (sin theta)
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


def ball_ball(pA, pB, vA, vB, wA=0.0, wB=0.0, e=E_BALL, mu=MU_BALL):
    """Doc section 5.4.5."""
    d = (pB[0]-pA[0], pB[1]-pA[1]); L = mag(d); n = (d[0]/L, d[1]/L); t = (-n[1], n[0])
    v_n = vA[0]*n[0]+vA[1]*n[1]
    rA = (R*n[0], R*n[1]); rB = (-R*n[0], -R*n[1])
    cA = add(vA, cr(wA, rA)); cB = add(vB, cr(wB, rB))
    u_t = (cA[0]-cB[0])*t[0] + (cA[1]-cB[1])*t[1]
    J_n = ((1+e)/2)*M*v_n
    J_t = math.copysign(min(M*abs(u_t)/7, mu*J_n), u_t)
    vA2 = add(vA, add(sc(n, -J_n/M), sc(t, -J_t/M)))
    vB2 = add(vB, add(sc(n,  J_n/M), sc(t,  J_t/M)))
    wA2 = wA + (-rA[1]*(-J_t*t[0]) + rA[0]*(-J_t*t[1]))/I
    wB2 = wB + (-rB[1]*( J_t*t[0]) + rB[0]*( J_t*t[1]))/I
    return dict(vA=vA2, vB=vB2, wA=wA2, wB=wB2, Jn=J_n, Jt=J_t, u_t=u_t,
                n=n, t=t, sticking=(M*abs(u_t)/7 <= mu*J_n))


def cushion3(v, w, sin_t=None, e=0.75, mu=0.14):
    """Doc section 5.1/5.3, in 3D. v, w are 3-vectors; the rail normal is +x.

    sin_t defaults to the value derived from the nose profile geometry
    (line of centres), which is how the simulator is meant to work.
    """
    if sin_t is None:
        sin_t = SIN_T
    th = math.asin(sin_t); ct = math.cos(th)
    # local frame: x = towards the rail, y = along the rail, z = up
    y = (0.0, 1.0, 0.0)
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
        "purpose": "Pins the sign of the sliding-state angular velocity (doc section 4.3). "
                   "A minus sign in the omega equation makes u grow instead of decaying, "
                   "which this fixture catches immediately.",
        "sources": ["docs section 4.3", "Kiefl 2020 (sign error in the published version)"],
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
        "sources": ["doc sections 5.2 and 5.4.5",
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


def fixture_throw():
    v0 = 2.0
    rows = []
    for cut in (10, 20, 30, 40, 50, 60):
        a = math.radians(cut)
        r = ball_ball((0.0, 0.0), (0.3*math.cos(a), 0.3*math.sin(a)), (v0, 0.0), (0.0, 0.0))
        throw = math.degrees(ang(r["vB"]) - a)
        measured = ALCIATORE_THROW.get(cut)
        rows.append({
            "cut_angle": cut,
            "model_throw_deg": round(throw, 4),
            "regime": "sticking" if r["sticking"] else "sliding",
            "measured_throw_deg": {
                "slow": measured[0] if measured else None,
                "medium": measured[1] if measured else None,
                "fast": measured[2] if measured else None,
            },
        })
    return {
        "id": "cut_throw_sweep",
        "title": "Object-ball throw versus cut angle",
        "purpose": "Validates the 1/7 sticking constant and the Coulomb cap on the tangential "
                   "impulse. Also records a known model limitation (see known_deviations).",
        "sources": ["Alciatore, 'The Amazing World of Billiards Physics' (2007), TP A-14 "
                    "throw data", "doc section 5.4.5"],
        "setup": {
            "balls": [
                {"id": "cue", "position": [0.0, 0.0], "velocity": [v0, 0.0],
                 "angular_velocity": [0.0, 0.0, 0.0]},
                {"id": "object", "position": [0.3, 0.0], "velocity": [0.0, 0.0],
                 "angular_velocity": [0.0, 0.0, 0.0]},
            ],
            "params": {"e_ball": E_BALL, "mu_ball": MU_BALL,
                       "note": "object ball placed along the cut-angle bisector per row"},
        },
        "expectations": rows,
        "known_deviations": [
            "The model caps the tangential impulse at mu_ball*J_n, which gives a constant "
            "throw angle of atan(mu_ball) = 2.86 deg once the balls start slipping "
            "(cut angles above ~18.8 deg). The measured throw is speed dependent and larger "
            "on slow shots (up to 5.5 deg at a 40 deg cut) because the table's static "
            "friction during the collision raises the effective friction. This is the "
            "correction described by Kim 2024 and is not implemented.",
            "In the sticking regime (cut angles below ~18.8 deg) the model predicts "
            "atan(2*tan(cut)/(7*(1+e))), which gives 1.48 deg at a 10 deg cut against a "
            "measured 1.45 deg - agreement to 0.03 deg.",
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
    # the first row is the reference profile: eps is derived from the nose geometry
    for label, eps_R in (("WPA reference nose (derived)", EPS_R),
                         ("flat face at 0.10R", 0.10),
                         ("flip threshold", MU_CUSHION/math.sqrt(1+MU_CUSHION**2)),
                         ("flat face at 0.15R", 0.15),
                         ("nose apex taken as contact height", 0.27),
                         ("Mathavan geometry", 0.40)):
        r = cushion3(v, w, eps_R)
        vz = r["v"][2]
        # if the ball is pressed into the slate it rebounds with e_table; if it is launched
        # upward it simply flies its ballistic arc
        up = vz if vz > 0 else e_table*abs(vz)
        flip.append({
            "profile": label,
            "eps_over_R": round(eps_R, 6),
            "rebound_vz": round(vz, 6),
            "direction": "up" if vz > 0 else "down",
            "hop_height_mm": round((up**2)/(2*G)*1000, 3),
        })
    return {
        "id": "cushion_rebound",
        "title": "Cushion rebound for a rolling, spinless ball",
        "purpose": "Validates the Han impulse model (doc section 5.1). The incidence angle is "
                   "measured from the rail normal, so 0 deg is a square hit. The second block "
                   "pins the vertical flip between the normal impulse (into the slate) and the "
                   "friction impulse (upward), which is what produces the hop.",
        "sources": ["doc sections 5.1 and 8.1", "Mathavan et al. 2010, Figs. 8-10"],
        "setup": {
            "balls": [{"id": "b0", "position": [0.5, 0.5],
                       "velocity": [2.0, 0.0], "angular_velocity": [0.0, 2.0/R, 0.0]}],
            "params": {"nose_apex_height": NOSE_APEX, "nose_arc_radius": NOSE_ARC_R,
                       "e_cushion": 0.75, "mu_cushion": MU_CUSHION,
                       "derived_eps_over_R": EPS_R},
            "note": "the incoming velocity makes `incidence_angle` with the rail normal (+x); "
                    "the ball is rolling, so omega = (k x v)/R",
        },
        "expectations": rows,
        "vertical_flip": flip,
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
                          "derives eps = 0.039 R and must therefore launch the ball upward",
             "tolerance": 1e-9, "unit": "m/s"},
            {"quantity": "eps_derived_from_nose_geometry",
             "statement": "the contact height comes from the line of centres: "
                          "eps = R*(apex - arc_radius - R)/(R + arc_radius), so it is an "
                          "output of the profile rather than a tuned input",
             "tolerance": 1e-9, "unit": "m"},
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
        "sources": ["doc section 4.3"],
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
    return {
        "id": "determinism_golden",
        "title": "Golden-shot determinism check",
        "purpose": "The whole point of the architecture: identical inputs must produce "
                   "identical state, bit for bit, on every run and every platform.",
        "sources": ["doc section 11.4"],
        "setup": {
            "balls": [
                {"id": "cue", "position": [0.635, 1.270], "velocity": [0.0, 0.0],
                 "angular_velocity": [0.0, 0.0, 0.0]},
            ] + [
                {"id": f"obj{n}", "position": [0.635 + 0.06*(n % 4), 1.270 + 0.06*(n // 4)],
                 "velocity": [0.0, 0.0], "angular_velocity": [0.0, 0.0, 0.0]}
                for n in range(1, 16)
            ],
            "shot": {"cue_speed": 8.0, "cue_direction": [0.0, 1.0],
                     "contact_offset": [0.0, 0.0]},
            "params": {"max_sim_time": 30.0},
        },
        "expectations": [
            {"quantity": "final_state_hash", "expected": None, "tolerance": 0,
             "notes": "TO BE FILLED: the first implementation writes the hash of the final "
                      "state here, and it becomes the golden value"},
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
