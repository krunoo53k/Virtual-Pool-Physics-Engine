//! The robustness of the simulation: the shots a game actually plays.
//!
//! The core is asked for a shot with the balls where the game put them, and a
//! game puts them where `f32` can hold them. The difference between an exact
//! position and one that has been through an `f32` is a hundredth of a
//! millimetre, and it must not change whether the game hangs.
//!
//! These tests are the ones that would have caught the two bugs the game
//! found: the vertical spin that ran away and never died, and the ball that
//! went through a rail and rolled off into the dark.
//!
//! A ball that is *put* off the table is the game's business, not the core's:
//! the pocket mouths reach past the nose line, so a ball past the line is not
//! by itself off the table.

use pool_core::config::TableConfig;
use pool_core::math::Vec3;
use pool_core::state::{BallState, ShotInput};
use pool_core::try_simulate_shot_traced;

/// The core's frame in Godot's.
fn to_godot(v: Vec3) -> Vec3 {
    Vec3::new(v.x as f32 as f64, v.z as f32 as f64, -(v.y as f32 as f64))
}

/// Godot's frame in the core's.
fn to_core(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.z, v.y)
}

/// A position on its way through Godot's `f32` vectors and back.
fn through_f32(v: Vec3) -> Vec3 {
    to_core(to_godot(v))
}

/// The eight-ball rack, the way the game puts it up.
fn rack(cfg: &TableConfig, round: bool) -> Vec<BallState> {
    let radius = cfg.ball.radius;
    let foot = Vec3::new(cfg.table.length / 4.0, cfg.table.width / 2.0, radius);
    let cue_ball = Vec3::new(3.0 * cfg.table.length / 4.0, cfg.table.width / 2.0, radius);
    let mut balls = vec![BallState::at_rest(cfg, 0, cue_ball)];
    let mut number = 1;
    for row in 0..5 {
        for column in 0..=row {
            let y = foot.y + (2.0 * column as f64 - row as f64) * radius;
            let x = foot.x - libm::sqrt(3.0) * radius * row as f64;
            let position = Vec3::new(x, y, radius);
            let position = if round {
                through_f32(position)
            } else {
                position
            };
            balls.push(BallState::at_rest(cfg, number, position));
            number += 1;
        }
    }
    if round {
        balls[0].position = through_f32(balls[0].position);
    }
    balls
}

#[test]
fn a_break_settles() {
    let cfg = TableConfig::default();
    let input = pool_core::ShotInput::new(8.0, Vec3::new(-1.0, 0.0, 0.0), rack(&cfg, false));
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the break runs");
    assert!(
        trace.duration < 20.0,
        "the break took {} s, which is not a break",
        trace.duration
    );
    for (index, ball) in state.balls.iter().enumerate() {
        assert!(
            ball.is_settled() || ball.is_pocketed || ball.is_off_table,
            "ball {index} is still moving: {:?}",
            ball.velocity
        );
    }
}

#[test]
fn a_break_settles_with_the_positions_a_game_can_hold() {
    // The same break, with the positions on their way through Godot's `f32`.
    // A hundredth of a millimetre of rounding must not change the outcome.
    let cfg = TableConfig::default();
    let input = pool_core::ShotInput::new(8.0, Vec3::new(-1.0, 0.0, 0.0), rack(&cfg, true));
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the break runs");
    assert!(
        trace.duration < 20.0,
        "the break took {} s, which is not a break",
        trace.duration
    );
    for (index, ball) in state.balls.iter().enumerate() {
        assert!(
            ball.is_settled() || ball.is_pocketed || ball.is_off_table,
            "ball {index} is still moving: {:?} at {:?}",
            ball.velocity,
            ball.position
        );
    }
    let off: Vec<u8> = state
        .balls
        .iter()
        .filter(|b| b.is_off_table)
        .map(|b| b.number)
        .collect();
    assert!(
        off.is_empty(),
        "the balls left the table: {:?} - a ball that rolls into a rail has to bounce",
        off
    );
}

#[test]
fn a_ball_a_nanometre_below_the_bed_does_not_fall_through_the_table() {
    // The bed is at exactly one radius. A game that keeps its balls in `f32` puts
    // the centre a fraction of a nanometre below that, and a ball that is a hair
    // below the bed and a hair above the ground is in no state at all: it cannot
    // land, because the landing time of a ball whose centre is below the bed does
    // not exist, and it falls through the table and off the world.
    let cfg = TableConfig::default();
    let mut balls = rack(&cfg, true);
    for ball in balls.iter_mut() {
        ball.position.z -= 5.0e-10;
    }
    let input = ShotInput::new(8.0, Vec3::new(-1.0, 0.0, 0.0), balls);
    let (state, _trace) = try_simulate_shot_traced(&input, &cfg).expect("the break runs");
    for (index, ball) in state.balls.iter().enumerate() {
        assert!(
            ball.is_settled() || ball.is_pocketed || ball.is_off_table,
            "ball {index} is still moving: {:?} at {:?}",
            ball.velocity,
            ball.position
        );
        assert!(
            ball.position.z >= cfg.ball.radius - 1e-9,
            "ball {index} is below the bed: {:?}",
            ball.position
        );
    }
}

