use crate::identity::{PlayerId, TeamId};
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::prelude::*;
use std::time::Duration;

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
    HalfTime,
    SecondHalf,
    FirstExtraTime,
    SecondExtraTime,
    Penalties,
    FullTime,
}

impl MatchPhase {
    /// Whether match time is running in this phase. The interval and full time
    /// are not playing time, so the clock stands still in them.
    pub fn is_period_of_play(self) -> bool {
        matches!(
            self,
            MatchPhase::FirstHalf
                | MatchPhase::SecondHalf
                | MatchPhase::FirstExtraTime
                | MatchPhase::SecondExtraTime
        )
    }

    /// After this, nothing more can happen in the match.
    pub fn is_over(self) -> bool {
        self == MatchPhase::FullTime
    }
}

/// Law 7 lengths, as competition data rather than constants: the Laws set two
/// halves of 45 and an interval of at most 15, both competition-modifiable, and
/// a scenario shortens them to demonstrate the clock in seconds. Extra time no.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct MatchRegulations {
    pub half_duration: Duration,
    pub half_time_interval: Duration,
}

impl Default for MatchRegulations {
    fn default() -> Self {
        Self {
            half_duration: Duration::from_secs(45 * 60),
            half_time_interval: Duration::from_secs(15 * 60),
        }
    }
}

#[derive(Resource, Debug, Clone, Reflect)]
pub struct MatchState {
    pub home_score: u32,
    pub away_score: u32,
    pub phase: MatchPhase,
    pub set_piece: SetPiece,
    pub set_piece_team: Option<TeamId>,
    /// Time played in the current period. It keeps running while play is
    /// stopped, as it does in a real match; what is missing is the allowance for
    /// time lost, which would be added on top at the end of each period.
    pub period_elapsed: Duration,
    /// Cuánto de este periodo se ha jugado con el balón parado. Es lo que el
    /// árbitro añade al final (Ley 7): el reloj no se detiene en las
    /// reanudaciones, así que el periodo se alarga en vez de descontarse.
    pub stoppage_elapsed: Duration,
    /// Team that kicked off the match, so the other one kicks off the second
    /// half (Law 8).
    pub opening_kick_off_team: TeamId,
    /// Which half each team defends. The interval swaps it, and nothing else
    /// touches it (Law 8).
    pub sides: crate::PitchSides,
    pub is_ball_in_goal: bool,
    pub possession_team: Option<TeamId>,
    /// How long until the pending restart is taken.
    pub restart_in: Duration,
    /// Where the ball is placed for the pending set piece (original: `buffer.restartPos`,
    /// computed at the moment play is stopped).
    pub restart_pos: Vec3,
    pub possession_player: Option<PlayerId>,
    pub previous_possessor: Option<PlayerId>,
    /// Quien tiene que poner el balón en juego y todavía no lo ha hecho. No
    /// puede conducirlo: lo pone en juego pasando o pateando, que es lo que
    /// dicen las leyes de cada reanudación.
    pub restart_taker: Option<PlayerId>,
    /// Match time at which possession last changed hands.
    pub possession_since: Duration,
    /// Intended receiver of the ball in flight (set on a pass, cleared on the
    /// next touch). The designation system gives him priority so the receiver
    /// attacks the pass (the original's receivers run onto `AI_GetPass` balls).
    pub pass_target: Option<PlayerId>,
    /// Where the ball in flight was aimed. Part of the intention, not a
    /// diagnostic: it is what the receiver is running onto.
    pub pass_aim: Vec2,
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
    /// Seeded from a scenario, so the same scenario replays identically.
    pub fn seeded(seed: u32) -> Self {
        MatchRng(crate::math::XorShift32(seed))
    }

    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        self.0.range(min, max)
    }
}

/// Written on every deliberate or accidental ball touch; the referee uses it to
/// run the offside bookkeeping (original: `Referee::BallTouched()`).
#[derive(Message, Debug, Clone, Copy)]
pub struct BallTouched {
    pub player: PlayerId,
}

impl BallTouched {
    pub fn team(&self) -> TeamId {
        self.player.team
    }
}

/// Un contacto que puede ser falta, tal y como ocurrió: quién entró, sobre quién
/// y dónde. Es un hecho y no una decisión: pitarlo es del árbitro, que puede no
/// haberlo visto o verlo y dejar seguir (§3).
#[derive(Message, Debug, Clone, Copy)]
pub struct PotentialFoul {
    pub by: PlayerId,
    pub on: PlayerId,
    pub at: Vec3,
}

/// Players of the last touching team that stood beyond the offside line at the
/// moment of the touch, with their position then (original: `Referee::offsidePlayers`).
#[derive(Resource, Debug, Clone, Default)]
pub struct OffsideRecords {
    pub team: Option<TeamId>,
    pub players: Vec<(PlayerId, Vec3)>,
    /// The line the referee actually judged against, in metres along x, and the
    /// team that was defending it. Published so diagnostics can show the
    /// decision instead of recomputing the rule (which would make presentation
    /// a second, silently diverging referee).
    pub judged_line_x: Option<f32>,
    pub judged_against_team: Option<TeamId>,
}

/// Original: each `Team` caches a designated possession player — the single
/// player expected to reach the ball first (`GetTimeNeededToGetToBall_ms` /
/// `GetDesignatedTeamPossessionPlayer`). Only he goes to the ball; everyone
/// else holds the team shape.
#[derive(Resource, Debug, Clone, Default)]
pub struct PossessionDesignation {
    pub designated: crate::identity::ByTeam<Option<PlayerId>>,
    /// Estimated time (ms) for that player to reach the ball's path.
    pub time_to_ball_ms: crate::identity::ByTeam<f32>,
}

impl Default for MatchState {
    fn default() -> Self {
        Self {
            home_score: 0,
            away_score: 0,
            phase: MatchPhase::PreMatch,
            set_piece: SetPiece::KickOff,
            set_piece_team: Some(TeamId::Home),
            period_elapsed: Duration::ZERO,
            stoppage_elapsed: Duration::ZERO,
            opening_kick_off_team: TeamId::Home,
            sides: crate::PitchSides::opening(),
            is_ball_in_goal: false,
            possession_team: None,
            restart_in: Duration::from_secs(2),
            restart_pos: Vec3::new(0.0, 0.0, 0.11),
            possession_player: None,
            previous_possessor: None,
            restart_taker: None,
            possession_since: Duration::ZERO,
            pass_target: None,
            pass_aim: Vec2::ZERO,
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
