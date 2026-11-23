//! The classes the game talks to.
//!
//! Two classes: [`PoolCore`] is the entry point - the table, the aim preview
//! and the shot - and [`PoolShot`] is a resolved shot: the path to play back
//! and the state it ends in.
//!
//! Everything that crosses into Godot is converted here, and only here: the
//! frame of the game is in [`crate::frame`], the shot itself is in
//! [`crate::shot`], and the core's JSON is in [`crate::json`].

use godot::builtin::{
    GString, PackedInt32Array, PackedVector3Array, VarArray, VarDictionary, Vector2, Vector3,
};
use godot::classes::IRefCounted;
use godot::obj::Gd;
use godot::prelude::*;

use crate::frame;
use crate::json as core_json;
use crate::shot;
use pool_core::config::TableConfig;
use pool_core::geometry::TableGeometry;
use pool_core::state::{GamePhase, GameState, MotionState};
use pool_core::trace::{event_kind_name, pocket_name};
use pool_core::{hash_state_hex, Trace};

/// The physics core, as the game sees it.
///
/// The core is the only physics in the game: this class hands out the table,
/// the aim preview and the shot, and the game plays the shot back.
#[derive(GodotClass)]
#[class(init)]
pub struct PoolCore {
    /// The table the shots are played on.
    table: TableConfig,
}

#[godot_api]
impl IRefCounted for PoolCore {}

#[godot_api]
impl PoolCore {
    /// The version of the physics model, following the research document.
    #[func]
    fn get_model_version(&self) -> GString {
        GString::from(pool_core::MODEL_VERSION)
    }

    /// The table configuration, as JSON.
    #[func]
    fn get_table_config(&self) -> GString {
        GString::from(self.table.to_json().as_str())
    }

    /// Reads the table configuration from JSON.
    ///
    /// Returns false and keeps the old configuration if the JSON does not
    /// describe a table.
    #[func]
    fn set_table_config(&mut self, json: GString) -> bool {
        match TableConfig::from_json(&json.to_string()) {
            Ok(table) => {
                self.table = table;
                true
            }
            Err(_) => false,
        }
    }

    /// The largest tip offset that does not miscue, in metres.
    #[func]
    fn get_miscue_limit(&self) -> f64 {
        self.table.max_offset_without_miscue()
    }

    /// The geometry of the table: the rails, the pockets and the cushion.
    ///
    /// The numbers are the ones the physics uses, so the table the player sees
    /// cannot drift away from the table the balls roll on.
    #[func]
    fn get_table_geometry(&self) -> VarDictionary {
        let geometry = TableGeometry::new(&self.table);
        core_json::to_dictionary(&geometry.to_json_value(&self.table))
    }

    /// The aim preview: where the cue ball leaves, and whether the tip holds.
    ///
    /// The direction is the one the cue ball really takes, squirt included: it
    /// comes from the same strike the shot uses, so the preview cannot promise
    /// a line the shot does not take.
    ///
    /// `contact_offset` is the tip on the cue ball's face: `x` is the distance
    /// to the shooter's **right**, `y` the distance **above** the centre, both
    /// in metres.
    #[func]
    fn preview_cue_strike(
        &self,
        cue_speed: f64,
        cue_direction: Vector3,
        contact_offset: Vector2,
    ) -> VarDictionary {
        let input = self.input(cue_speed, cue_direction, contact_offset, &[], &[], &[]);
        let strike = shot::strike(&self.table, &input);
        let mut dictionary = VarDictionary::new();
        dictionary.set("velocity", frame::to_godot(strike.velocity));
        dictionary.set("squirt_deg", strike.squirt.to_degrees());
        dictionary.set("squirt_sign", strike.squirt_sign);
        dictionary.set("miscue", strike.miscue);
        dictionary.set("impact_parameter", strike.impact_parameter);
        dictionary.set(
            // The angle the cue makes with the cloth, signed the way the core
            // signs a stick (sim.rs:1209 takes the asin of the vertical of the
            // very same direction this is handed it, so a cue pointing down at
            // the ball, the way a raised butt points it, is negative). The game
            // reads its own raise back off this number rather than deciding it
            // again: one number, and the readout cannot lie about the physics.
            "elevation_deg",
            match frame::to_core(cue_direction).normalize() {
                Some(direction) => libm::asin(direction.z.clamp(-1.0, 1.0)).to_degrees(),
                None => 0.0,
            },
        );
        dictionary
    }

