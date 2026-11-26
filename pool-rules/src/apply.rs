//! Folding a ruling into the match state and the rack.
//!
//! The rules decide; this module only records the decision. It is the Rust
//! counterpart of `RulingApply` and `RackModel`, so the server and the client
//! keep their `MatchState` and rack bookkeeping in step.

use pool_core::math::Vec3;

use crate::{Judgement, MatchState};

/// The table's metric numbers, from the client's `TableLayout`.
///
/// `RackState` stores flat triples in the **core** frame: `x` along the table,
/// `y` across it, `z` up. This is also the wire frame, so a rack crosses a
/// WebSocket unchanged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableLayout {
    pub length: f64,
    pub width: f64,
    pub ball_radius: f64,
}

impl TableLayout {
    pub const LENGTH: f64 = 2.54;
    pub const WIDTH: f64 = 1.267;
    pub const BALL_RADIUS: f64 = 0.028575;

    pub const fn new() -> Self {
        Self {
            length: Self::LENGTH,
            width: Self::WIDTH,
            ball_radius: Self::BALL_RADIUS,
        }
    }

    /// The head spot in the core frame, for this layout.
    pub fn head_spot(self) -> Vec3 {
        head_spot(&self)
    }
}

impl Default for TableLayout {
    fn default() -> Self {
        Self::new()
    }
}

/// The client's head spot converted once into the core frame: Godot's
/// `(length * 0.25, radius, -width * 0.5)` becomes
/// `[length * 0.25, width * 0.5, radius]`.
pub const HEAD_SPOT: Vec3 = Vec3::new(0.635, 0.6335, 0.028575);

/// The head spot in the core frame for `layout`.
pub fn head_spot(layout: &TableLayout) -> Vec3 {
    Vec3::new(layout.length * 0.25, layout.width * 0.5, layout.ball_radius)
}

/// Every object-ball number, in rack order.
pub fn full_rack() -> Vec<i32> {
    (1..=15).collect()
}

/// Folds one [`Judgement`] into the composer's [`MatchState`].
///
/// `removed` is the caller's own bookkeeping of which ball numbers left play.
/// The fixture runner passes only pocketed balls; production passes pocketed
/// plus off-table balls. The ruling is recorded verbatim: turn, a non-zero
/// group assignment, ball-in-hand, and `break_next = re_rack`.
pub fn apply_ruling(state: &mut MatchState, ruling: &Judgement, removed: &[i32]) {
    state.player = ruling.next_player;
    if ruling.solids_player != 0 {
        state.solids_player = ruling.solids_player;
    }
    state.ball_in_hand = ruling.ball_in_hand;
    state.break_next = ruling.re_rack;
    if ruling.re_rack {
        state.remaining = full_rack();
        return;
    }
    state.remaining.retain(|number| !removed.contains(number));
}

/// Every ball the simulation sees, as one hand-off.
///
/// `positions` and `rotations` are flat f64 triples in the core frame, three
/// consecutive numbers per ball; `numbers[i]` is the ball carried by triple
/// `i`. The cue is number 0 and sits first. An empty `rotations` means every
/// ball starts at identity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RackState {
    /// m, flat core-frame triples (x along, y across, z up).
    pub positions: Vec<f64>,
    /// The number every triple carries.
    pub numbers: Vec<i32>,
    /// rad, flat core-frame axis-times-angle triples.
    pub rotations: Vec<f64>,
}

impl RackState {
    /// Ball count is the length of the index-aligned `numbers` list.
    pub fn ball_count(&self) -> usize {
        self.numbers.len()
    }

    /// Folds a shot's final frame into the rack.
    ///
    /// `final_positions` and `final_rotations` are core-frame vectors indexed
    /// like `self.numbers` at the moment of the call; the caller keeps those
    /// lists aligned with the core's final ball order. A pocketed or off-table
    /// object ball leaves all three lists. The cue is the one exception: a
    /// scratched cue is respotted at the head spot with zero rotation, because
    /// a ball taken off the bed can stop in mid-air. Every surviving ball is
    /// snapped to rest at exactly one radius, and rotations carry by final-
    /// state index (a missing rotation reads as identity).
    pub fn after_shot(
        &mut self,
        final_positions: &[Vec3],
        final_rotations: &[Vec3],
        removed: &[i32],
    ) {
        let layout = TableLayout::default();
        let mut numbers = Vec::with_capacity(self.numbers.len());
        let mut positions = Vec::with_capacity(self.numbers.len() * 3);
        let mut rotations = Vec::with_capacity(self.numbers.len() * 3);
        for (index, &number) in self.numbers.iter().enumerate() {
            if number != 0 && removed.contains(&number) {
                continue;
            }
            numbers.push(number);
            if number == 0 && removed.contains(&0) {
                let spot = head_spot(&layout);
                positions.extend([spot.x, spot.y, spot.z]);
                rotations.extend([0.0, 0.0, 0.0]);
                continue;
            }
            let position = final_positions.get(index).copied().unwrap_or(Vec3::ZERO);
            positions.extend([position.x, position.y, layout.ball_radius]);
            let rotation = final_rotations.get(index).copied().unwrap_or(Vec3::ZERO);
            rotations.extend([rotation.x, rotation.y, rotation.z]);
        }
        self.numbers = numbers;
        self.positions = positions;
        self.rotations = rotations;
    }

