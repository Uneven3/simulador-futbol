use crate::SimulationSet;
use crate::diagnostics::{MatchFact, MatchTelemetry};
use crate::match_setup::base_formation_position;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use football_domain::{
    BALL_RADIUS, Ball, BallTouched, Facing, MatchState, OffsideRecords, PitchConfig, Player,
    PlayerId, PlayerMatchState, Position, SetPiece, TeamId, Velocity,
};
use std::time::Duration;

pub struct RefereePlugin;

impl Plugin for RefereePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OffsideRecords::default()).add_systems(
            FixedUpdate,
            (
                referee_offside_system,
                referee_system,
                referee_set_piece_system,
            )
                .chain()
                .in_set(SimulationSet::Referee),
        );
    }
}

/// Port of `Match::CheckForGoal(side)`: swept segment (previous → current ball
/// position) against the goal mouth plane at x = ±(pitchHalfW + lineHalfW + 0.11),
/// i.e. the whole ball must cross the outer edge of the line. `side` is -1 for
/// the left goal, 1 for the right goal.
fn check_for_goal(
    side: f32,
    prev: Vec3,
    current: Vec3,
    predict_10ms: Vec3,
    pitch: &PitchConfig,
) -> bool {
    if predict_10ms.x.abs() < pitch.half_width - 1.0 {
        return false;
    }

    let plane_x = (pitch.half_width + pitch.line_half_width + 0.11) * side;

    // segment must cross the plane going outward
    let d_prev = prev.x - plane_x;
    let d_curr = current.x - plane_x;
    if !(d_prev * side <= 0.0 && d_curr * side > 0.0) {
        return false;
    }
    let denom = current.x - prev.x;
    if denom.abs() < 1e-9 {
        return false;
    }
    let t = (plane_x - prev.x) / denom;
    let hit = prev + (current - prev) * t;
    if hit.y.abs() >= pitch.goal_half_width || hit.z <= 0.0 || hit.z >= pitch.goal_height {
        return false;
    }

    // extra check: ball could have gone 'in' via the side netting, if segment
    // begin == outside of post but already behind the line. disallow!
    if prev.y.abs() > pitch.goal_half_width
        && prev.x.abs() > pitch.half_width - pitch.line_half_width - 0.11
    {
        return false;
    }

    true
}

