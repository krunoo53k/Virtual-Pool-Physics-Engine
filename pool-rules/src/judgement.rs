//! The one ruling a shot produces, and its announcement words.

/// What the rules decided about one shot: pure data, no methods except the
/// announcement projection.
///
/// The ruling never stores itself anywhere: whoever owns the [`MatchState`]
/// applies it (see [`crate::apply_ruling`]) and hands it to the HUD. The reason
/// words are the fixture contract and must never be re-spelled:
/// `scratch`, `ball_off_table`, `wrong_group_first_contact`, `no_contact`,
/// `no_rail_after_contact`, `illegal_break`, `miscue`, `incomplete_shot`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Judgement {
    /// `""` means a clean shot; otherwise exactly one stable reason word.
    pub foul_reason: String,
    /// Who shoots next; `0` only when the game is over.
    pub next_player: i32,
    /// The next shooter may place the cue anywhere (BCA).
    pub ball_in_hand: bool,
    /// The rack is rebuilt: the 8 on the break, or an illegal break.
    pub re_rack: bool,
    /// Player newly assigned solids; `0` means "no change this shot".
    pub solids_player: i32,
    pub game_over: bool,
    /// `1` or `2` when `game_over`, `0` otherwise.
    pub winner: i32,
}

impl Judgement {
    /// The five signals in the order the fixtures assert them: foul, group
    /// assignment, ball-in-hand grant, then turn *or* game over. A game-over
    /// result deliberately never announces a turn as well.
    pub fn announcements(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(4);
        if !self.foul_reason.is_empty() {
            out.push(format!("foul_committed:{}", self.foul_reason));
        }
        if self.solids_player != 0 {
            out.push(format!("groups_assigned:{}", self.solids_player));
        }
        if self.ball_in_hand {
            // The grant goes to whoever shoots next: the incoming player.
            out.push(format!("ball_in_hand_granted:{}", self.next_player));
        }
        if self.game_over {
            out.push(format!("game_over:{}", self.winner));
        } else {
            out.push(format!("turn_changed:{}", self.next_player));
        }
        out
    }
}