    /// Restores a seeded rack. A re-rack starts from the rack's seeded layout,
    /// not from the shot's final frame, and always starts unspun: the incoming
    /// `rotations` are accepted as the seeded snapshot's shape but
    /// deliberately discarded so spin never carries across a re-rack.
    pub fn rerack(&mut self, numbers: &[i32], positions: &[f64], _rotations: &[f64]) {
        self.numbers = numbers.to_vec();
        self.positions = positions.to_vec();
        self.rotations = vec![0.0; self.numbers.len() * 3];
    }

    /// Places the cue at `spot` and zeros its spin. `spot` is in the core
    /// frame; its height is snapped to the table's ball radius so a
    /// ball-in-hand always lands on the bed. A rack without a cue is unchanged.
    pub fn respot_cue(&mut self, spot: Vec3, table: &TableLayout) {
        let Some(index) = self.numbers.iter().position(|&number| number == 0) else {
            return;
        };
        write_triple(
            &mut self.positions[..],
            index,
            spot.x,
            spot.y,
            table.ball_radius,
        );
        write_triple(&mut self.rotations[..], index, 0.0, 0.0, 0.0);
    }
}

/// The free-function form of [`RackState::respot_cue`].
pub fn respot_cue(rack: &mut RackState, spot: Vec3, table: &TableLayout) {
    rack.respot_cue(spot, table);
}