fn referee_system(
    mut match_state: ResMut<MatchState>,
    pitch_config: Res<PitchConfig>,
    mut telemetry: ResMut<MatchTelemetry>,
    ball_query: Single<(&Position, &Ball)>,
) {
    // If a set piece is already pending/ticking down, don't check for new events
    if match_state.set_piece != SetPiece::None {
        return;
    }

    let (position, ball) = *ball_query;

    let pos = position.0;
    let prev = ball.previous_position;
    let predict_10ms = ball.predictions[1];
    let pitch_half_w = pitch_config.half_width;
    let pitch_half_h = pitch_config.half_height;
    let line_half_w = pitch_config.line_half_width;

    // 1. Goal Detection (swept, per goal side)
    if !match_state.is_ball_in_goal {
        for side in [-1.0f32, 1.0] {
            if check_for_goal(side, prev, pos, predict_10ms, &pitch_config) {
                if side < 0.0 {
                    // Away team scores (in the left goal, defended by home)
                    match_state.away_score += 1;
                    // the conceding team kicks off (Law 8)
                    match_state.set_piece_team = Some(TeamId::Home);
                } else {
                    match_state.home_score += 1;
                    match_state.set_piece_team = Some(TeamId::Away);
                }
                match_state.is_ball_in_goal = true;
                match_state.set_piece = SetPiece::KickOff;
                // original: 6000 ms celebration + 2000 ms preparation
                match_state.restart_in = Duration::from_secs_f32(8.0);
                match_state.restart_pos = Vec3::ZERO;
                let scored_by = if side < 0.0 {
                    TeamId::Away
                } else {
                    TeamId::Home
                };
                telemetry.record(MatchFact::Goal { scored_by });
                telemetry.record(MatchFact::RestartAwarded {
                    set_piece: SetPiece::KickOff,
                    team: scored_by.opponent(),
                });
                return;
            }
        }
    }

    // Out-of-play detection: the whole ball must be past the outer edge of the
    // line (original referee.cpp: fabs(pos) > pitchHalf + lineHalfW + 0.11)
    let last_touch = ball.last_touch_team.unwrap_or(TeamId::Home);
    // side of the pitch the last touching team defends (-1 left for home)
    let last_side = crate::team_tactics::team_side(last_touch);

    // 2. Over the backline: corner or goal kick
    if pos.x.abs() > pitch_half_w + line_half_w + 0.11 {
        let taking_team = last_touch.opponent();
        if pos.x * last_side > 0.0 {
            // last touch by the team defending this side -> corner for the attackers
            match_state.set_piece = SetPiece::Corner;
            match_state.set_piece_team = Some(taking_team);
            match_state.restart_in = Duration::from_secs_f32(4.0);
            match_state.restart_pos = Vec3::new(
                pitch_half_w * last_side,
                if pos.y > 0.0 {
                    pitch_half_h
                } else {
                    -pitch_half_h
                },
                0.0,
            );
            telemetry.record(MatchFact::RestartAwarded {
                set_piece: SetPiece::Corner,
                team: taking_team,
            });
        } else {
            match_state.set_piece = SetPiece::GoalKick;
            match_state.set_piece_team = Some(taking_team);
            match_state.restart_in = Duration::from_secs_f32(4.0);
            match_state.restart_pos = Vec3::new(pitch_half_w * 0.92 * -last_side, 0.0, 0.0);
            telemetry.record(MatchFact::RestartAwarded {
                set_piece: SetPiece::GoalKick,
                team: taking_team,
            });
        }
    }
    // 3. Over the sideline: throw-in
    else if pos.y.abs() > pitch_half_h + line_half_w + 0.11 {
        let throw_in_team = last_touch.opponent();
        match_state.set_piece = SetPiece::ThrowIn;
        match_state.set_piece_team = Some(throw_in_team);
        match_state.restart_in = Duration::from_secs_f32(4.0);
        match_state.restart_pos = Vec3::new(
            pos.x.clamp(-pitch_half_w + 0.6, pitch_half_w - 0.6),
            if pos.y > 0.0 {
                pitch_half_h
            } else {
                -pitch_half_h
            },
            0.0,
        );
        telemetry.record(MatchFact::RestartAwarded {
            set_piece: SetPiece::ThrowIn,
            team: throw_in_team,
        });
    }
}

/// Port of `Referee::BallTouched()`: on every touch, first whistle if the toucher
/// was recorded offside at the previous touch, then re-record teammates standing
/// beyond the offside line (`AI_GetOffsideLine`) at this moment.
fn referee_offside_system(
    mut match_state: ResMut<MatchState>,
    mut records: ResMut<OffsideRecords>,
    pitch_config: Res<PitchConfig>,
    mut touches: MessageReader<BallTouched>,
    mut telemetry: ResMut<MatchTelemetry>,
    player_query: Query<(Entity, &Position, &Player)>,
    ball_query: Single<&Position, With<Ball>>,
) {
    for touch in touches.read() {
        if match_state.set_piece != SetPiece::None {
            records.players.clear();
            records.team = None;
            records.judged_line_x = None;
            records.judged_against_team = None;
            continue;
        }

        // offside player receiving the ball?
        if records.team == Some(touch.team())
            && let Some((_, recorded_pos)) = records
                .players
                .iter()
                .find(|(id, _)| *id == touch.player)
                .copied()
        {
            match_state.set_piece = SetPiece::FreeKick;
            match_state.set_piece_team = Some(touch.team().opponent());
            match_state.restart_in = Duration::from_secs_f32(4.0);
            match_state.restart_pos = Vec3::new(recorded_pos.x, recorded_pos.y, 0.0);
            records.players.clear();
            records.team = None;
            telemetry.record(MatchFact::OffsideGiven {
                against: touch.player,
            });
            telemetry.record(MatchFact::RestartAwarded {
                set_piece: SetPiece::FreeKick,
                team: touch.team().opponent(),
            });
            continue;
        }

        records.players.clear();
        records.team = Some(touch.team());

        let bodies: Vec<(PlayerId, Vec3)> = player_query
            .iter()
            .map(|(_, position, player)| (player.id, position.0))
            .collect();
        let judged = judge_offside_positions(&bodies, touch.player, ball_query.0.x, &pitch_config);

        records.judged_line_x = Some(judged.line_x);
        records.judged_against_team = Some(touch.team().opponent());
        records.players = judged.beyond_the_line;
    }
}