    /// The cue direction for a level aim and a cue raised at the butt, as a
    /// unit vector in the Godot frame with the elevation baked in (§12.3),
    /// computed with the core's own `libm` trig (§11.4) rather than GDScript's.
    /// The GDScript side never calls `sin`/`cos` to build a replayable
    /// direction: it stores degrees and asks for the direction it wants, so a
    /// pin written as `8 m/s, 90 degrees, raised 12` replays identically on
    /// every platform and the headless pins can use the same values.
    ///
    /// `level_aim_deg` is measured from the core's `+x` (Godot's +x, along the
    /// length of the table) towards its `+y` (Godot's -y, the shooter's left);
    /// `elevation_deg` is the raise at the butt - positive raises the cue and
    /// points it DOWN at the ball, which is the opposite sign to the `elevation_deg`
    /// the preview reads back (`asin` of the axis' height).
    #[func]
    fn make_cue_direction(level_aim_deg: f64, elevation_deg: f64) -> Vector3 {
        frame::cue_direction(level_aim_deg, elevation_deg)
    }

    /// The shot: the whole thing, resolved in one call.
    ///
    /// The balls are given as their positions in Godot's frame, in the order
    /// the numbers are: the cue ball is the first one. The shot comes back as a
    /// [`PoolShot`] to play back; the simulation itself takes no time at all,
    /// because the core is event driven and never integrates.
    ///
    /// The balls start at identity: no ball carries an orientation into the
    /// shot. A game that plays one shot after another wants
    /// [`PoolCore.simulate_with_rotations`] instead, or the spins of its
    /// resting balls reset at every shot.
    #[func]
    fn simulate(
        &self,
        cue_speed: f64,
        cue_direction: Vector3,
        contact_offset: Vector2,
        positions: PackedVector3Array,
        numbers: PackedInt32Array,
    ) -> Gd<PoolShot> {
        self.resolve(self.input(
            cue_speed,
            cue_direction,
            contact_offset,
            positions.as_slice(),
            numbers.as_slice(),
            &[],
        ))
    }

    /// The shot, with the orientation every ball carries into it.
    ///
    /// `rotations` is the rotation every ball had when the shot started, as a
    /// rotation vector in Godot's frame - the direction is the axis of the
    /// orientation, the length is the angle in radians - index by index with
    /// `positions`. The game passes the rotations it played at the end of the
    /// last shot, so a ball that is still lying where that shot left it starts
    /// this one turned the way it was left turned, and its spin does not reset;
    /// a ball that was put back on the table carries `Vector3.ZERO` and starts
    /// over. This is §11.2's hand-off seen from the input side, and it is the
    /// only way the carry can be done: the renderer plays the path, so the
    /// carry belongs to the library that made the path, not to the renderer.
    #[func]
    fn simulate_with_rotations(
        &self,
        cue_speed: f64,
        cue_direction: Vector3,
        contact_offset: Vector2,
        positions: PackedVector3Array,
        numbers: PackedInt32Array,
        rotations: PackedVector3Array,
    ) -> Gd<PoolShot> {
        self.resolve(self.input(
            cue_speed,
            cue_direction,
            contact_offset,
            positions.as_slice(),
            numbers.as_slice(),
            rotations.as_slice(),
        ))
    }

    /// The shot, from an input the core already takes.
    fn resolve(&self, input: pool_core::ShotInput) -> Gd<PoolShot> {
        match shot::simulate(&self.table, &input) {
            Ok((state, trace)) => Gd::from_object(PoolShot {
                trace,
                state,
                error: GString::new(),
                table: self.table.clone(),
            }),
            Err(error) => Gd::from_object(PoolShot {
                trace: Trace::default(),
                state: shot::faulted_state(&input),
                error: GString::from(error.message.as_str()),
                table: self.table.clone(),
            }),
        }
    }

