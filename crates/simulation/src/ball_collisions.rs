use crate::SimulationSet;
use crate::ball_physics::touch_ball;
use crate::diagnostics::{MatchFact, MatchTelemetry};
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use football_domain::math::{XorShift32, normalized_clamp, normalized_or};
use football_domain::tuning::ContestTuning;
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
        app.add_systems(
            FixedUpdate,
            ball_body_collisions.in_set(SimulationSet::BallCollisions),
        );
    }
}

#[derive(Resource)]
pub(crate) struct CollisionRng(XorShift32);

impl CollisionRng {
    /// A collision has its own random stream: adding a bounce cannot shift the
    /// random decisions made by the rest of the match.
    pub(crate) fn seeded(scenario_seed: u32) -> Self {
        Self(XorShift32(scenario_seed ^ 0x9E37_79B9))
    }
}

const PLAYER_CAPSULE_RADIUS: f32 = 0.35;
/// Original: `boundingBoxSizeOffset = -0.1f`, `+0.03f` when not in possession.
const BOUNDING_BOX_SIZE_OFFSET: f32 = -0.07;
/// How long a touch keeps its hold on the ball.
const TOUCH_WINDOW: Duration = Duration::from_millis(200);
/// Two bodies cannot deflect the same ball inside this window.
const COLLISION_COOLDOWN: Duration = Duration::from_millis(150);

/// Linear last-touch bias like `GetLastTouchBias`: 1.0 at the touch, fading to
/// 0.0 after `window`.
fn last_touch_bias(now: Duration, touched_at: Duration, window: Duration) -> f32 {
    if touched_at.is_zero() || now < touched_at {
        return 0.0;
    }
    let elapsed = now - touched_at;
    if elapsed >= window {
        0.0
    } else {
        1.0 - elapsed.as_secs_f32() / window.as_secs_f32()
    }
}

/// Si un cuerpo puede desviar el balón o está exento por disputarlo: los toques
/// de quien lo disputa son deliberados y los resuelve el sistema de golpeo. Un
/// balón que viaja ya no se disputa, y ahí sí lo desvía quien se cruce.
pub fn deflects_it(contesting_it: bool, ball_speed: f32, tuning: &ContestTuning) -> bool {
    !contesting_it || ball_speed > tuning.travelling_ball_speed
}

// A Bevy system states its dependencies as parameters (see `player_kick_system`).
#[allow(clippy::too_many_arguments)]
fn ball_body_collisions(
    mut match_state: ResMut<MatchState>,
    designation: Res<PossessionDesignation>,
    tuning: Res<football_domain::MatchTuning>,
    mut rng: ResMut<CollisionRng>,
    mut last_collision_at: Local<Duration>,
    time: Res<Time>,
    mut ball_query: Query<(&mut Ball, &mut Position), Without<Player>>,
    mut player_query: Query<crate::ball_contest::BallSystemBody, Without<Ball>>,
    mut touched_writer: MessageWriter<BallTouched>,
    mut telemetry: ResMut<MatchTelemetry>,
) {
    let now = time.elapsed();
    if now <= *last_collision_at + COLLISION_COOLDOWN {
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

    let ball_speed = ball.momentum.length();

    for (entity, position, player, attributes, player_state, velocity) in player_query.iter() {
        let contesting_it = match_state.possession_player == Some(player.id)
            || designation.designated[player.id.team] == Some(player.id);
        if !deflects_it(contesting_it, ball_speed, &tuning.contest) {
            continue;
        }
        // quien acaba de golpearlo no se lo bloquea a sí mismo
        if ball.last_touch_player == Some(player.id) {
            continue;
        }

        let opp_last_touch_bias = if ball.last_touch_team == Some(player.id.team.opponent()) {
            last_touch_bias(now, ball.last_touch_at, TOUCH_WINDOW)
        } else {
            0.0
        };
        let player_last_touch_bias = last_touch_bias(now, player_state.last_touch_at, TOUCH_WINDOW);

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
            ball.last_touch_at = now;
            if let Ok((.., mut player_state, _)) = player_query.get_mut(body) {
                player_state.last_touch_at = now;
            }
            // an accidental body touch also interrupts any dribble possession
            if let Some(interrupted) = match_state.possession_player {
                telemetry.record(MatchFact::PossessionLost {
                    player: interrupted,
                    at: ball_pos.truncate(),
                });
                match_state.lose_possession_to_a_loose_ball();
            }
            telemetry.record(MatchFact::Touched {
                player,
                deliberate: false,
            });
            touched_writer.write(BallTouched { player });
        }

        *last_collision_at = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Quien disputa el balón no lo desvía por accidente: sus toques son cosa
    /// del sistema de golpeo.
    #[test]
    fn a_body_contesting_the_ball_does_not_deflect_it() {
        let tuning = ContestTuning::default();
        let rolling = tuning.travelling_ball_speed * 0.5;

        assert!(!deflects_it(true, rolling, &tuning));
        assert!(
            deflects_it(false, rolling, &tuning),
            "un cuerpo cualquiera sí"
        );
    }

    /// Pero un tiro no se disputa: se bloquea, y lo bloquea el que esté
    /// delante aunque fuese el designado.
    #[test]
    fn a_travelling_ball_is_blocked_by_whoever_is_in_the_way() {
        let tuning = ContestTuning::default();
        let struck = tuning.travelling_ball_speed * 2.0;

        assert!(
            deflects_it(true, struck, &tuning),
            "el defensor que estaba delante dejó pasar el tiro"
        );
    }

    #[test]
    fn collision_randomness_is_a_repeatable_stream_of_the_scenario_seed() {
        let mut first = CollisionRng::seeded(42);
        let mut replay = CollisionRng::seeded(42);
        let mut another_situation = CollisionRng::seeded(43);

        let first_value = first.0.next_u32();
        assert_eq!(first_value, replay.0.next_u32());
        assert_ne!(first_value, another_situation.0.next_u32());

        let mut match_stream = XorShift32(42);
        assert_ne!(first_value, match_stream.next_u32());
    }

    #[test]
    fn the_kernel_installs_the_collision_stream_from_its_scenario_seed() {
        let mut first_scenario =
            football_domain::Scenario::kick_off().for_duration(Duration::from_secs(1));
        first_scenario.seed = 42;
        let replay_scenario = first_scenario.clone();
        let mut another_scenario = first_scenario.clone();
        another_scenario.seed = 43;

        let mut first = crate::scenario_runner::headless_scenario_app(first_scenario);
        let mut replay = crate::scenario_runner::headless_scenario_app(replay_scenario);
        let mut another = crate::scenario_runner::headless_scenario_app(another_scenario);

        let first_value = first
            .world_mut()
            .resource_mut::<CollisionRng>()
            .0
            .next_u32();
        assert_eq!(
            first_value,
            replay
                .world_mut()
                .resource_mut::<CollisionRng>()
                .0
                .next_u32()
        );
        assert_ne!(
            first_value,
            another
                .world_mut()
                .resource_mut::<CollisionRng>()
                .0
                .next_u32()
        );
    }
}
