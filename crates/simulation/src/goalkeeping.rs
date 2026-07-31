//! Lo que pasa cuando el balón llega a la portería y hay alguien defendiéndola.
//!
//! El portero no para por sorteo: para lo que alcanza. Lo que decide es el
//! tiempo que el balón le concede, y por eso un disparo lejano y colocado se
//! ataja y uno de cerca a la escuadra no.

use bevy_ecs::prelude::*;
use bevy_math::prelude::*;

use crate::ball_contest::BallBody;
use crate::ball_physics::touch_ball;
use crate::diagnostics::{MatchFact, MatchTelemetry};
use football_domain::scenario::TICK;
use football_domain::tuning::GoalkeepingTuning;
use football_domain::{
    BALL_RADIUS, MatchState, MatchTuning, PitchConfig, Player, PlayingPosition, Position, SetPiece,
    TeamId,
};

/// Dónde y cuándo cruzaría el balón la línea de gol que defiende el portero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GoalBoundShot {
    /// Punto de cruce del plano de la portería.
    pub crossing: Vec3,
    /// Cuánto falta para que llegue, en segundos.
    pub time_to_arrival: f32,
}

/// Lo que el portero puede tocar cuando el balón le llega.
///
/// Es su envergadura estirado, y no depende del tiempo: el tiempo ya decidió
/// dónde está, porque colocarse es cosa de `goalie_movement`. Un disparo se ata
/// o entra según lo lejos que le pase, y le pasa lejos cuando no le dio tiempo
/// a moverse. Por eso la estirada no salva un tiro rápido y colocado.
pub fn diving_reach(tuning: &GoalkeepingTuning) -> f32 {
    tuning.dive_reach
}

/// Por dónde y cuándo pasa el balón por el plano `plane_x`, yendo hacia él.
pub fn crossing_of_plane(
    predictions: &[Vec3],
    plane_x: f32,
    towards: f32,
) -> Option<GoalBoundShot> {
    for (step, pair) in predictions.windows(2).enumerate() {
        let (from, to) = (pair[0], pair[1]);
        if (to.x - from.x) * towards <= 0.0 || (to.x - plane_x) * towards < 0.0 {
            continue;
        }
        let span = to.x - from.x;
        let along = if span.abs() < 1e-6 {
            0.0
        } else {
            ((plane_x - from.x) / span).clamp(0.0, 1.0)
        };
        return Some(GoalBoundShot {
            crossing: from + (to - from) * along,
            time_to_arrival: (step as f32 + along) * TICK.as_secs_f32(),
        });
    }
    None
}

/// Si la trayectoria prevista entra en la portería que defiende `goal_x`, dónde
/// y cuándo la cruza. `None` si el balón no va a puerta, que es la mayoría de
/// los ticks de un partido.
pub fn goal_bound_shot(
    predictions: &[Vec3],
    goal_x: f32,
    pitch: &PitchConfig,
) -> Option<GoalBoundShot> {
    let shot = crossing_of_plane(predictions, pitch.half_width * goal_x, goal_x)?;
    let inside_the_frame =
        shot.crossing.y.abs() < pitch.goal_half_width && shot.crossing.z < pitch.goal_height;
    inside_the_frame.then_some(shot)
}

/// Qué hace el portero con el balón que alcanza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Save {
    /// Lo atrapa: el balón se queda con él.
    Caught,
    /// Lo desvía: sigue vivo, y más lento, hacia donde lo mandó la mano.
    Parried,
}