#[test]
fn a_ball_with_a_tiny_vertical_speed_does_not_bounce_for_ever() {
    // The bed tolerance and the settle threshold of §8.2 have to agree. A ball on the
    // bed with a vertical speed far below the settle threshold must be absorbed on
    // its first bounce, not turn into a chain of near-zero events.
    let cfg = TableConfig::default();
    for speed in [1e-9_f64, 1e-6, 1e-3, 0.019] {
        let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.635, cfg.ball.radius));
        ball.velocity = Vec3::new(0.5, 0.0, speed);
        let input = ShotInput::new(0.0, Vec3::X, vec![ball]);
        let mut simulation = pool_core::Simulation::new(&input, &cfg);
        let _ = simulation.run_without_strike();
        let events = simulation.events().len();
        assert!(
            events <= 4,
            "a ball with a vertical speed of {speed} m/s took {events} events"
        );
    }
}

#[test]
fn the_spin_of_a_ball_dies_and_does_not_grow() {
    // The vertical spin decays towards zero and stops there. A spin that grew
    // instead of dying would keep the ball alive for ever, and the game would
    // wait for a shot that never ends.
    let cfg = TableConfig::default();
    let mut ball = BallState::at_rest(&cfg, 0, Vec3::new(0.5, 0.635, cfg.ball.radius));
    ball.velocity = Vec3::new(1.0, 0.0, 0.0);
    ball.angular_velocity = Vec3::new(0.0, 1.0 / cfg.ball.radius, 5.0);
    let input = pool_core::ShotInput::new(0.0, Vec3::X, vec![ball]);
    let (state, trace) = try_simulate_shot_traced(&input, &cfg).expect("the ball rolls");
    assert!(
        trace.duration < 20.0,
        "the roll took {} s, which is not a roll",
        trace.duration
    );
    assert!(
        state.balls[0].angular_velocity.z.abs() < 1e-9,
        "the ball ended with a vertical spin of {:?}",
        state.balls[0].angular_velocity
    );
}

#[test]
fn the_spin_a_game_sees_never_turns_the_wrong_way() {
    // A game reads the spin off the samplers of §11.2, one sample per frame, for
    // a shot it plays back. The line those samplers draw is the model of §3.2 -
    // a decay that acts towards zero. Modelled as one straight line, the decay
    // reaches zero and then runs the other way, and the ball is seen turning the
    // wrong way for the rest of the shot; the spin that was dying instead grows.
    //
    // The vertical spin of §3.2 therefore dies at zero on the sampled numbers
    // too, and stays there: a sampled spin never has the opposite sign to the
    // spin of the segment it is sampled from, and a shot that has run long
    // enough ends with the cue ball dead on its spin.
    let cfg = TableConfig::default();
    let radius = cfg.ball.radius;
    let cue = BallState::at_rest(&cfg, 0, Vec3::new(0.40, 0.635, radius));
    let object = BallState::at_rest(&cfg, 1, Vec3::new(0.90, 0.635, radius));
    let mut input = ShotInput::new(2.0, Vec3::X, vec![cue, object]);
    input.contact_offset_x = 0.45 * radius; // a big side english
    input.contact_offset_y = 0.45 * radius; // ..with follow, so the cue rolls on

    let (_, trace) = try_simulate_shot_traced(&input, &cfg).expect("an english shot is a legal shot");

    // The decay of §3.2 with the default cloth: 5 mu_sp g / 2R, about 10 rad/s².
    // A cue ball that leaves the strike with tens of rad/s of vertical spin dies
    // well inside the long roll that follows the contact.
    let decay = 5.0 * cfg.cloth.mu_sp * cfg.simulation.gravity / (2.0 * radius);
    assert!(decay > 0.0, "The cloth of §2.4 has to decay the spin of §3.2");

    let step = 0.25; // a quarter of a second: what a game samples
    let samples = (trace.duration / step) as i64 + 1;
    for sample in 0..samples {
        let t = sample as f64 * step;
        let segment = trace.segment_at(0, t).unwrap_or_else(|| panic!("the cue ball has a path at t = {t} s"));
        let spin = segment.angular_velocity_at(t).z;
        let start = segment.w0.z;
        // The line of the segment may reach zero, at its very end, and stop
        // there. It may not pass it.
        if start > 0.0 {
            assert!(spin >= 0.0, "cue ball with topspin {start} rad/s was seen at {spin} rad/s at t = {t} s, on the other side of zero");
        } else if start < 0.0 {
            assert!(spin <= 0.0, "cue ball with backspin {start} rad/s was seen at {spin} rad/s at t = {t} s, on the other side of zero");
        } else {
            // A segment that starts with no vertical spin has none to decay,
            // and so never gets any.
            assert_eq!(spin, 0.0, "cue ball with no vertical spin was seen turning at {spin} rad/s at t = {t} s");
        }
    }

    // Late in the shot the spin is dead, and stays dead: the ball is left lying
    // where it stopped, not still winding up.
    let late = trace.sample_angular(trace.duration)[0];
    let dead = late.z;
    assert!(dead.abs() < 1e-6, "the cue ball was still spinning {dead} rad/s about the vertical at the end of the shot");
}