    /// The shot input for a stroke.
    ///
    /// `rotations` is the orientation every ball carries into the shot, in
    /// Godot's frame: a rotation vector is an axis times an angle, and the
    /// frame's mapping is a rotation, so the mapping carries it across the
    /// same way it carries a position. A list that does not name a ball leaves
    /// it at identity.
    fn input(
        &self,
        cue_speed: f64,
        cue_direction: Vector3,
        contact_offset: Vector2,
        positions: &[Vector3],
        numbers: &[i32],
        rotations: &[Vector3],
    ) -> pool_core::ShotInput {
        let positions: Vec<pool_core::math::Vec3> =
            positions.iter().map(|p| frame::to_core(*p)).collect();
        let rotations: Vec<pool_core::math::Vec3> =
            rotations.iter().map(|r| frame::to_core(*r)).collect();
        shot::shot_input(
            &self.table,
            cue_speed,
            frame::to_core(cue_direction),
            (contact_offset.x as f64, contact_offset.y as f64),
            &positions,
            numbers,
            &rotations,
        )
    }
}

/// A resolved shot: the path to play back and the state it ends in.
///
/// The path is the trace of the shot: between two events every ball's state is
/// given by a closed form expression, so the position at any time - including
/// a time between two frames - is exact.
#[derive(GodotClass)]
#[class(no_init)]
pub struct PoolShot {
    /// The path of every ball.
    trace: Trace,
    /// The state the shot ends in.
    state: GameState,
    /// The fault, if there was one.
    error: GString,
    /// The table the shot was played on, because sampling the path at a time
    /// inside the shot needs it and the state at the end of the shot does not.
    table: TableConfig,
}

#[godot_api]
impl IRefCounted for PoolShot {}

#[godot_api]
impl PoolShot {
    /// How long the shot lasts, in seconds.
    #[func]
    fn get_duration(&self) -> f64 {
        self.trace.duration
    }

    /// The fault the simulation reported, if there was one.
    #[func]
    fn get_error(&self) -> GString {
        self.error.clone()
    }

    /// The hash of the final state.
    ///
    /// The same shot always gives the same hash, on every machine: that is the
    /// determinism the core promises (§11.4), and the game can check it.
    #[func]
    fn get_hash(&self) -> GString {
        GString::from(hash_state_hex(&self.state).as_str())
    }

    /// The position of every ball at the time `t`, in Godot's frame.
    #[func]
    fn sample(&self, t: f64) -> PackedVector3Array {
        self.trace
            .sample_positions(t)
            .into_iter()
            .map(frame::to_godot)
            .collect()
    }

    /// The angular velocity of every ball at the time `t`, in Godot's frame.
    #[func]
    fn sample_spin(&self, t: f64) -> PackedVector3Array {
        self.trace
            .sample_angular(t)
            .into_iter()
            .map(frame::to_godot)
            .collect()
    }

    /// The orientation of every ball at the time `t`, in Godot's frame.
    ///
    /// A rotation vector: the direction is the axis of the orientation and the
    /// length is the angle in radians.
    ///
    /// This is an orientation and not an amount of turning: it already includes
    /// the rotation the ball carried into the shot, so the game can play the
    /// last frame of one shot and the first of the next without integrating
    /// anything between them (§11.2's hand-off).
    #[func]
    fn sample_rotation(&self, t: f64) -> PackedVector3Array {
        self.trace
            .sample_rotation(t)
            .into_iter()
            .map(frame::to_godot)
            .collect()
    }

    /// The state of one ball at the end of the shot.
    #[func]
    fn get_ball_state(&self, index: i64) -> VarDictionary {
        let mut dictionary = VarDictionary::new();
        let index = usize::try_from(index).unwrap_or(usize::MAX);
        let ball = match self.state.balls.get(index) {
            Some(ball) => ball,
            None => return dictionary,
        };
        dictionary.set("number", ball.number as i64);
        dictionary.set("position", frame::to_godot(ball.position));
        dictionary.set("pocketed", ball.is_pocketed);
        dictionary.set("off_table", ball.is_off_table);
        dictionary.set(
            "pocket_state",
            &GString::from(pocket_name(ball.pocket_state)).to_variant(),
        );
        dictionary.set(
            "motion_state",
            &GString::from(motion_state_name(ball.motion_state)).to_variant(),
        );
        dictionary
    }

