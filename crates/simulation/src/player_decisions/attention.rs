//! Adónde decide mirar un jugador, separado de adónde decide correr.

use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use std::time::Duration;

use crate::perception::Beliefs;
use crate::team_tactics::SPRINT_VELOCITY;
use football_domain::tuning::PerceptionTuning;
use football_domain::{
    Gaze, MatchState, MatchTuning, MovementIntent, Player, PlayerId, Position, Vision,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanSide {
    Left,
    Right,
}

impl ScanSide {
    fn angle_sign(self) -> f32 {
        match self {
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

/// Reparte los barridos en el tiempo para que los veintidós no giren juntos y
/// alterna el hombro explorado. La identidad basta: no hace falta RNG ni estado
/// mutable para producir la misma atención con la misma corrida (§5).
fn scan_side_at(now: Duration, who: PlayerId, tuning: &PerceptionTuning) -> Option<ScanSide> {
    let interval = tuning.scan_interval.as_millis();
    let duration = tuning.scan_duration.as_millis().min(interval);
    if interval == 0 || duration == 0 {
        return None;
    }

    let player_slot = (who.team.index() * 11 + usize::from(who.shirt.saturating_sub(1))) % 22;
    let stagger = interval * player_slot as u128 / 22;
    let elapsed = now.as_millis() + stagger;
    if elapsed % interval >= duration {
        return None;
    }

    match (elapsed / interval) % 2 {
        0 => Some(ScanSide::Left),
        1 => Some(ScanSide::Right),
        _ => unreachable!("el resto módulo dos solo puede ser cero o uno"),
    }
}

/// Punto que pone el cono al costado y atrás. El ángulo sale del propio cono:
/// uno de sus bordes mira justo hacia atrás y el balón queda fuera del otro.
fn scan_target(eyes: Vec2, ball: Vec2, vision: &Vision, side: ScanSide) -> Option<Vec2> {
    let towards_ball = Dir2::new(ball - eyes).ok()?;
    let angle = side.angle_sign() * (std::f32::consts::PI - vision.half_angle);
    let direction = Vec2::from_angle(angle).rotate(*towards_ball);
    Some(eyes + direction * vision.range)
}

struct AttentionSituation<'a> {
    now: Duration,
    who: PlayerId,
    possesses_ball: bool,
    desired_velocity: Vec2,
    eyes: Vec2,
    ball: Option<Vec2>,
    /// Cuánto duda de dónde está el balón, en metros.
    ball_doubt: f32,
    vision: &'a Vision,
}

fn attention_target(situation: AttentionSituation<'_>, tuning: &PerceptionTuning) -> Option<Vec2> {
    let ball = situation.ball?;
    if situation.possesses_ball {
        return Some(ball);
    }
    if situation.desired_velocity.length() >= SPRINT_VELOCITY {
        return None;
    }
    // Quien ya no sabe dónde está el balón no reconoce el entorno: lo busca.
    // El barrido es lo que se hace teniéndolo situado, y por eso la duda manda
    // sobre la cadencia y no al revés.
    if situation.ball_doubt >= tuning.lost_ball_doubt {
        return Some(ball);
    }
    scan_side_at(situation.now, situation.who, tuning)
        .and_then(|side| scan_target(situation.eyes, ball, situation.vision, side))
        .or(Some(ball))
}

/// La intención motora ya está decidida; con ella se sabe si se puede apartar
/// la vista o si la carrera obliga a mirar hacia donde se va. Solo este sistema
/// escribe `Gaze` (§2).
pub(crate) fn direct_visual_attention(
    time: Res<Time>,
    match_state: Res<MatchState>,
    tuning: Res<MatchTuning>,
    beliefs: Res<Beliefs>,
    mut players: Query<(&Position, &Player, &Vision, &MovementIntent, &mut Gaze)>,
) {
    for (position, player, vision, intent, mut gaze) in players.iter_mut() {
        gaze.0 = attention_target(
            AttentionSituation {
                now: time.elapsed(),
                who: player.id,
                possesses_ball: match_state.possession_player == Some(player.id),
                desired_velocity: intent.0.truncate(),
                eyes: position.on_pitch(),
                ball: beliefs.ball_of(player.id),
                ball_doubt: beliefs.ball_uncertainty_of(player.id),
                vision,
            },
            &tuning.perception,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use football_domain::can_see;

    #[test]
    fn a_scan_alternates_sides_and_returns_to_the_ball() {
        let tuning = PerceptionTuning::default();
        let player = PlayerId::home(1);

        assert_eq!(
            scan_side_at(Duration::ZERO, player, &tuning),
            Some(ScanSide::Left)
        );
        assert_eq!(scan_side_at(tuning.scan_duration, player, &tuning), None);
        assert_eq!(
            scan_side_at(tuning.scan_interval, player, &tuning),
            Some(ScanSide::Right)
        );
    }

    #[test]
    fn scanning_really_takes_the_ball_out_of_the_visual_field() {
        let eyes = Vec2::ZERO;
        let ball = Vec2::X * 10.0;
        let vision = Vision::default();

        for side in [ScanSide::Left, ScanSide::Right] {
            let target = scan_target(eyes, ball, &vision, side).expect("el balón da una dirección");
            let facing = Dir2::new(target - eyes).expect("el barrido da una dirección");
            assert!(!can_see(eyes, facing, ball, &vision));
        }
    }

    /// El momento del barrido según el metrónomo, para preguntar qué lo tumba.
    fn scanning_at(ball_doubt: f32) -> AttentionSituation<'static> {
        static VISION: Vision = Vision {
            half_angle: std::f32::consts::PI * 0.28,
            sharp_range: 12.0,
            range: 30.0,
        };
        AttentionSituation {
            now: Duration::ZERO,
            who: PlayerId::home(1),
            possesses_ball: false,
            desired_velocity: Vec2::ZERO,
            eyes: Vec2::ZERO,
            ball: Some(Vec2::X * 10.0),
            ball_doubt,
            vision: &VISION,
        }
    }

    #[test]
    fn possession_and_sprinting_override_a_scan() {
        let tuning = PerceptionTuning::default();
        let ball = Vec2::X * 10.0;

        assert_eq!(
            attention_target(
                AttentionSituation {
                    possesses_ball: true,
                    ..scanning_at(0.0)
                },
                &tuning,
            ),
            Some(ball)
        );
        assert_eq!(
            attention_target(
                AttentionSituation {
                    desired_velocity: Vec2::X * SPRINT_VELOCITY,
                    ..scanning_at(0.0)
                },
                &tuning,
            ),
            None
        );
    }

    /// Reconocer el entorno es un lujo de quien tiene el balón situado: perdido,
    /// la misma cadencia deja de barrer y se va a buscarlo.
    #[test]
    fn losing_the_ball_stops_the_scan_and_looks_for_it() {
        let tuning = PerceptionTuning::default();
        let ball = Vec2::X * 10.0;

        let scanning = attention_target(scanning_at(0.0), &tuning).expect("hay balón que mirar");
        assert_ne!(scanning, ball, "con el balón situado, este tick barre");

        assert_eq!(
            attention_target(scanning_at(tuning.lost_ball_doubt), &tuning),
            Some(ball)
        );
    }
}
