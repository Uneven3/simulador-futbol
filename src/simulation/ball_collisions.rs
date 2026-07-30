use crate::data::{Ball, BallTouched, MatchState, Player, PossessionDesignation, Velocity};
use crate::math::{XorShift32, normalized_clamp, normalized_or};
use crate::simulation::SimulationSet;
use crate::simulation::ball_physics::touch_ball;
use bevy::prelude::*;

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

fn ball_body_collisions(
    mut match_state: ResMut<MatchState>,
    designation: Res<PossessionDesignation>,
    mut rng: ResMut<CollisionRng>,
    mut last_collision_ms: Local<u64>,
    time: Res<Time>,
    mut ball_query: Query<(&mut Ball, &mut Transform), Without<Player>>,
    mut player_query: Query<(Entity, &Transform, &mut Player, &Velocity), Without<Ball>>,
    mut touched_writer: MessageWriter<BallTouched>,
) {
    let now_ms = (time.elapsed_secs_f64() * 1000.0) as u64;
    if now_ms <= *last_collision_ms + 150 {
        return;
    }

    let Ok((mut ball, mut ball_transform)) = ball_query.single_mut() else {
        return;
    };
    let ball_pos = ball_transform.translation;

    let mut bounce_vec = Vec3::ZERO;
    let mut bias = 0.0f32;
    let mut bounce_count = 0;
    let mut toucher: Option<(Entity, u32)> = None;

    for (entity, transform, player, velocity) in player_query.iter() {
        // Original: the designated team possession player triggers a CONTROLLED
        // ball collision instead of an accidental deflection — his deliberate
        // touches happen in the kick system (pickup/tackle/knock-on). Without
        // this exclusion, two designated players over a contested ball deflect
        // it back and forth forever, wiping possession every 150 ms.
        if match_state.possession_player == Some(entity)
            || designation.designated[player.team_index as usize] == Some(entity)
        {
            continue;
        }

        let opp_team = 1 - player.team_index;
        let opp_last_touch_bias = if ball.last_touch_team == Some(opp_team) {
            last_touch_bias(now_ms, ball.last_touch_time_ms, TOUCH_TIME_THRESHOLD_MS)
        } else {
            0.0
        };
        let player_last_touch_bias =
            last_touch_bias(now_ms, player.last_touch_time_ms, TOUCH_TIME_THRESHOLD_MS);

        // cannot collide if opp didn't recently touch ball (we would be able to
        // predict ball by then), or if player itself already did (to overcome the
        // 'perpetuum collision' problem)
        if player_last_touch_bias > 0.01 || opp_last_touch_bias <= 0.01 {
            continue;
        }

        // capsule body: vertical segment between foot and head sphere centers
        let base = transform.translation;
        let seg_lo = Vec3::new(base.x, base.y, PLAYER_CAPSULE_RADIUS);
        let seg_hi = Vec3::new(base.x, base.y, player.height - PLAYER_CAPSULE_RADIUS);
        let closest_z = ball_pos.z.clamp(seg_lo.z, seg_hi.z);
        let closest = Vec3::new(base.x, base.y, closest_z);
        let dist = (ball_pos - closest).length();

        let hit_radius = 0.11 + PLAYER_CAPSULE_RADIUS + BOUNDING_BOX_SIZE_OFFSET;
        if dist < hit_radius {
            let movement_bias = opp_last_touch_bias * 0.8 + 0.2;
            let body_center = Vec3::new(base.x, base.y, player.height * 0.5);
            bounce_vec += normalized_or(ball_pos - body_center, Vec3::ZERO) * movement_bias
                + velocity.0 * (1.0 - movement_bias);
            bounce_count += 1;
            bias += (1.0 - ((dist - 0.11) / PLAYER_CAPSULE_RADIUS).clamp(0.0, 1.0)) * 0.9 + 0.1;
            toucher = Some((entity, player.team_index));
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

        touch_ball(&mut ball, &mut ball_transform, result_vector);
        let (rx, ry, rz) = (
            rng.0.range(-30.0, 30.0),
            rng.0.range(-30.0, 30.0),
            rng.0.range(-30.0, 30.0),
        );
        ball.set_rotation(rx, ry, rz, 0.5 * bias);
        let _touch_gain = normalized_clamp(result_vector.length(), 4.0, 40.0).powf(0.7); // audio hook (not yet ported)

        if let Some((entity, team)) = toucher {
            ball.last_touch_team = Some(team);
            ball.last_touch_player = Some(entity);
            ball.last_touch_time_ms = now_ms;
            if let Ok((_, _, mut player, _)) = player_query.get_mut(entity) {
                player.last_touch_time_ms = now_ms;
            }
            // an accidental body touch also interrupts any dribble possession
            if match_state.possession_player.is_some() {
                debug!("POSSESSION INTERRUPTED by body deflection off {:?}", entity);
                match_state.possession_player = None;
            }
            touched_writer.write(BallTouched {
                player: entity,
                team,
            });
        }

        *last_collision_ms = now_ms;
    }
}
