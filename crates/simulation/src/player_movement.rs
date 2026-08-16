//! Los cuerpos: quién va a por el balón, cómo se mueven y cómo no se
//! superponen.
//!
//! Lo que pasa cuando uno de ellos alcanza el balón vive en
//! [`crate::ball_contest`] y [`crate::ball_release`].

use crate::SimulationSet;
use crate::force_field::{self, Falloff, ForceSpot};
use crate::perception::Beliefs;
use crate::player_decisions;
use crate::team_tactics::{self, TeamTactics};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;

use football_domain::math::{normalized_clamp, sign_side};
use football_domain::{
    Attributes, ByTeam, PLAYER_BODY_RADIUS, PitchSides, Player, PlayerId, PlayerMatchState,
    PlayingPosition, Position, PossessionDesignation, TeamId, Velocity,
};

pub struct PlayerMovementPlugin;

impl Plugin for PlayerMovementPlugin {
    fn build(&self, app: &mut App) {
        // `MatchRng` is deliberately not defaulted here: the seed belongs to the
        // scenario, which `MatchSetupPlugin` installs (law 11).
        app.insert_resource(PossessionDesignation::default())
            .init_resource::<TeamTactics>()
            .add_systems(
                FixedUpdate,
                (
                    update_possession_designation,
                    team_tactics::update_team_tactics,
                    team_tactics::assign_perceived_responsibilities,
                    player_decisions::select_player_movement,
                    player_decisions::direct_visual_attention,
                    crate::locomotion::drive_bodies,
                    apply_player_velocity,
                    resolve_player_overlap,
                )
                    .chain()
                    .in_set(SimulationSet::Players),
            );
    }
}

/// Designa, por equipo, a quien *cree* que llega primero al balón. No recibe
/// ni el poseedor ni el receptor verdaderos: cuando una cabeza no vio el balón
/// no puede ser elegida por conocimiento ajeno (§3). La designación es sólo un
/// desempate de coordinación; el contacto físico sigue resolviéndose aparte.
pub(crate) fn update_possession_designation(
    mut designation: ResMut<PossessionDesignation>,
    beliefs: Res<Beliefs>,
    player_query: Query<(&Position, &Player, &Attributes)>,
) {
    let mut best: ByTeam<Option<(PlayerId, f32)>> = ByTeam::default();
    for (position, player, stats) in player_query.iter() {
        if player.position == PlayingPosition::Goalkeeper {
            continue;
        }
        let path = beliefs.ball_path_of(player.id);
        if path.is_empty() {
            continue;
        }
        let (_, time_ms) = find_believed_interception(position.on_pitch(), stats.top_speed, path);
        let slot = &mut best[player.id.team];
        if slot.is_none_or(|(_, t)| time_ms < t) {
            *slot = Some((player.id, time_ms));
        }
    }

    for team in TeamId::BOTH {
        designation.designated[team] = best[team].map(|(id, _)| id);
        designation.time_to_ball_ms[team] = best[team].map_or(f32::MAX, |(_, t)| t);
    }
}

/// Kinematic integration of player velocities (players are not physics bodies,
/// as in the original). Also maintains the ~10 s average velocity used by
/// `GetLazyVelocity`. La orientación es del motor (`locomotion`), no de aquí.
fn apply_player_velocity(
    time: Res<Time>,
    mut query: Query<(&mut Position, &Velocity, &mut PlayerMatchState)>,
) {
    let dt = time.delta_secs();
    for (mut position, velocity, mut player_state) in query.iter_mut() {
        position.0.x += velocity.0.x * dt;
        position.0.y += velocity.0.y * dt;
        let speed = velocity.0.length();
        player_state.recent_speed += (speed - player_state.recent_speed) * (dt / 10.0).min(1.0);
    }
}

/// Positional separation between player bodies: dos cuerpos no pueden ocupar el
/// mismo sitio, así que las parejas superpuestas se separan. Sin esto los dos
/// designados se superponen sobre el balón en disputa.
fn resolve_player_overlap(mut query: Query<&mut Position, With<Player>>) {
    const MIN_DIST: f32 = PLAYER_BODY_RADIUS * 2.0;

    let mut combinations = query.iter_combinations_mut();
    while let Some([mut a, mut b]) = combinations.fetch_next() {
        let mut delta = b.0 - a.0;
        delta.z = 0.0;
        let dist = delta.length();
        if dist >= MIN_DIST {
            continue;
        }
        // fully overlapping: pick an arbitrary but deterministic axis
        let dir = if dist < 1e-5 { Vec3::X } else { delta / dist };
        let push = (MIN_DIST - dist) * 0.5;
        a.0 -= dir * push;
        b.0 += dir * push;
    }
}

