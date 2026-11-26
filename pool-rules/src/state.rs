//! The two inputs to a ruling that describe the match and the stroke.

/// Everything true about the match between shots.
///
/// Players are numbered 1 and 2. `solids_player` is 0 while the table is open
/// (the break never assigns groups). `remaining` carries the object-ball
/// numbers still on the table, cue excluded, the 8 among them until it drops:
/// "on the 8" is decided from this list as it was *before* the shot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchState {
    /// Whose turn it is (1 or 2).
    pub player: i32,
    /// 0 while open; otherwise the player holding balls 1..=7.
    pub solids_player: i32,
    /// Object-ball numbers still on the table, cue excluded.
    pub remaining: Vec<i32>,
    /// The next stroke places the cue anywhere (set by a foul's ruling).
    pub ball_in_hand: bool,
    /// The next stroke is a break stroke.
    pub break_next: bool,
}

impl Default for MatchState {
    fn default() -> Self {
        Self {
            player: 1,
            solids_player: 0,
            remaining: Vec::new(),
            ball_in_hand: false,
            break_next: true,
        }
    }
}

impl MatchState {
    /// A fresh match: player 1, open table, empty rack, next stroke is a break.
    pub fn new() -> Self {
        Self::default()
    }
}

/// One stroke, described in player-intent terms before any frame exists.
///
/// The rules only read [`ShotParams::called_pocket`]: BCA calls only the 8, so
/// the other fields are carried for the simulator and the wire.
#[derive(Clone, Debug, PartialEq)]
pub struct ShotParams {
    /// m/s: how hard the cue ball is struck.
    pub speed: f64,
    /// deg: aim from the table's long axis towards the shooter's left.
    pub level_aim_deg: f64,
    /// deg: how much the cue is raised at the butt.
    pub elevation_deg: f64,
    /// m: where the tip meets the cue ball, to the shooter's right.
    pub contact_x: f64,
    /// m: where the tip meets the cue ball, above the centre.
    pub contact_y: f64,
    /// Core pocket index (0-5, corners first) called for the 8; -1 means none.
    pub called_pocket: i32,
}

impl Default for ShotParams {
    fn default() -> Self {
        Self {
            speed: 8.0,
            level_aim_deg: 90.0,
            elevation_deg: 0.0,
            contact_x: 0.0,
            contact_y: 0.0,
            called_pocket: -1,
        }
    }
}

impl ShotParams {
    /// The client's default stroke: 8 m/s straight ahead, centre contact,
    /// no call.
    pub fn new() -> Self {
        Self::default()
    }
}