/// Si el portero llega al disparo, y qué puede hacer con él.
///
/// Solo interviene sobre lo que está a punto de pasarle: mientras el balón viaja
/// no lo toca, se coloca. Atraparlo o rechazarlo depende de la velocidad a la
/// que le llegue.
pub fn attempt_save(
    keeper_pos: Vec2,
    shot: GoalBoundShot,
    ball_speed: f32,
    tuning: &GoalkeepingTuning,
) -> Option<Save> {
    if shot.time_to_arrival > tuning.reaction_window {
        return None;
    }
    let reach = diving_reach(tuning);
    let sideways = (shot.crossing.y - keeper_pos.y).abs();
    if sideways > reach || shot.crossing.z > tuning.reach_height {
        return None;
    }
    if ball_speed <= tuning.catchable_speed {
        Some(Save::Caught)
    } else {
        Some(Save::Parried)
    }
}

/// Hacia dónde sale el balón rechazado: lejos de la portería y hacia la banda
/// más cercana, que es adonde un portero lo manda cuando no puede retenerlo.
pub fn parry_momentum(incoming: Vec3, crossing_y: f32, goal_x: f32, pace: f32) -> Vec3 {
    let towards_pitch = -goal_x;
    let towards_touchline = if crossing_y >= 0.0 { 1.0 } else { -1.0 };
    let direction = Vec3::new(towards_pitch, towards_touchline, 0.35).normalize_or_zero();
    direction * incoming.length() * pace
}

/// El portero ataja lo que alcanza.
///
/// Solo toca el balón: quién lo tiene después lo decide `ball_contest`, que es
/// su dueño, y para un balón atrapado la respuesta es el propio portero porque
/// se le queda en las manos.
pub fn goalkeepers_save_shots(
    match_state: Res<MatchState>,
    tuning: Res<MatchTuning>,
    pitch: Res<PitchConfig>,
    mut telemetry: ResMut<MatchTelemetry>,
    mut ball_query: Query<BallBody, Without<Player>>,
    keeper_query: Query<(&Position, &Player)>,
) {
    if match_state.set_piece != SetPiece::None {
        return;
    }
    let Ok((mut ball_position, mut ball)) = ball_query.single_mut() else {
        return;
    };

    let keeping = &tuning.goalkeeping;
    let ball_speed = ball.momentum.length();

    for (position, player) in keeper_query.iter() {
        if player.position != PlayingPosition::Goalkeeper {
            continue;
        }
        let goal_x = defended_goal_x(player.id.team);
        if goal_bound_shot(&ball.predictions, goal_x, &pitch).is_none() {
            continue;
        }
        // Lo que puede tocar es lo que le pasa a él por delante, no lo que
        // cruza la línea diez metros por detrás.
        let Some(at_the_keeper) = crossing_of_plane(&ball.predictions, position.0.x, goal_x) else {
            continue;
        };
        let Some(save) = attempt_save(position.on_pitch(), at_the_keeper, ball_speed, keeping)
        else {
            continue;
        };
        let shot = at_the_keeper;

        let momentum = match save {
            Save::Caught => Vec3::ZERO,
            Save::Parried => parry_momentum(
                ball.momentum,
                shot.crossing.y,
                goal_x,
                keeping.parry_pace,
            ),
        };
        if save == Save::Caught {
            ball_position.0 = Vec3::new(
                position.0.x,
                position.0.y,
                BALL_RADIUS,
            );
        }
        touch_ball(&mut ball, &mut ball_position, momentum);
        ball.last_touch_team = Some(player.id.team);
        telemetry.record(MatchFact::ShotSaved {
            keeper: player.id,
            caught: save == Save::Caught,
        });
        return;
    }
}