/// Un jugador cuenta como adelantado sólo si lo está claramente: veinte
/// centímetros, que es el margen con el que el original evita anotar a quien
/// está a la altura de la línea.
const OFFSIDE_TOLERANCE: f32 = 0.20;

/// Lo que el árbitro juzgó en un toque: contra qué línea, y quién estaba por
/// delante de ella.
#[derive(Debug, Clone, PartialEq)]
pub struct OffsideJudgement {
    pub line_x: f32,
    pub beyond_the_line: Vec<(PlayerId, Vec3)>,
}

/// La línea de fuera de juego en el momento del toque, y los compañeros del que
/// tocó que están por delante de ella (`AI_GetOffsideLine`).
///
/// La línea es el penúltimo defensor o el balón, el que esté más cerca de la
/// portería defendida, y nunca dentro del campo del que ataca. El signo importa
/// más que el valor: invertido anota a media plantilla, y como un anotado no
/// disputa el balón, congela al equipo en vez de pitar fuera de juego.
pub fn judge_offside_positions(
    bodies: &[(PlayerId, Vec3)],
    touched_by: PlayerId,
    ball_x: f32,
    pitch: &PitchConfig,
) -> OffsideJudgement {
    let attacking_team = touched_by.team;
    let defending_team = attacking_team.opponent();
    // hacia dónde defiende el rival, y por tanto hacia dónde se ataca
    let def_side = crate::team_tactics::team_side(defending_team);
    let attacking_towards_x = -crate::team_tactics::team_side(attacking_team);

    let defenders = || bodies.iter().filter(|(id, _)| id.team == defending_team);
    let mut deepest: Option<(PlayerId, f32)> = None;
    for (id, pos) in defenders() {
        let depth = pos.x * def_side;
        if deepest.is_none_or(|(_, best)| depth > best) {
            deepest = Some((*id, depth));
        }
    }
    let deepest = deepest.map(|(id, _)| id);
    let mut line_x = 0.0f32;
    for (id, pos) in defenders() {
        if Some(*id) == deepest {
            continue;
        }
        if pos.x * def_side > line_x * def_side {
            line_x = pos.x;
        }
    }

    if ball_x * def_side > line_x * def_side {
        line_x = ball_x;
    }
    if line_x * def_side < 0.0 {
        line_x = 0.01 * -def_side;
    }
    let line_x = line_x.clamp(-pitch.half_width, pitch.half_width);

    let beyond_the_line = bodies
        .iter()
        .filter(|(id, _)| id.team == attacking_team && *id != touched_by)
        .filter(|(_, pos)| {
            pos.x * attacking_towards_x > line_x * attacking_towards_x + OFFSIDE_TOLERANCE
        })
        .copied()
        .collect();

    OffsideJudgement {
        line_x,
        beyond_the_line,
    }
}

