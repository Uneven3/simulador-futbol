//! Los cuerpos: quién va a por el balón, cómo se mueven y cómo no se
//! superponen.
//!
//! Lo que pasa cuando uno de ellos alcanza el balón vive en
//! [`crate::ball_contest`] y [`crate::ball_release`].

use crate::SimulationSet;
use crate::force_field::{self, Falloff, ForceSpot};
use crate::player_decisions;
use crate::team_tactics::{self, TeamTactics};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;

use football_domain::math::{normalized_clamp, sign_side};
use football_domain::{
    Attributes, Ball, ByTeam, MatchState, PLAYER_BODY_RADIUS, PitchSides, Player, PlayerId,
    PlayerMatchState, PlayerRegistry, PlayingPosition, Position, PossessionDesignation, TeamId,
    Velocity,
};

/// Speed below which a ball counts as dead, in m/s.
const BALL_AT_REST_SPEED: f32 = 0.3;

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
                    player_decisions::select_player_movement,
                    crate::locomotion::drive_bodies,
                    apply_player_velocity,
                    resolve_player_overlap,
                )
                    .chain()
                    .in_set(SimulationSet::Players),
            );
    }
}

/// Port of the original team bookkeeping (`Team::GetTimeNeededToGetToBall_ms` +
/// `GetDesignatedTeamPossessionPlayer`): per team, find the single outfield
/// player who can reach the ball's predicted path first. Only he chases; a
/// player in possession is always his team's designated player.
fn update_possession_designation(
    mut match_state: ResMut<MatchState>,
    records: Res<football_domain::OffsideRecords>,
    mut designation: ResMut<PossessionDesignation>,
    registry: Res<PlayerRegistry>,
    ball_query: Query<&Ball>,
    player_query: Query<(&Position, &Player, &Attributes)>,
) {
    let Ok(ball) = ball_query.single() else {
        return;
    };

    let mut best: ByTeam<Option<(PlayerId, f32)>> = ByTeam::default();
    for (position, player, stats) in player_query.iter() {
        if player.position == PlayingPosition::Goalkeeper {
            continue;
        }
        // a player caught in an offside position when the ball was last played
        // must not go for it (or the whistle blows the moment he touches it)
        if records.team == Some(player.id.team)
            && records.players.iter().any(|(id, _)| *id == player.id)
        {
            continue;
        }
        let (_, time_ms) =
            find_interception(position.on_pitch(), stats.top_speed, &ball.predictions);
        let slot = &mut best[player.id.team];
        if slot.is_none_or(|(_, t)| time_ms < t) {
            *slot = Some((player.id, time_ms));
        }
    }

    // whoever holds the ball is his team's designated player by definition
    if let Some(possessor) = match_state.possession_player {
        best[possessor.team] = Some((possessor, 0.0));
    }

    // A pass in flight suspends the designation race, so the threshold has to
    // be near-rest: solved passes arrive dying, and expiring earlier strips the
    // receiver of his priority at the very moment of reception.
    if ball.momentum.length() < BALL_AT_REST_SPEED {
        match_state.pass_target = None;
    }

    // the intended receiver of a pass in flight attacks it, even if a teammate
    // is nominally a bit faster to the ball (the original's receivers run onto
    // `AI_GetPass` balls; without this the passer often re-chases his own pass)
    if let Some(receiver) = match_state.pass_target
        && let Some(body) = registry.body(receiver)
        && let Ok((position, player, stats)) = player_query.get(body)
    {
        let (_, time_ms) =
            find_interception(position.on_pitch(), stats.top_speed, &ball.predictions);
        let slot = &mut best[player.id.team];
        if time_ms < 3500.0 && slot.is_none_or(|(_, t)| time_ms < t * 1.5 + 300.0) {
            *slot = Some((receiver, time_ms));
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
