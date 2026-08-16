use crate::identity::{ByTeam, PlayerId, TeamId};
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
pub enum SetPiece {
    None,
    KickOff,
    GoalKick,
    FreeKick,
    Corner,
    ThrowIn,
    Penalty,
    DroppedBall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
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

/// Kicks from the penalty mark are a separate tie-break, not goals added to
/// the match score. The state records the alternating order and remains valid
/// through sudden death after the first five kicks each (Law 10).
#[derive(Debug, Clone, PartialEq, Eq, Reflect)]
pub struct PenaltyShootout {
    pub taken: ByTeam<u8>,
    pub scored: ByTeam<u8>,
    pub next_team: TeamId,
}

impl PenaltyShootout {
    pub fn new(first_team: TeamId) -> Self {
        Self {
            taken: ByTeam::splat(0),
            scored: ByTeam::splat(0),
            next_team: first_team,
        }
    }

    /// Winner after a kick, including an early decision in the first five and
    /// sudden death only after both sides have taken an equal number.
    pub fn winner(&self) -> Option<TeamId> {
        for team in TeamId::BOTH {
            let opponent = team.opponent();
            if self.taken[team] < 5
                && self.scored[opponent] > self.scored[team] + (5 - self.taken[team])
            {
                return Some(opponent);
            }
        }
        if self.taken.home == self.taken.away && self.taken.home >= 5 {
            if self.scored.home > self.scored.away {
                return Some(TeamId::Home);
            }
            if self.scored.away > self.scored.home {
                return Some(TeamId::Away);
            }
        }
        None
    }
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
#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchRegulations {
    pub half_duration: Duration,
    pub half_time_interval: Duration,
    /// Duración de cada mitad del tiempo suplementario. `None` conserva el
    /// empate al final de los noventa minutos; una competición eliminatoria
    /// declara `Some` y, si corresponde, habilita la tanda.
    pub extra_time_half_duration: Option<Duration>,
    pub extra_time_interval: Duration,
    pub kicks_from_penalty_mark_if_draw: bool,
    /// Law 3 limit, supplied by the competition rather than hidden in a
    /// referee system.
    pub maximum_substitutions: u8,
    /// A side below this count cannot continue. The Laws set seven; retaining
    /// it as data makes variants and didactic scenarios explicit.
    pub minimum_players: u8,
    /// Probability used by the explicitly simplified shoot-out conversion
    /// mechanism. It is competition scenario data, never a hidden referee
    /// constant.
    pub shootout_conversion_probability: f32,
}

impl Default for MatchRegulations {
    fn default() -> Self {
        Self {
            half_duration: Duration::from_secs(45 * 60),
            half_time_interval: Duration::from_secs(15 * 60),
            extra_time_half_duration: None,
            extra_time_interval: Duration::from_secs(5 * 60),
            kicks_from_penalty_mark_if_draw: false,
            maximum_substitutions: 5,
            minimum_players: 7,
            shootout_conversion_probability: 0.75,
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
    /// Team that fell below the competition minimum after a dismissal. `None`
    /// is a normal result; this preserves the distinct Law 3 conclusion.
    pub unable_to_continue: Option<TeamId>,
    /// Present only while the Law 10 tie-break is being resolved.
    pub penalty_shootout: Option<PenaltyShootout>,
    /// Once extra time has begun the terminal broadcast clock must retain its
    /// two additional periods instead of falling back to ninety minutes.
    pub extra_time_started: bool,
}

impl MatchState {
    /// A deliberate kick leaves the team controlling its own ball in flight;
    /// only the individual carrier is gone until the next touch.
    pub fn release_possessor_to_controlled_flight(&mut self) {
        self.possession_player = None;
    }

    /// A deliberate kick can leave a team controlling the ball in flight, even
    /// though no individual is carrying it. Accidental releases are different:
    /// the ball is genuinely loose and neither side has possession.
    pub fn lose_possession_to_a_loose_ball(&mut self) {
        self.possession_player = None;
        self.possession_team = None;
        self.pass_target = None;
    }

    /// The final whistle cannot leave a restart, carrier, or tie-break alive.
    /// Those fields describe ways play may continue, which is impossible once
    /// the match has ended.
    pub fn end_match(&mut self) {
        self.phase = MatchPhase::FullTime;
        self.set_piece = SetPiece::None;
        self.set_piece_team = None;
        self.restart_in = Duration::ZERO;
        self.restart_taker = None;
        self.lose_possession_to_a_loose_ball();
        self.penalty_shootout = None;
    }
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
            unable_to_continue: None,
            penalty_shootout: None,
            extra_time_started: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_accidental_release_leaves_no_team_in_possession() {
        let mut state = MatchState {
            possession_player: Some(PlayerId::home(9)),
            possession_team: Some(TeamId::Home),
            pass_target: Some(PlayerId::home(10)),
            ..Default::default()
        };

        state.lose_possession_to_a_loose_ball();

        assert_eq!(state.possession_player, None);
        assert_eq!(state.possession_team, None);
        assert_eq!(state.pass_target, None);
    }

    #[test]
    fn a_deliberate_kick_keeps_its_team_in_control_while_the_ball_flies() {
        let mut state = MatchState {
            possession_player: Some(PlayerId::home(9)),
            possession_team: Some(TeamId::Home),
            pass_target: Some(PlayerId::home(10)),
            ..Default::default()
        };

        state.release_possessor_to_controlled_flight();

        assert_eq!(state.possession_player, None);
        assert_eq!(state.possession_team, Some(TeamId::Home));
        assert_eq!(state.pass_target, Some(PlayerId::home(10)));
    }

    #[test]
    fn an_ended_match_has_no_restart_or_possession_state() {
        let mut state = MatchState {
            phase: MatchPhase::Penalties,
            set_piece: SetPiece::KickOff,
            set_piece_team: Some(TeamId::Away),
            restart_in: Duration::from_secs(2),
            restart_taker: Some(PlayerId::away(1)),
            possession_player: Some(PlayerId::away(9)),
            possession_team: Some(TeamId::Away),
            pass_target: Some(PlayerId::away(10)),
            penalty_shootout: Some(PenaltyShootout::new(TeamId::Home)),
            ..Default::default()
        };

        state.end_match();

        assert_eq!(state.phase, MatchPhase::FullTime);
        assert_eq!(state.set_piece, SetPiece::None);
        assert_eq!(state.set_piece_team, None);
        assert_eq!(state.restart_in, Duration::ZERO);
        assert_eq!(state.restart_taker, None);
        assert_eq!(state.possession_player, None);
        assert_eq!(state.possession_team, None);
        assert_eq!(state.pass_target, None);
        assert_eq!(state.penalty_shootout, None);
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct PitchConfig {
    pub half_width: f32,       // pitchHalfW = 55.0
    pub half_height: f32,      // pitchHalfH = 36.0
    pub full_half_width: f32,  // pitchFullHalfW = 60.0
    pub full_half_height: f32, // pitchFullHalfH = 40.0
    pub line_half_width: f32,  // lineHalfW = 0.06
    pub goal_depth: f32,       // goalDepth = 2.55
    pub goal_height: f32,      // goalHeight = 2.5
    pub goal_half_width: f32,  // goalHalfWidth = 3.7
    /// Ley 1: profundidad del área penal desde la línea de gol, en metros.
    pub penalty_area_depth: f32,
    /// Medio ancho del área penal, en metros.
    pub penalty_area_half_width: f32,
    /// Distancia de la marca penal a la línea de gol, en metros (Ley 1).
    pub penalty_mark_distance: f32,
    pub ball_radius: f32, // ballRadius = 0.11
    pub post_radius: f32, // postRadius = 0.07
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
            penalty_area_depth: 16.5,
            penalty_area_half_width: 20.16,
            penalty_mark_distance: 11.0,
            ball_radius: 0.11,
            post_radius: 0.07,
        }
    }
}
