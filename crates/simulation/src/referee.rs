use crate::SimulationSet;
use crate::diagnostics::{MatchFact, MatchTelemetry};
use crate::match_setup::base_formation_position;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use football_domain::math::normalized_or_2d;
use football_domain::{
    BALL_RADIUS, Ball, BallTouched, Facing, MatchState, OffsideRecords, PitchConfig, Player,
    PitchSides, PlayerId, PlayerMatchState, PlayingPosition, Position, PotentialFoul, SetPiece,
    TeamId, Velocity,
};
use std::time::Duration;

pub struct RefereePlugin;

impl Plugin for RefereePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(OffsideRecords::default()).add_systems(
            FixedUpdate,
            (
                referee_offside_system,
                referee_foul_system,
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

/// El árbitro juzga los contactos: hoy pita todos los que le llegan.
///
/// La ventaja (Ley 5) y la disciplina son lo siguiente, y entran aquí sin tocar
/// nada más: la decisión ya está separada del incidente, que es lo que costaba.
fn referee_foul_system(
    mut match_state: ResMut<MatchState>,
    mut fouls: MessageReader<PotentialFoul>,
    mut telemetry: ResMut<MatchTelemetry>,
) {
    if match_state.set_piece != SetPiece::None {
        fouls.clear();
        return;
    }
    let Some(foul) = fouls.read().next().copied() else {
        return;
    };
    fouls.clear();

    let awarded_to = foul.on.team;
    match_state.set_piece = SetPiece::FreeKick;
    match_state.set_piece_team = Some(awarded_to);
    match_state.restart_in = Duration::from_secs_f32(3.0);
    match_state.restart_pos = Vec3::new(foul.at.x, foul.at.y, 0.0);
    telemetry.record(MatchFact::FoulGiven {
        by: foul.by,
        on: foul.on,
    });
    telemetry.record(MatchFact::RestartAwarded {
        set_piece: SetPiece::FreeKick,
        team: awarded_to,
    });
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
    let last_side = match_state.sides.defending_x(last_touch);

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
        let judged = judge_offside_positions(
            &bodies,
            touch.player,
            ball_query.0.x,
            match_state.sides,
            &pitch_config,
        );

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
    sides: PitchSides,
    pitch: &PitchConfig,
) -> OffsideJudgement {
    let attacking_team = touched_by.team;
    let defending_team = attacking_team.opponent();
    // hacia dónde defiende el rival, y por tanto hacia dónde se ataca
    let def_side = sides.defending_x(defending_team);
    let attacking_towards_x = sides.attacking_x(attacking_team);

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

/// A qué distancia del balón se planta quien saca, en metros. Dentro de
/// `ball_at_feet_distance` (0,7): el balón está en su pie, no a su alcance.
const TAKER_DISTANCE: f32 = 0.4;

/// Lo que la Ley 13 exige que se aparten los rivales, en metros, salvo en el
/// saque de banda (Ley 15), donde son dos.
const OPPONENT_CLEARANCE: f32 = 9.15;
const THROW_IN_CLEARANCE: f32 = 2.0;

/// Quién saca: el portero en el saque de puerta, y en el resto el jugador de
/// campo que tenga más cerca el balón.
///
/// El equipo lo decide la regla antes que esto; aquí no hay desempate entre
/// equipos, que es de donde salía el sesgo local: dos formaciones espejo
/// empatan en todo y los tres desempates iban al mismo lado.
fn select_restart_taker(
    bodies: &[(PlayerId, PlayingPosition, Vec2)],
    taking_team: TeamId,
    set_piece: SetPiece,
    restart_pos: Vec2,
) -> Option<PlayerId> {
    if set_piece == SetPiece::GoalKick {
        return bodies
            .iter()
            .find(|(id, position, _)| {
                id.team == taking_team && *position == PlayingPosition::Goalkeeper
            })
            .map(|(id, _, _)| *id);
    }

    bodies
        .iter()
        .filter(|(id, position, _)| {
            id.team == taking_team && *position != PlayingPosition::Goalkeeper
        })
        .min_by(|(_, _, left), (_, _, right)| {
            left.distance_squared(restart_pos)
                .total_cmp(&right.distance_squared(restart_pos))
        })
        .map(|(id, _, _)| *id)
}

/// A qué distancia se ofrece el compañero de apoyo, en metros.
///
/// Sin él, sacar es una desventaja: el que saca queda solo contra un bloque
/// entero, pierde el balón cerca de su área y el rival contraataca. Como saca
/// quien encaja, eso se realimenta hasta la goleada. La Ley solo aparta a los
/// rivales; a los compañeros los deja acercarse, y esto es lo que hacen.
const SUPPORT_DISTANCE: f32 = 6.0;
const SUPPORT_WIDTH: f32 = 4.0;

/// Dónde se ofrece el apoyo: por detrás del sacador y abierto hacia su propia
/// banda, que es de donde ya venía —así el espejo entre equipos se mantiene—.
fn restart_support_spot(restart_pos: Vec2, attacking_towards_x: f32, support_y: f32) -> Vec2 {
    let towards_own_half = -attacking_towards_x;
    let side_of_the_pitch = if support_y >= 0.0 { 1.0 } else { -1.0 };
    restart_pos
        + Vec2::new(
            towards_own_half * SUPPORT_DISTANCE,
            side_of_the_pitch * SUPPORT_WIDTH,
        )
}

/// Dónde se planta quien saca: detrás del balón desde su punto de vista, para
/// que el primer golpeo salga hacia la portería contraria y no hacia la propia.
/// El saque de banda se ejecuta desde fuera del campo (Ley 15), así que ahí el
/// paso atrás es hacia la grada.
fn restart_taker_spot(restart_pos: Vec2, attacking_towards_x: f32, set_piece: SetPiece) -> Vec2 {
    if set_piece == SetPiece::ThrowIn {
        let outward = if restart_pos.y >= 0.0 { 1.0 } else { -1.0 };
        return restart_pos + Vec2::new(0.0, outward * TAKER_DISTANCE);
    }
    restart_pos - Vec2::new(attacking_towards_x * TAKER_DISTANCE, 0.0)
}

/// Aparta radialmente lo justo: quien ya respeta la distancia no se mueve, y
/// quien no, sale por donde estaba en vez de teletransportarse a un sitio nuevo.
///
/// Quien esté justo encima del balón retrocede hacia su propia portería, que es
/// lo que hace un defensa y además es simétrico: una dirección fija de reserva
/// —`Vec2::X`— apartaría a los dos equipos hacia el mismo lado del campo.
fn cleared_position(
    position: Vec2,
    restart_pos: Vec2,
    clearance: f32,
    towards_own_goal: Vec2,
) -> Vec2 {
    let offset = position - restart_pos;
    if offset.length() >= clearance {
        return position;
    }
    let direction = normalized_or_2d(offset, towards_own_goal);
    restart_pos + direction * clearance
}

/// Reset System:
/// Ticks down the set piece timer. When it expires, places the ball at the
/// restart position recorded when play was stopped, teleports players to their
/// base positions and hands the ball to whoever takes the restart.
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
    let sides = match_state.sides;
    for (mut position, mut facing, mut velocity, player, mut player_state) in
        player_query.iter_mut()
    {
        let base = base_formation_position(player.id, player.position, sides);
        *position = Position::from_pitch(base, 0.0);
        facing.0 = if sides.attacking_x(player.id.team) > 0.0 {
            Dir2::X
        } else {
            Dir2::NEG_X
        };
        velocity.0 = Vec3::ZERO;
        player_state.last_touch_at = Duration::ZERO;
    }

    let prev_set_piece = match_state.set_piece;

    // Someone takes the restart. Standing them on the ball is all it takes:
    // possession is positional, so `ball_contest` hands it over next tick —
    // awarding it here would make the referee a second owner of possession.
    if let Some(taking_team) = match_state.set_piece_team {
        let restart_2d = restart_pos.truncate();
        let bodies: Vec<(PlayerId, PlayingPosition, Vec2)> = player_query
            .iter()
            .map(|(position, _, _, player, _)| {
                (player.id, player.position, position.on_pitch())
            })
            .collect();
        let taker = select_restart_taker(&bodies, taking_team, prev_set_piece, restart_2d);
        let support = taker.and_then(|taker| {
            let others: Vec<(PlayerId, PlayingPosition, Vec2)> = bodies
                .iter()
                .filter(|(id, _, _)| *id != taker)
                .copied()
                .collect();
            select_restart_taker(&others, taking_team, SetPiece::None, restart_2d)
        });
        let attacking_towards_x = sides.attacking_x(taking_team);
        let clearance = if prev_set_piece == SetPiece::ThrowIn {
            THROW_IN_CLEARANCE
        } else {
            OPPONENT_CLEARANCE
        };

        for (mut position, mut facing, _, player, _) in player_query.iter_mut() {
            if Some(player.id) == taker {
                let spot = restart_taker_spot(restart_2d, attacking_towards_x, prev_set_piece);
                *position = Position::from_pitch(spot, 0.0);
                if let Ok(towards_ball) = Dir2::new(restart_2d - spot) {
                    facing.0 = towards_ball;
                }
            } else if Some(player.id) == support {
                let spot = restart_support_spot(
                    restart_2d,
                    attacking_towards_x,
                    position.on_pitch().y,
                );
                *position = Position::from_pitch(spot, 0.0);
            } else if player.id.team != taking_team {
                let towards_own_goal = Vec2::new(-attacking_towards_x, 0.0);
                let cleared =
                    cleared_position(position.on_pitch(), restart_2d, clearance, towards_own_goal);
                *position = Position::from_pitch(cleared, 0.0);
            }
        }
    }

    match_state.set_piece = SetPiece::None;
    match_state.set_piece_team = None;
    match_state.is_ball_in_goal = false;
    match_state.possession_player = None;
    match_state.possession_team = None;
    match_state.previous_possessor = None;
    match_state.possession_since = Duration::ZERO;
    records.players.clear();
    records.team = None;

}

#[cfg(test)]
mod tests {
    use super::*;

    fn mirrored_teams(restart: Vec2) -> Vec<(PlayerId, PlayingPosition, Vec2)> {
        vec![
            (
                PlayerId::new(TeamId::Home, 1),
                PlayingPosition::Goalkeeper,
                Vec2::new(-54.0, 0.0),
            ),
            (
                PlayerId::new(TeamId::Home, 9),
                PlayingPosition::CentreForward,
                restart + Vec2::new(3.0, 0.0),
            ),
            (
                PlayerId::new(TeamId::Away, 1),
                PlayingPosition::Goalkeeper,
                Vec2::new(54.0, 0.0),
            ),
            (
                PlayerId::new(TeamId::Away, 9),
                PlayingPosition::CentreForward,
                restart + Vec2::new(-3.0, 0.0),
            ),
        ]
    }

    /// Quien saca sale del equipo al que se le concedió, y de nadie más: es el
    /// desempate que antes decidía la posesión y siempre caía del mismo lado.
    #[test]
    fn the_restart_is_taken_by_the_team_it_was_awarded_to() {
        let restart = Vec2::new(0.0, 0.0);
        let bodies = mirrored_teams(restart);

        for team in TeamId::BOTH {
            let taker = select_restart_taker(&bodies, team, SetPiece::KickOff, restart)
                .expect("alguien saca");
            assert_eq!(taker.team, team);
        }
    }

    /// El saque de puerta lo saca el portero aunque tenga a un delantero encima
    /// del balón.
    #[test]
    fn the_goalkeeper_takes_the_goal_kick() {
        let restart = Vec2::new(-50.0, 0.0);
        let bodies = mirrored_teams(restart);

        let taker = select_restart_taker(&bodies, TeamId::Home, SetPiece::GoalKick, restart)
            .expect("alguien saca");
        assert_eq!(taker, PlayerId::new(TeamId::Home, 1));

        let open_play = select_restart_taker(&bodies, TeamId::Home, SetPiece::Corner, restart)
            .expect("alguien saca");
        assert_ne!(
            open_play,
            PlayerId::new(TeamId::Home, 1),
            "el portero no saca los córners"
        );
    }

    /// Quien saca queda con el balón en el pie —dentro de
    /// `ball_at_feet_distance`— y por detrás de él, no pasado.
    #[test]
    fn the_taker_stands_on_the_ball_and_behind_it() {
        let restart = Vec2::new(0.0, 0.0);
        let towards_x = 1.0;

        let spot = restart_taker_spot(restart, towards_x, SetPiece::KickOff);
        assert!(spot.distance(restart) < 0.7, "el balón no está en su pie");
        assert!(spot.x < restart.x, "saca de espaldas a la portería rival");
    }

    /// El saque de banda se ejecuta desde fuera del campo (Ley 15).
    #[test]
    fn the_throw_in_is_taken_from_outside_the_pitch() {
        let restart = Vec2::new(10.0, 36.0);
        let spot = restart_taker_spot(restart, 1.0, SetPiece::ThrowIn);
        assert!(spot.y > restart.y, "el sacador pisa el campo");
        assert!(spot.distance(restart) < 0.7);
    }

    /// Los rivales se apartan lo que exige la ley, y quien ya estaba lejos no se
    /// mueve: apartar a todos rehace la formación en vez de despejar el balón.
    #[test]
    fn opponents_are_moved_only_as_far_as_the_law_asks() {
        let restart = Vec2::new(0.0, 0.0);

        let crowding = Vec2::new(1.0, 0.0);
        let cleared = cleared_position(crowding, restart, OPPONENT_CLEARANCE, Vec2::NEG_X);
        assert!((cleared.distance(restart) - OPPONENT_CLEARANCE).abs() < 1e-3);
        assert!(cleared.x > 0.0, "salió por el lado contrario al que estaba");

        let far_away = Vec2::new(30.0, 5.0);
        assert_eq!(
            cleared_position(far_away, restart, OPPONENT_CLEARANCE, Vec2::NEG_X),
            far_away
        );
    }

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

        let judged = judge_offside_positions(&bodies, passer, 0.0, PitchSides::opening(), &pitch);

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
        let judged = judge_offside_positions(&bodies, passer, -35.0, PitchSides::opening(), &pitch);

        assert_eq!(judged.line_x, -35.0);
        let flagged: Vec<u8> = judged
            .beyond_the_line
            .iter()
            .map(|(id, _)| id.shirt)
            .collect();
        assert_eq!(flagged, vec![9]);
    }
}
