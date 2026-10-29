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
        let input = self.input(cue_speed, cue_direction, contact_offset, &[], &[]);
        let strike = shot::strike(&self.table, &input);
        let mut dictionary = VarDictionary::new();
        dictionary.set("velocity", frame::to_godot(strike.velocity));
        dictionary.set("squirt_deg", strike.squirt.to_degrees());
        dictionary.set("squirt_sign", strike.squirt_sign);
        dictionary.set("miscue", strike.miscue);
        dictionary.set("impact_parameter", strike.impact_parameter);
        dictionary
    }

    /// The shot: the whole thing, resolved in one call.
    ///
    /// The balls are given as their positions in Godot's frame, in the order
    /// the numbers are: the cue ball is the first one. The shot comes back as
    /// a [`PoolShot`] to play back; the simulation itself takes no time at all,
    /// because the core is event driven and never integrates.
    #[func]
    fn simulate(
        &self,
        cue_speed: f64,
        cue_direction: Vector3,
        contact_offset: Vector2,
        positions: PackedVector3Array,
        numbers: PackedInt32Array,
    ) -> Gd<PoolShot> {
        let input = self.input(
            cue_speed,
            cue_direction,
            contact_offset,
            positions.as_slice(),
            numbers.as_slice(),
        );
        match shot::simulate(&self.table, &input) {
            Ok((state, trace)) => Gd::from_object(PoolShot {
                trace,
                state,
                error: GString::new(),
            }),
            Err(error) => Gd::from_object(PoolShot {
                trace: Trace::default(),
                state: shot::faulted_state(&input),
                error: GString::from(error.message.as_str()),
            }),
        }
    }

    /// The shot input for a stroke.
    fn input(
        &self,
        cue_speed: f64,
        cue_direction: Vector3,
        contact_offset: Vector2,
        positions: &[Vector3],
        numbers: &[i32],
    ) -> pool_core::ShotInput {
        let positions: Vec<pool_core::math::Vec3> =
            positions.iter().map(|p| frame::to_core(*p)).collect();
        shot::shot_input(
            &self.table,
            cue_speed,
            frame::to_core(cue_direction),
            (contact_offset.x as f64, contact_offset.y as f64),
            &positions,
            numbers,
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

    /// The rotation every ball has made by the time `t`, in Godot's frame.
    ///
    /// A rotation vector: the direction is the axis of the rotation and the
    /// length is the angle in radians.
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