/// Port of `AI_GetBestDribbleMovement` + `AI_GetForceFieldMovement`
/// (aifunctions.cpp): the carrier is repelled by the 5 nearest opponents and by
/// the lines, and attracted to the opponent goal.
pub fn dribble_direction(
    my_pos: Vec2,
    my_vel: Vec2,
    team: TeamId,
    sides: PitchSides,
    all_players: &[(TeamId, Vec2, Vec2)], // (team, position, velocity)
) -> Vec2 {
    let side = sides.defending_x(team);
    let future_sec = 0.25;
    let offense_factor = 0.75; // 0.7 + dribble_offensiveness/mindset defaults

    // 5 closest opponents
    let mut opponents: Vec<(f32, Vec2)> = all_players
        .iter()
        .filter(|(t, _, _)| *t != team)
        .map(|(_, p, v)| (p.distance(my_pos), *p + *v * future_sec))
        .collect();
    opponents.sort_by(|a, b| a.0.total_cmp(&b.0));
    opponents.truncate(5);

    let near_backline = normalized_clamp(my_pos.x.abs() / 55.0, 0.0, 1.0);
    // near the end of the pitch, we want to get inside again
    let center_modifier_inv = (1.0 - near_backline.powi(2)) * 0.5;
    let opp_goal_pos = Vec2::new(-side * 55.0, my_pos.y * 0.5 * center_modifier_inv);

    let mut force_field = Vec::with_capacity(8);
    for (_, opp_pos) in &opponents {
        force_field.push(ForceSpot {
            origin: *opp_pos,
            repel: true,
            power: 2.0,
            falloff: Falloff::Linear { radius: 10.0 },
        });
    }
    // sideline / backline. Stronger than the original's 4.0: without body
    // shielding the carrier gets herded outward by surrounding opponents, and
    // the lines must win that contest or the ball dribbles out of play.
    force_field.push(ForceSpot {
        origin: Vec2::new(my_pos.x, (36.0 + 5.0) * sign_side(my_pos.y) as f32),
        repel: true,
        power: 7.0,
        falloff: Falloff::Curved {
            radius: 20.0,
            exponent: 0.7,
        },
    });
    force_field.push(ForceSpot {
        origin: Vec2::new((55.0 + 5.0) * sign_side(my_pos.x) as f32, my_pos.y),
        repel: true,
        power: 7.0,
        falloff: Falloff::Curved {
            radius: 20.0,
            exponent: 0.7,
        },
    });
    // love for da goal
    force_field.push(ForceSpot {
        origin: opp_goal_pos,
        repel: false,
        power: offense_factor,
        falloff: Falloff::Constant,
    });

    let current_pos = my_pos + my_vel * future_sec;
    let attractor_damping_distance = 1.0;
    force_field::resolve(&force_field, current_pos, attractor_damping_distance)
        .map_or_else(|| Vec2::new(-side, 0.0), Vec2::normalize_or_zero)
}

/// Finds the earliest spot on the ball's predicted path the player can reach in
/// time, and the estimated arrival time in ms (simplified port of the original
/// `GetTimeNeededToGetToBall` AI support routine).
pub fn find_interception(
    player_pos_2d: Vec2,
    player_speed: f32,
    predictions: &[Vec3],
) -> (Vec2, f32) {
    if predictions.is_empty() {
        return (Vec2::ZERO, f32::MAX);
    }

    // Step time in predictions is 10ms (0.01 seconds)
    let step_time = 0.01;

    for (step_idx, &pred_3d) in predictions.iter().enumerate() {
        let pred_2d = Vec2::new(pred_3d.x, pred_3d.y);
        let dist = player_pos_2d.distance(pred_2d);

        // Time player needs to run to the spot at max speed (adding 0.05s reaction delay)
        let time_to_reach = dist / player_speed + 0.05;
        let ball_arrival_time = step_idx as f32 * step_time;

        if time_to_reach <= ball_arrival_time {
            return (pred_2d, ball_arrival_time * 1000.0);
        }
    }

    // Can't intercept within the 3s horizon: run to the resting point; the time
    // estimate is the full run there
    let last_pred = predictions[predictions.len() - 1];
    let last_2d = Vec2::new(last_pred.x, last_pred.y);
    let time_ms = (player_pos_2d.distance(last_2d) / player_speed + 0.05) * 1000.0;
    (last_2d, 3000.0f32.max(time_ms))
}

/// La misma pregunta sobre una trayectoria que un jugador cree haber visto. La
/// decisión necesita solo el plano del campo; altura y contacto siguen siendo
/// autoridad de la física cuando el cuerpo llega al último metro (§3).
pub fn find_believed_interception(
    player_pos_2d: Vec2,
    player_speed: f32,
    predictions: &[Vec2],
) -> (Vec2, f32) {
    if predictions.is_empty() {
        return (Vec2::ZERO, f32::MAX);
    }

    let step_time = 0.01;
    for (step_idx, &spot) in predictions.iter().enumerate() {
        let time_to_reach = player_pos_2d.distance(spot) / player_speed + 0.05;
        let ball_arrival_time = step_idx as f32 * step_time;
        if time_to_reach <= ball_arrival_time {
            return (spot, ball_arrival_time * 1000.0);
        }
    }

    let last = predictions[predictions.len() - 1];
    let time_ms = (player_pos_2d.distance(last) / player_speed + 0.05) * 1000.0;
    (last, 3000.0f32.max(time_ms))
}