/// El plano de gol que defiende un equipo, como signo sobre el eje x.
fn defended_goal_x(team: TeamId) -> f32 {
    crate::team_tactics::team_side(team)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shot_arriving_in(seconds: f32, y: f32, z: f32) -> GoalBoundShot {
        GoalBoundShot {
            crossing: Vec3::new(-55.0, y, z),
            time_to_arrival: seconds,
        }
    }

    /// Toca lo que le llega, no lo que va a llegarle: mientras el balón viaja,
    /// el portero se coloca.
    #[test]
    fn the_keeper_touches_the_ball_when_it_arrives_and_not_before() {
        let tuning = GoalkeepingTuning::default();
        let keeper = Vec2::new(-45.0, 0.0);

        let arriving = attempt_save(keeper, shot_arriving_in(0.05, 1.0, 0.5), 20.0, &tuning);
        assert_eq!(arriving, Some(Save::Parried));

        let still_travelling = attempt_save(keeper, shot_arriving_in(1.5, 1.0, 0.5), 20.0, &tuning);
        assert_eq!(still_travelling, None, "paró un balón a quince metros");
    }

    /// Lo que le pasa lejos no lo alcanza, y le pasa lejos cuando no le dio
    /// tiempo a colocarse.
    #[test]
    fn a_shot_placed_away_from_him_is_not_reached() {
        let tuning = GoalkeepingTuning::default();
        let keeper = Vec2::new(-45.0, 0.0);

        let at_him = shot_arriving_in(0.05, 1.0, 0.5);
        assert_eq!(attempt_save(keeper, at_him, 20.0, &tuning), Some(Save::Parried));

        let past_him = shot_arriving_in(0.05, tuning.dive_reach + 0.4, 0.5);
        assert_eq!(attempt_save(keeper, past_him, 20.0, &tuning), None);
    }

    /// La escuadra es inalcanzable aunque sobre tiempo: hay un techo.
    #[test]
    fn the_top_corner_stays_out_of_reach() {
        let tuning = GoalkeepingTuning::default();
        let keeper = Vec2::new(-54.0, 0.0);

        let high = shot_arriving_in(0.05, 0.5, tuning.reach_height + 0.3);
        assert_eq!(attempt_save(keeper, high, 20.0, &tuning), None);

        let wide = shot_arriving_in(0.05, tuning.dive_reach + 0.5, 0.5);
        assert_eq!(attempt_save(keeper, wide, 20.0, &tuning), None);
    }

    /// Lo que llega flojo se atrapa; lo que llega fuerte se rechaza.
    #[test]
    fn a_fierce_shot_is_parried_and_a_soft_one_is_caught() {
        let tuning = GoalkeepingTuning::default();
        let keeper = Vec2::new(-45.0, 0.0);
        let shot = shot_arriving_in(0.05, 0.5, 0.5);

        assert_eq!(
            attempt_save(keeper, shot, tuning.catchable_speed - 1.0, &tuning),
            Some(Save::Caught)
        );
        assert_eq!(
            attempt_save(keeper, shot, tuning.catchable_speed + 1.0, &tuning),
            Some(Save::Parried)
        );
    }

    /// El rechace sale del área, no hacia dentro de la portería.
    #[test]
    fn a_parry_sends_the_ball_away_from_the_goal() {
        let incoming = Vec3::new(-25.0, 0.0, 0.0);
        let parried = parry_momentum(incoming, 2.0, -1.0, 0.45);

        assert!(parried.x > 0.0, "el rechace entró en su propia portería");
        assert!(parried.y > 0.0, "no salió hacia la banda más cercana");
        assert!(parried.length() < incoming.length(), "rechazó más fuerte");
    }

    /// Un balón que no va entre los palos no es cosa del portero.
    #[test]
    fn a_ball_going_wide_is_not_a_shot_to_save() {
        let pitch = PitchConfig::default();
        let wide: Vec<Vec3> = (0..40)
            .map(|i| Vec3::new(-50.0 - i as f32 * 0.5, 20.0, 0.5))
            .collect();
        assert_eq!(goal_bound_shot(&wide, -1.0, &pitch), None);

        let on_target: Vec<Vec3> = (0..40)
            .map(|i| Vec3::new(-50.0 - i as f32 * 0.5, 1.0, 0.5))
            .collect();
        let shot = goal_bound_shot(&on_target, -1.0, &pitch).expect("va dentro");
        assert!(shot.time_to_arrival > 0.0);
    }
}
