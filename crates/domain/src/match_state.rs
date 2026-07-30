use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TeamSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum SetPiece {
    None,
    KickOff,
    GoalKick,
    FreeKick,
    Corner,
    ThrowIn,
    Penalty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum MatchPhase {
    PreMatch,
    FirstHalf,
    SecondHalf,
    FirstExtraTime,
    SecondExtraTime,
    Penalties,
}

#[derive(Resource, Debug, Clone, Reflect)]
pub struct MatchState {
    pub home_score: u32,
    pub away_score: u32,
    pub phase: MatchPhase,
    pub set_piece: SetPiece,
    pub set_piece_team: Option<u32>, // 0 for home, 1 for away
    pub timer_ms: u64,
    pub is_ball_in_goal: bool,
    pub possession_team: Option<u32>,
    pub set_piece_timer: f32, // Time left before resetting/restarting play
    /// Where the ball is placed for the pending set piece (original: `buffer.restartPos`,
    /// computed at the moment play is stopped).
    pub restart_pos: Vec3,
    pub possession_player: Option<Entity>, // Entity currently in possession of the ball
    pub previous_possessor: Option<Entity>, // Entity that previously had possession
    pub last_possession_change_time: u64,  // Time when possession last changed
    /// Intended receiver of the ball in flight (set on a pass, cleared on the
    /// next touch). The designation system gives him priority so the receiver
    /// attacks the pass (the original's receivers run onto `AI_GetPass` balls).
    pub pass_target: Option<Entity>,
    /// Diagnostics: kind of the last deliberate release
    /// (0 none, 1 pass, 2 dribble knock, 3 clear/shot).
    pub last_release_kind: u8,
    /// Diagnostics: turnovers attributed to the release kind that lost the ball.
    pub turnovers_by_kind: [u32; 4],
    /// Diagnostics: aim point of the last pass.
    pub last_pass_aim: Vec2,
    /// Diagnostics: pass turnovers within 2.5 m of the aim (lost receptions)
    /// vs farther away (lane interceptions).
    pub pass_turnovers_near: u32,
    pub pass_turnovers_far: u32,
    /// Diagnostics: origin of the last pass.
    pub last_pass_origin: Vec2,
    /// Diagnostics: first N pass-turnover descriptions (u along lane, roles).
    pub pass_turnover_log: Vec<String>,
}

/// Deterministic match RNG (stand-in for the engine's global `random(min, max)`;
/// seeded so headless test matches replay identically).
#[derive(Resource, Debug, Clone)]
pub struct MatchRng(pub crate::math::XorShift32);

impl Default for MatchRng {
    fn default() -> Self {
        MatchRng(crate::math::XorShift32(0xC0FFEE))
    }
}

impl MatchRng {
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        self.0.range(min, max)
    }
}

/// Written on every deliberate or accidental ball touch; the referee uses it to
/// run the offside bookkeeping (original: `Referee::BallTouched()`).
#[derive(Message, Debug, Clone, Copy)]
pub struct BallTouched {
    pub player: Entity,
    pub team: u32,
}

/// Players of the last touching team that stood beyond the offside line at the
/// moment of the touch, with their position then (original: `Referee::offsidePlayers`).
#[derive(Resource, Debug, Clone, Default)]
pub struct OffsideRecords {
    pub team: Option<u32>,
    pub players: Vec<(Entity, Vec3)>,
}

/// Original: each `Team` caches a designated possession player — the single
/// player expected to reach the ball first (`GetTimeNeededToGetToBall_ms` /
/// `GetDesignatedTeamPossessionPlayer`). Only he goes to the ball; everyone
/// else holds the team shape.
#[derive(Resource, Debug, Clone, Default)]
pub struct PossessionDesignation {
    /// Designated player per team (index 0 = home, 1 = away).
    pub designated: [Option<Entity>; 2],
    /// Estimated time (ms) for that player to reach the ball's path.
    pub time_to_ball_ms: [f32; 2],
}

impl Default for MatchState {
    fn default() -> Self {
        Self {
            home_score: 0,
            away_score: 0,
            phase: MatchPhase::PreMatch,
            set_piece: SetPiece::KickOff,
            set_piece_team: Some(0), // home kicks off
            timer_ms: 0,
            is_ball_in_goal: false,
            possession_team: None,
            set_piece_timer: 2.0, // Start with a 2-second kickoff delay
            restart_pos: Vec3::new(0.0, 0.0, 0.11),
            possession_player: None,
            previous_possessor: None,
            last_possession_change_time: 0,
            pass_target: None,
            last_release_kind: 0,
            turnovers_by_kind: [0; 4],
            last_pass_aim: Vec2::ZERO,
            pass_turnovers_near: 0,
            pass_turnovers_far: 0,
            last_pass_origin: Vec2::ZERO,
            pass_turnover_log: Vec::new(),
        }
    }
}

#[derive(Resource, Debug, Clone, Reflect)]
pub struct PitchConfig {
    pub half_width: f32,       // pitchHalfW = 55.0
    pub half_height: f32,      // pitchHalfH = 36.0
    pub full_half_width: f32,  // pitchFullHalfW = 60.0
    pub full_half_height: f32, // pitchFullHalfH = 40.0
    pub line_half_width: f32,  // lineHalfW = 0.06
    pub goal_depth: f32,       // goalDepth = 2.55
    pub goal_height: f32,      // goalHeight = 2.5
    pub goal_half_width: f32,  // goalHalfWidth = 3.7
    pub ball_radius: f32,      // ballRadius = 0.11
    pub post_radius: f32,      // postRadius = 0.07
}

impl Default for PitchConfig {
    fn default() -> Self {
        Self {
            half_width: 55.0,
            half_height: 36.0,
            full_half_width: 60.0,
            full_half_height: 40.0,
            line_half_width: 0.06,
            goal_depth: 2.55,
            goal_height: 2.5,
            goal_half_width: 3.7,
            ball_radius: 0.11,
            post_radius: 0.07,
        }
    }
}
