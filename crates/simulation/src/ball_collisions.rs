use crate::SimulationSet;
use crate::ball_physics::touch_ball;
use crate::diagnostics::{MatchFact, MatchTelemetry};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use football_domain::math::{XorShift32, normalized_clamp, normalized_or};
use football_domain::{
    BALL_RADIUS, Ball, BallTouched, MatchState, Player, PlayerId, Position, PossessionDesignation,
};
use std::time::Duration;

/// Port of `Match::CheckBallCollisions()` (match.cpp), simplified: the original
/// tests the ball against every animated limb AABB and the player's current
/// animation; here the body is a fixed capsule. The deflection formula, touch
/// biases and cooldowns are kept as in the original.
pub struct BallCollisionPlugin;

impl Plugin for BallCollisionPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(CollisionRng(XorShift32(0x9E3779B9)))
            .add_systems(
                FixedUpdate,
                ball_body_collisions.in_set(SimulationSet::BallCollisions),
            );
    }
}

#[derive(Resource)]
struct CollisionRng(XorShift32);

const PLAYER_CAPSULE_RADIUS: f32 = 0.35;
/// Original: `boundingBoxSizeOffset = -0.1f`, `+0.03f` when not in possession.
const BOUNDING_BOX_SIZE_OFFSET: f32 = -0.07;
const TOUCH_TIME_THRESHOLD_MS: u64 = 200;

/// Linear last-touch bias like `GetLastTouchBias`: 1.0 at the touch, fading to
/// 0.0 after `threshold_ms`.
fn last_touch_bias(now_ms: u64, touch_time_ms: u64, threshold_ms: u64) -> f32 {
    if touch_time_ms == 0 || now_ms < touch_time_ms {
        return 0.0;
    }
    let elapsed = now_ms - touch_time_ms;
    if elapsed >= threshold_ms {
        0.0
    } else {
        1.0 - elapsed as f32 / threshold_ms as f32
    }
}

// A Bevy system states its dependencies as parameters (see `player_kick_system`).
#[allow(clippy::too_many_arguments)]
fn ball_body_collisions(
    mut match_state: ResMut<MatchState>,
    designation: Res<PossessionDesignation>,
    mut rng: ResMut<CollisionRng>,
    mut last_collision_ms: Local<u64>,
    time: Res<Time>,
    mut ball_query: Query<(&mut Ball, &mut Position), Without<Player>>,
    mut player_query: Query<crate::player_movement::BallSystemBody, Without<Ball>>,
    mut touched_writer: MessageWriter<BallTouched>,
    mut telemetry: ResMut<MatchTelemetry>,
) {
    let now_ms = (time.elapsed_secs_f64() * 1000.0) as u64;
    if now_ms <= *last_collision_ms + 150 {
        return;
    }

    let Ok((mut ball, mut ball_position)) = ball_query.single_mut() else {
        return;
    };
    let ball_pos = ball_position.0;

    let mut bounce_vec = Vec3::ZERO;
    let mut bias = 0.0f32;
    let mut bounce_count = 0;
    let mut toucher: Option<(Entity, PlayerId)> = None;

    for (entity, position, player, attributes, player_state, velocity) in player_query.iter() {
        // Original: the designated team possession player triggers a CONTROLLED
        // ball collision instead of an accidental deflection — his deliberate
        // touches happen in the kick system (pickup/tackle/knock-on). Without
        // this exclusion, two designated players over a contested ball deflect
        // it back and forth forever, wiping possession every 150 ms.
        if match_state.possession_player == Some(player.id)
            || designation.designated[player.id.team] == Some(player.id)
        {
            continue;
        }

        let opp_last_touch_bias = if ball.last_touch_team == Some(player.id.team.opponent()) {
            last_touch_bias(now_ms, ball.last_touch_time_ms, TOUCH_TIME_THRESHOLD_MS)
        } else {
            0.0
        };
        let player_last_touch_bias = last_touch_bias(
            now_ms,
            player_state.last_touch_at.as_millis() as u64,
            TOUCH_TIME_THRESHOLD_MS,
        );

        // cannot collide if opp didn't recently touch ball (we would be able to
        // predict ball by then), or if player itself already did (to overcome the
        // 'perpetuum collision' problem)
        if player_last_touch_bias > 0.01 || opp_last_touch_bias <= 0.01 {
            continue;
        }

        // capsule body standing on the pitch: vertical segment between the foot
        // and head sphere centres, anchored at the player's support point
        let base = position.0;
        let seg_lo = Vec3::new(base.x, base.y, PLAYER_CAPSULE_RADIUS);
        let seg_hi = Vec3::new(base.x, base.y, attributes.height - PLAYER_CAPSULE_RADIUS);
        let closest_z = ball_pos.z.clamp(seg_lo.z, seg_hi.z);
        let closest = Vec3::new(base.x, base.y, closest_z);
        let dist = (ball_pos - closest).length();

        let hit_radius = BALL_RADIUS + PLAYER_CAPSULE_RADIUS + BOUNDING_BOX_SIZE_OFFSET;
        if dist < hit_radius {
            let movement_bias = opp_last_touch_bias * 0.8 + 0.2;
            let body_center = Vec3::new(base.x, base.y, attributes.height * 0.5);
            bounce_vec += normalized_or(ball_pos - body_center, Vec3::ZERO) * movement_bias
                + velocity.0 * (1.0 - movement_bias);
            bounce_count += 1;
            bias +=
                (1.0 - ((dist - BALL_RADIUS) / PLAYER_CAPSULE_RADIUS).clamp(0.0, 1.0)) * 0.9 + 0.1;
            toucher = Some((entity, player.id));
        }
    }

    if bias > 0.0 {
        bounce_vec /= bounce_count as f32;
        bounce_vec.z *= 0.6;
        bounce_vec = normalized_or(bounce_vec, Vec3::ZERO);
        let current_movement = ball.momentum;
        let full_collision_vec = bounce_vec * 6.0
            + bounce_vec * current_movement.length() * 0.6
            + current_movement * -0.2;
        let bias = bias.clamp(0.0, 1.0) * 0.5 + 0.5;
        let mut result_vector = full_collision_vec * bias + current_movement * (1.0 - bias);
        if result_vector.length() > current_movement.length() {
            result_vector = normalized_or(result_vector, Vec3::ZERO) * current_movement.length();
        }
        result_vector *= 0.7;

        touch_ball(&mut ball, &mut ball_position, result_vector);
        let (rx, ry, rz) = (
            rng.0.range(-30.0, 30.0),
            rng.0.range(-30.0, 30.0),
            rng.0.range(-30.0, 30.0),
        );
        ball.set_rotation(rx, ry, rz, 0.5 * bias);
        let _touch_gain = normalized_clamp(result_vector.length(), 4.0, 40.0).powf(0.7); // audio hook (not yet ported)

        if let Some((body, player)) = toucher {
            ball.last_touch_team = Some(player.team);
            ball.last_touch_player = Some(player);
            ball.last_touch_time_ms = now_ms;
            if let Ok((.., mut player_state, _)) = player_query.get_mut(body) {
                player_state.last_touch_at = Duration::from_millis(now_ms);
            }
            // an accidental body touch also interrupts any dribble possession
            if let Some(interrupted) = match_state.possession_player {
                telemetry.record(MatchFact::PossessionLost {
                    player: interrupted,
                    at: ball_pos.truncate(),
                });
                match_state.possession_player = None;
            }
            telemetry.record(MatchFact::Touched {
                player,
                deliberate: false,
            });
            touched_writer.write(BallTouched { player });
        }

        *last_collision_ms = now_ms;
    }
}