/// Reset System:
/// Ticks down the set piece timer. When it expires, places the ball at the
/// restart position recorded when play was stopped, teleports players to their
/// base positions and resets all state (original: `ResetSituation`).
///
/// Lo que **no** hace, y es la deuda que abrió el sesgo local de MVP 2: nadie
/// ejecuta la reanudación. La formación base es un espejo exacto, así que los
/// dos equipos salen a la vez y el balón se lo lleva quien gane un desempate.
fn referee_set_piece_system(
    mut match_state: ResMut<MatchState>,
    mut records: ResMut<OffsideRecords>,
    time: Res<Time>,
    mut ball_query: Query<(&mut Position, &mut Ball), Without<Player>>,
    mut player_query: Query<
        (
            &mut Position,
            &mut Facing,
            &mut Velocity,
            &Player,
            &mut PlayerMatchState,
        ),
        Without<Ball>,
    >,
) {
    if match_state.set_piece == SetPiece::None {
        return;
    }

    // The match is over: the pending kick-off is one that will never be taken.
    if match_state.phase.is_over() {
        return;
    }

    if !match_state.restart_in.is_zero() {
        match_state.restart_in = match_state.restart_in.saturating_sub(time.delta());
        // dead ball: park it at the restart spot right away, or it keeps
        // rolling into the stands while the restart timer runs
        if let Ok((mut ball_position, mut ball)) = ball_query.single_mut() {
            let restart_pos = match_state.restart_pos;
            if ball_position.0.distance(restart_pos) > 0.3 {
                ball.reset(restart_pos);
                ball_position.0 = restart_pos + Vec3::new(0.0, 0.0, BALL_RADIUS);
            }
        }
        return;
    }

    // Timer expired! Execute reset.
    let Ok((mut ball_position, mut ball)) = ball_query.single_mut() else {
        return;
    };

    let restart_pos = match_state.restart_pos;
    ball.reset(restart_pos);
    ball_position.0 = restart_pos + Vec3::new(0.0, 0.0, BALL_RADIUS);

    // Re-form both teams at their base positions, facing the opponent goal
    for (mut position, mut facing, mut velocity, player, mut player_state) in
        player_query.iter_mut()
    {
        let base = base_formation_position(player.id, player.position);
        *position = Position::from_pitch(base, 0.0);
        facing.0 = match player.id.team {
            TeamId::Home => Dir2::X,
            TeamId::Away => Dir2::NEG_X,
        };
        velocity.0 = Vec3::ZERO;
        player_state.last_touch_at = Duration::ZERO;
    }

    // Clear set piece state
    let prev_set_piece = match_state.set_piece;
    match_state.set_piece = SetPiece::None;
    match_state.set_piece_team = None;
    match_state.is_ball_in_goal = false;
    match_state.possession_player = None;
    match_state.possession_team = None;
    match_state.previous_possessor = None;
    match_state.possession_since = Duration::ZERO;
    records.players.clear();
    records.team = None;

    let _ = prev_set_piece;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_requires_whole_ball_over_the_line() {
        let pitch = PitchConfig::default();
        let far_predict = Vec3::new(56.0, 0.0, 0.11);

        // ball center barely past the line but not fully over: no goal yet
        let prev = Vec3::new(54.8, 0.0, 0.5);
        let current = Vec3::new(55.05, 0.0, 0.5);
        assert!(!check_for_goal(1.0, prev, current, far_predict, &pitch));

        // whole ball (center past 55 + 0.06 + 0.11) crossed: goal
        let current = Vec3::new(55.3, 0.0, 0.5);
        assert!(check_for_goal(1.0, prev, current, far_predict, &pitch));
    }

    #[test]
    fn test_fast_shot_does_not_tunnel() {
        let pitch = PitchConfig::default();
        let far_predict = Vec3::new(60.0, 0.0, 0.11);
        // 40 m/s shot moves 0.4 m per 10 ms tick: an instantaneous check could
        // miss the mouth, the swept segment must not
        let prev = Vec3::new(54.9, 1.0, 1.0);
        let current = Vec3::new(55.6, 1.05, 1.0);
        assert!(check_for_goal(1.0, prev, current, far_predict, &pitch));
    }

    #[test]
    fn test_no_goal_through_side_netting() {
        let pitch = PitchConfig::default();
        let far_predict = Vec3::new(56.0, 0.0, 0.11);
        // segment starting almost on the line and outside the post, crossing the
        // plane inside the mouth: the original explicitly disallows this
        let prev = Vec3::new(55.16, 3.8, 0.5);
        let current = Vec3::new(55.30, 2.0, 0.5);
        assert!(!check_for_goal(1.0, prev, current, far_predict, &pitch));
    }

    #[test]
    fn test_shot_over_the_bar_is_not_a_goal() {
        let pitch = PitchConfig::default();
        let far_predict = Vec3::new(56.0, 0.0, 3.5);
        let prev = Vec3::new(54.0, 0.0, 3.4);
        let current = Vec3::new(55.5, 0.0, 3.3);
        assert!(!check_for_goal(1.0, prev, current, far_predict, &pitch));
    }

    fn body(team: TeamId, shirt: u8, x: f32) -> (PlayerId, Vec3) {
        (PlayerId { team, shirt }, Vec3::new(x, 0.0, 0.0))
    }

    /// El fuera de juego se anota **por delante** de la línea, no por detrás.
    ///
    /// Es el test que faltaba: el signo estuvo invertido desde el port y anotaba
    /// a los 9,4 jugadores que estaban en su sitio en vez de al que se había
    /// adelantado. Local ataca hacia +x.
    #[test]
    fn only_the_player_ahead_of_the_line_is_recorded() {
        let pitch = PitchConfig::default();
        let passer = PlayerId {
            team: TeamId::Home,
            shirt: 10,
        };
        let bodies = [
            body(TeamId::Home, 10, 0.0), // el que toca, en el centro
            body(TeamId::Home, 9, 30.0), // adelantado: pasado el penúltimo
            body(TeamId::Home, 8, 10.0), // en su sitio, detrás de la línea
            body(TeamId::Away, 1, 52.0), // portero: el más profundo
            body(TeamId::Away, 4, 25.0), // penúltimo: pone la línea
            body(TeamId::Away, 6, 15.0),
        ];

        let judged = judge_offside_positions(&bodies, passer, 0.0, &pitch);

        assert_eq!(judged.line_x, 25.0);
        let flagged: Vec<u8> = judged
            .beyond_the_line
            .iter()
            .map(|(id, _)| id.shirt)
            .collect();
        assert_eq!(flagged, vec![9]);
    }

    /// La línea la puede poner el balón, y el visitante ataca hacia -x: el mismo
    /// juicio con los dos signos cambiados tiene que dar lo simétrico.
    #[test]
    fn the_ball_can_be_the_line_and_the_sides_are_symmetric() {
        let pitch = PitchConfig::default();
        let passer = PlayerId {
            team: TeamId::Away,
            shirt: 10,
        };
        let bodies = [
            body(TeamId::Away, 10, 0.0),
            body(TeamId::Away, 9, -45.0), // adelantado respecto al balón
            body(TeamId::Away, 8, -10.0),
            body(TeamId::Home, 1, -52.0),
            body(TeamId::Home, 4, -20.0), // penúltimo, menos profundo que el balón
        ];

        // el balón está más cerca de la portería defendida que el penúltimo
        // defensor: entonces la línea la pone el balón
        let judged = judge_offside_positions(&bodies, passer, -35.0, &pitch);

        assert_eq!(judged.line_x, -35.0);
        let flagged: Vec<u8> = judged
            .beyond_the_line
            .iter()
            .map(|(id, _)| id.shirt)
            .collect();
        assert_eq!(flagged, vec![9]);
    }
}