    /// The state of one ball at a time inside the shot, rather than at its end.
    ///
    /// The end of a shot is not all of it: a ball leaves the cloth for a moment
    /// when the stroke drives it down off the slate (§7.1, §7.2), and the state
    /// at the end of the shot has already settled that moment away. This is the
    /// same dictionary the game reads at the end, but read on the path of the
    /// shot at the time it was asked about: the same analytic sampling the
    /// renderer plays the path with, so a game that asks about a moment and a
    /// game that plays it cannot disagree about the ball being in the air.
    #[func]
    fn get_ball_state_at(&self, index: i64, t: f64) -> VarDictionary {
        let mut dictionary = VarDictionary::new();
        let index = usize::try_from(index).unwrap_or(usize::MAX);
        let at_the_time = self.trace.sample(t, &self.table);
        let ball = match at_the_time.get(index) {
            Some(ball) => ball,
            None => return dictionary,
        };
        dictionary.set("number", ball.number as i64);
        dictionary.set("position", frame::to_godot(ball.position));
        dictionary.set("pocketed", ball.is_pocketed);
        dictionary.set("off_table", ball.is_off_table);
        dictionary.set(
            "pocket_state",
            &GString::from(pocket_name(ball.pocket_state)).to_variant(),
        );
        dictionary.set(
            "motion_state",
            &GString::from(motion_state_name(ball.motion_state)).to_variant(),
        );
        dictionary
    }

    /// The events of the shot, in the order they were resolved.
    #[func]
    fn get_events(&self) -> VarArray {
        let mut events = VarArray::new();
        for event in &self.trace.events {
            let mut dictionary = VarDictionary::new();
            dictionary.set("time", event.time);
            dictionary.set(
                "kind",
                &GString::from(event_kind_name(event.kind)).to_variant(),
            );
            dictionary.set("a", event.a as i64);
            dictionary.set("b", event.b as i64);
            dictionary.set("detail", event.detail as i64);
            events.push(&dictionary.to_variant());
        }
        events
    }

    /// The state the shot ends in.
    #[func]
    fn get_final_state(&self) -> VarDictionary {
        let mut dictionary = VarDictionary::new();
        dictionary.set("elapsed_time", self.state.elapsed_time);
        dictionary.set("foul", self.state.foul);
        dictionary.set("miscue", self.state.miscue);
        dictionary.set(
            "phase",
            &GString::from(phase_name(self.state.phase)).to_variant(),
        );
        let mut balls = VarArray::new();
        for ball in &self.state.balls {
            let mut entry = VarDictionary::new();
            entry.set("number", ball.number as i64);
            entry.set("position", frame::to_godot(ball.position));
            entry.set("velocity", frame::to_godot(ball.velocity));
            entry.set("pocketed", ball.is_pocketed);
            entry.set("off_table", ball.is_off_table);
            entry.set(
                "pocket_state",
                &GString::from(pocket_name(ball.pocket_state)).to_variant(),
            );
            entry.set(
                "motion_state",
                &GString::from(motion_state_name(ball.motion_state)).to_variant(),
            );
            balls.push(&entry.to_variant());
        }
        dictionary.set("balls", &balls.to_variant());
        dictionary
    }
}

/// The name of a motion state, as the game sees it.
fn motion_state_name(state: MotionState) -> &'static str {
    match state {
        MotionState::Sliding => "sliding",
        MotionState::Rolling => "rolling",
        MotionState::Spinning => "spinning",
        MotionState::Stationary => "stationary",
        MotionState::Airborne => "airborne",
    }
}

/// The name of a game phase, as the game sees it.
fn phase_name(phase: GamePhase) -> &'static str {
    match phase {
        GamePhase::Aiming => "aiming",
        GamePhase::Shooting => "shooting",
        GamePhase::Settled => "settled",
        GamePhase::Settling => "settling",
        GamePhase::Faulted => "faulted",
    }
}