#[test]
fn the_spin_death_leaves_the_two_halves_of_the_path_agreeing() {
    // The recorder cuts the path of a ball at the time its vertical spin dies
    // (see `Recorder.close` in trace.rs). The cut is not an event: the
    // simulation never steps there, so the two halves describe one motion, and
    // the renderer - which samples every frame, and reads both halves - must
    // not see anything there. A half that begins dead begins exactly dead.
    let cfg = TableConfig::default();
    let radius = cfg.ball.radius;
    let cue = BallState::at_rest(&cfg, 0, Vec3::new(0.40, 0.635, radius));
    let object = BallState::at_rest(&cfg, 1, Vec3::new(0.90, 0.635, radius));
    let mut input = ShotInput::new(2.0, Vec3::X, vec![cue, object]);
    input.contact_offset_x = 0.45 * radius;
    input.contact_offset_y = 0.45 * radius;
    let (_, trace) = try_simulate_shot_traced(&input, &cfg).expect("an english shot is a legal shot");

    for (index, segments) in trace.balls.iter().enumerate() {
        for pair in segments.windows(2) {
            let first = pair[0];
            let second = pair[1];
            if first.kind != second.kind {
                continue; // an event changed the motion state here; that is the event's business
            }
            // A cut where the first half decays and the second is dead is the
            // recorder's own split.
            let cut = first.wa.z != 0.0 && second.w0.z == 0.0 && second.wa.z == 0.0;
            if !cut {
                continue;
            }
            let at = first.t1;
            let where_first = first.position_at(at);
            let where_second = second.position_at(second.t0);
            let jump = (where_first - where_second).norm();
            assert!(jump < 1e-9, "ball {index}: the path is cut at t = {at} s with a jump of {jump} m");
            let turned_first = first.rotation_at(at);
            let turned_second = second.rotation_at(second.t0);
            let twist = (turned_first - turned_second).norm();
            assert!(twist < 1e-9, "ball {index}: the rotation jumps at the cut at t = {at} s by {twist} rad");
            // The line of the first half ends at zero, exactly where the second
            // half starts.
            let at_death = first.w0.z + first.wa.z * (at - first.t0);
            assert!(at_death.abs() < 1e-9, "ball {index}: the spin of the first half dies at t = {at} s at {at_death} rad/s, not where the second half starts");
        }
    }
}

#[test]
fn a_ball_carries_its_orientation_into_the_next_shot() {
    // §11.2: the game hands the shot back as it stood, and the next shot is
    // fired at the balls where they lie. A ball that the last shot left turned
    // to some orientation is seen at that orientation when this shot starts -
    // the renderer plays the last shot up to its last frame and this one on
    // from there, with nothing integrated in between.
    let cfg = TableConfig::default();
    let radius = cfg.ball.radius;

    // The same shot, fired twice, the second time with the cue ball handed the
    // orientation the first shot left it in.
    let fire = |turned: Vec3| -> ShotInput {
        let mut cue = BallState::at_rest(&cfg, 0, Vec3::new(0.40, 0.635, radius));
        cue.rotation = turned;
        cue.angular_velocity = Vec3::new(0.0, 1.0 / radius, 3.0); // it was left spinning
        let object = BallState::at_rest(&cfg, 1, Vec3::new(0.90, 0.635, radius));
        let mut input = ShotInput::new(2.0, Vec3::X, vec![cue, object]);
        input.contact_offset_x = 0.45 * radius;
        input.contact_offset_y = 0.45 * radius;
        input
    };

    let (_, first) = try_simulate_shot_traced(&fire(Vec3::ZERO), &cfg).expect("the shot runs");
    let fresh = first.sample_rotation(0.0)[0];
    assert_eq!(fresh, Vec3::ZERO, "a ball that was put on the table starts a shot at an orientation of {fresh:?}");

    let carried = first.sample_rotation(first.duration)[0];
    let (_, second) = try_simulate_shot_traced(&fire(carried), &cfg).expect("the next shot runs");
    let starts = second.sample_rotation(0.0)[0];
    assert_eq!(starts, carried, "the next shot starts the cue ball at {starts:?} rather than the orientation {carried:?} the last shot left it in");

    // And the shot then turns on from there, rather than over again from zero.
    let later = second.sample_rotation(0.5)[0];
    let onward = (later - carried).norm();
    assert!(onward > 0.0, "the cue ball was left unturned through the second shot");
}
