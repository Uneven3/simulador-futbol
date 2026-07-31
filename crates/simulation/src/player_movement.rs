//! Los cuerpos: quién va a por el balón, cómo se mueven y cómo no se
//! superponen.
//!
//! Lo que pasa cuando uno de ellos alcanza el balón vive en
//! [`crate::ball_contest`] y [`crate::ball_release`].

use crate::SimulationSet;
use crate::player_decisions;
use crate::team_tactics::{self, TeamTactics};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;

use football_domain::math::{normalized_clamp, sign_side};
use football_domain::{
    Attributes, Ball, ByTeam, Facing, MatchState, Player, PlayerId, PlayerMatchState,
    PlayerRegistry, PlayingPosition, Position, PossessionDesignation, TeamId, Velocity,
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
                    player_decisions::select_player_movement,
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

    // a pass is only "in flight" while the ball travels; once it truly dies the
    // normal designation race resumes (otherwise a lost pass freezes the team).
    // The threshold must be near-rest: the solved passes ARRIVE dying at the
    // receiver, and expiring at 1.0 m/s stripped the receiver of his priority
    // and trap reach exactly at the moment of reception.
    if ball.momentum.length() < 0.3 {
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
/// as in the original engine). Also derives the body's facing from the movement
/// and maintains the ~10 s average velocity used by `GetLazyVelocity` (original
/// `Player::GetAverageVelocity(10)`).
fn apply_player_velocity(
    time: Res<Time>,
    mut query: Query<(&mut Position, &mut Facing, &Velocity, &mut PlayerMatchState)>,
) {
    let dt = time.delta_secs();
    for (mut position, mut facing, velocity, mut player_state) in query.iter_mut() {
        position.0.x += velocity.0.x * dt;
        position.0.y += velocity.0.y * dt;
        // Facing follows the movement instantly: turning is not yet a limited
        // capability (MVP 3). A standing player keeps his previous facing.
        if let Ok(direction) = Dir2::new(Vec2::new(velocity.0.x, velocity.0.y)) {
            facing.0 = direction;
        }
        let speed = velocity.0.length();
        player_state.recent_speed += (speed - player_state.recent_speed) * (dt / 10.0).min(1.0);
    }
}

/// Positional separation between player bodies: two bodies of radius 0.35 m
/// cannot occupy the same spot, so overlapping pairs get pushed apart. This is
/// the cheap stand-in for body contact until the motor model of MVP 3; without
/// it, both designated players superimpose on the contested ball.
fn resolve_player_overlap(mut query: Query<&mut Position, With<Player>>) {
    const MIN_DIST: f32 = 0.7; // two body radii

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
/// (aifunctions.cpp): the carrier is repelled by the 5 nearest opponents
/// (projected 0.25 s ahead) and by the side/back lines, and attracted to the
/// opponent goal (with a center magnet that grows near the backline). Returns
/// the desired dribble direction.
pub fn dribble_direction(
    my_pos: Vec2,
    my_vel: Vec2,
    team: TeamId,
    all_players: &[(TeamId, Vec2, Vec2)], // (team, position, velocity)
) -> Vec2 {
    let side = crate::team_tactics::team_side(team);
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

    struct ForceSpot {
        origin: Vec2,
        repel: bool,
        constant: bool,
        power: f32,
        scale: f32,
        exp: f32,
    }
    let mut force_field = Vec::with_capacity(8);
    for (_, opp_pos) in &opponents {
        force_field.push(ForceSpot {
            origin: *opp_pos,
            repel: true,
            constant: false,
            power: 2.0,
            scale: 10.0,
            exp: 1.0,
        });
    }
    // sideline / backline. Stronger than the original's 4.0: without body
    // shielding the carrier gets herded outward by surrounding opponents, and
    // the lines must win that contest or the ball dribbles out of play.
    force_field.push(ForceSpot {
        origin: Vec2::new(my_pos.x, (36.0 + 5.0) * sign_side(my_pos.y) as f32),
        repel: true,
        constant: false,
        power: 7.0,
        scale: 20.0,
        exp: 0.7,
    });
    force_field.push(ForceSpot {
        origin: Vec2::new((55.0 + 5.0) * sign_side(my_pos.x) as f32, my_pos.y),
        repel: true,
        constant: false,
        power: 7.0,
        scale: 20.0,
        exp: 0.7,
    });
    // love for da goal
    force_field.push(ForceSpot {
        origin: opp_goal_pos,
        repel: false,
        constant: true,
        power: offense_factor,
        scale: 1.0,
        exp: 1.0,
    });

    let current_pos = my_pos + my_vel * future_sec;
    let attractor_damping_distance = 1.0;
    let mut cumul_vec = Vec2::ZERO;
    let mut cumul_force = 0.0;
    for spot in &force_field {
        let distance = spot.origin.distance(current_pos);
        let intensity = if spot.constant {
            1.0
        } else {
            let i = (1.0 - distance / spot.scale).clamp(0.0, 1.0);
            if spot.exp != 1.0 { i.powf(spot.exp) } else { i }
        };
        if intensity > 0.0 {
            let mut relative_origin = (spot.origin - current_pos).normalize_or_zero();
            if spot.repel {
                relative_origin = -relative_origin;
            } else if distance < attractor_damping_distance {
                relative_origin *= distance / attractor_damping_distance;
            }
            let force = spot.power * intensity;
            cumul_vec += relative_origin * force;
            cumul_force += force;
        }
    }
    if cumul_force == 0.0 {
        Vec2::new(-side, 0.0)
    } else {
        (cumul_vec / cumul_force).normalize_or_zero()
    }
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