fn write_triple(flat: &mut [f64], index: usize, x: f64, y: f64, z: f64) {
    let k = index * 3;
    if k + 2 >= flat.len() {
        return;
    }
    flat[k] = x;
    flat[k + 1] = y;
    flat[k + 2] = z;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded_rack() -> RackState {
        let layout = TableLayout::default();
        let mut rack = RackState {
            numbers: vec![0, 1, 2],
            positions: Vec::new(),
            rotations: Vec::new(),
        };
        rack.positions.extend([0.635, 0.6335, layout.ball_radius]);
        rack.positions.extend([1.9, 0.4, layout.ball_radius]);
        rack.positions.extend([1.9, 0.8, layout.ball_radius]);
        rack.rotations.extend([0.0, 0.0, 0.0]);
        rack.rotations.extend([0.1, 0.2, 0.3]);
        rack.rotations.extend([0.4, 0.5, 0.6]);
        rack
    }

    #[test]
    fn full_rack_is_one_through_fifteen() {
        assert_eq!(full_rack(), (1..=15).collect::<Vec<i32>>());
    }

    #[test]
    fn head_spot_is_in_the_core_frame() {
        let spot = head_spot(&TableLayout::default());
        assert_eq!(spot, HEAD_SPOT);
        assert_eq!(spot.x, 0.635);
        assert_eq!(spot.y, 0.6335);
        assert_eq!(spot.z, TableLayout::BALL_RADIUS);
    }

    #[test]
    fn a_scratch_respots_the_cue() {
        let mut rack = seeded_rack();
        let finals = [
            Vec3::new(0.2, 0.3, 1.7), // the cue froze in mid-air
            Vec3::new(1.9, 0.4, 0.028_575),
            Vec3::new(1.9, 0.8, 0.028_575),
        ];
        let spins = [Vec3::new(9.0, 9.0, 9.0), Vec3::X, Vec3::Y];
        rack.after_shot(&finals, &spins, &[0]);
        assert_eq!(rack.numbers, vec![0, 1, 2]);
        assert_eq!(rack.ball_count(), 3);
        assert_eq!(
            rack.positions[0..3],
            [HEAD_SPOT.x, HEAD_SPOT.y, HEAD_SPOT.z]
        );
        assert_eq!(rack.rotations[0..3], [0.0, 0.0, 0.0]);
    }

    #[test]
    fn removed_object_balls_leave_all_three_lists() {
        let mut rack = seeded_rack();
        let finals = [
            Vec3::new(0.2, 0.3, 0.028_575),
            Vec3::new(1.9, 0.4, 0.028_575),
            Vec3::new(1.9, 0.8, 0.028_575),
        ];
        rack.after_shot(&finals, &[Vec3::ZERO; 3], &[2]);
        assert_eq!(rack.numbers, vec![0, 1]);
        assert_eq!(rack.positions.len(), 6);
        assert_eq!(rack.rotations.len(), 6);
        assert_eq!(&rack.positions[3..6], &[1.9, 0.4, TableLayout::BALL_RADIUS]);
    }

    #[test]
    fn surviving_balls_snap_to_the_cloth_and_carry_rotation_by_index() {
        let mut rack = seeded_rack();
        let finals = [
            Vec3::new(0.2, 0.3, 1.7),
            Vec3::new(1.9, 0.4, 0.9),
            Vec3::new(1.9, 0.8, 0.0),
        ];
        let spins = [
            Vec3::new(9.0, 9.0, 9.0),
            Vec3::new(0.7, 0.8, 0.9),
            Vec3::new(1.1, 1.2, 1.3),
        ];
        rack.after_shot(&finals, &spins, &[]);
        assert_eq!(rack.positions[3..6], [1.9, 0.4, TableLayout::BALL_RADIUS]);
        assert_eq!(rack.positions[6..9], [1.9, 0.8, TableLayout::BALL_RADIUS]);
        assert_eq!(rack.rotations[3..6], [0.7, 0.8, 0.9]);
        assert_eq!(rack.rotations[6..9], [1.1, 1.2, 1.3]);
    }

    #[test]
    fn a_missing_rotation_reads_as_identity() {
        let mut rack = seeded_rack();
        let finals = [
            Vec3::new(0.2, 0.3, 0.028_575),
            Vec3::new(1.9, 0.4, 0.028_575),
            Vec3::new(1.9, 0.8, 0.028_575),
        ];
        rack.after_shot(&finals, &[Vec3::ZERO], &[]);
        assert_eq!(rack.rotations.len(), 9);
        assert_eq!(rack.rotations[3..6], [0.0, 0.0, 0.0]);
        assert_eq!(rack.rotations[6..9], [0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_rerack_restores_the_seed_and_resets_rotations() {
        let mut rack = seeded_rack();
        let numbers = vec![0, 1, 2, 3];
        let positions = vec![
            0.635, 0.6335, 0.028_575, 1.9, 0.4, 0.028_575, 2.0, 0.4, 0.028_575, 2.0, 0.8, 0.028_575,
        ];
        let old_spins = vec![1.0; 12];
        rack.rerack(&numbers, &positions, &old_spins);
        assert_eq!(rack.numbers, numbers);
        assert_eq!(rack.positions, positions);
        assert_eq!(rack.rotations, vec![0.0; 12]);
    }

    #[test]
    fn respot_cue_uses_the_given_spot_and_zeros_spin() {
        let mut rack = seeded_rack();
        respot_cue(
            &mut rack,
            Vec3::new(1.0, -0.2, 9.0),
            &TableLayout::default(),
        );
        assert_eq!(rack.positions[0..3], [1.0, -0.2, TableLayout::BALL_RADIUS]);
        assert_eq!(rack.rotations[0..3], [0.0, 0.0, 0.0]);
    }

    #[test]
    fn respot_cue_ignores_a_rack_without_a_cue() {
        let mut rack = RackState {
            numbers: vec![1],
            positions: vec![1.9, 0.4, TableLayout::BALL_RADIUS],
            rotations: vec![1.0, 2.0, 3.0],
        };
        rack.respot_cue(Vec3::ZERO, &TableLayout::default());
        assert_eq!(rack.positions, vec![1.9, 0.4, TableLayout::BALL_RADIUS]);
        assert_eq!(rack.rotations, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn apply_ruling_records_the_decision_and_filters_remaining() {
        let mut state = MatchState {
            player: 1,
            remaining: vec![1, 2, 3, 8],
            ..MatchState::default()
        };
        let ruling = Judgement {
            foul_reason: "scratch".to_string(),
            next_player: 2,
            ball_in_hand: true,
            re_rack: false,
            solids_player: 1,
            game_over: false,
            winner: 0,
        };
        apply_ruling(&mut state, &ruling, &[3]);
        assert_eq!(state.player, 2);
        assert_eq!(state.solids_player, 1);
        assert!(state.ball_in_hand);
        assert!(!state.break_next);
        assert_eq!(state.remaining, vec![1, 2, 8]);
    }

    #[test]
    fn apply_ruling_restores_the_full_rack_on_a_rerack() {
        let mut state = MatchState {
            player: 1,
            remaining: vec![8],
            ..MatchState::default()
        };
        let ruling = Judgement {
            re_rack: true,
            next_player: 1,
            solids_player: 0,
            ..Judgement::default()
        };
        apply_ruling(&mut state, &ruling, &[8]);
        assert_eq!(state.remaining, full_rack());
        assert!(state.break_next);
        assert_eq!(state.player, 1);
    }

    #[test]
    fn apply_ruling_never_clears_an_existing_group() {
        let mut state = MatchState {
            solids_player: 2,
            ..MatchState::default()
        };
        let ruling = Judgement::default();
        apply_ruling(&mut state, &ruling, &[]);
        assert_eq!(state.solids_player, 2);
    }
}
