use crate::SimulationSet;
use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use football_domain::math::{QuatExt, normalized_clamp, normalized_or, sign_side};
use football_domain::{
    BALL_HISTORY_STEPS, BALL_PREDICTION_STEPS, BALL_RADIUS, Ball, MatchState, PitchConfig, Position,
};

// Ball physics constants (Ball::Ball(), ball.cpp)
const BOUNCE: f32 = 0.62; // 1 = full bounce, 0 = no bounce
const LINEAR_BOUNCE: f32 = 0.06; // bigger = more brake force
const DRAG: f32 = 0.015; // bigger = more
const FRICTION: f32 = 0.04; // bigger = more
const LINEAR_FRICTION: f32 = 1.6; // bigger = more, arbitrary scale
const GRAVITY: f32 = -9.81;
const GRASS_HEIGHT: f32 = 0.025;

pub struct BallPhysicsPlugin;

impl Plugin for BallPhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, ball_process.in_set(SimulationSet::BallPhysics));
    }
}

/// Result of one full prediction pass: the state the ball adopts at t = 10 ms.
/// Port of `BallSpatialInfo` plus the orientation buffer and net-touch flag.
pub struct BallSpatialInfo {
    pub momentum: Vec3,
    pub rotation_ms: Quat,
    pub orient_prediction: Quat,
    pub touches_net: bool,
}

/// Port of `Ball::Process()`: like the original, the analytical prediction IS the
/// real ball physics — the state at prediction step 1 becomes the actual state.
fn ball_process(
    mut query: Query<(&mut Ball, &mut Position)>,
    pitch_config: Res<PitchConfig>,
    match_state: Res<MatchState>,
) {
    for (mut ball, mut position) in query.iter_mut() {
        let ball = &mut *ball;
        let info = calculate_prediction(
            position.0,
            ball.orientation,
            ball.momentum,
            ball.rotation_ms,
            match_state.is_ball_in_goal,
            &pitch_config,
            &mut ball.predictions,
        );

        ball.previous_position = position.0;
        ball.momentum = info.momentum;
        ball.rotation_ms = info.rotation_ms;
        ball.orientation = info.orient_prediction;
        ball.touches_net = info.touches_net;

        // positionBuffer = Predict(10)
        position.0 = ball.predictions[1];

        ball.position_history.push_back(position.0);
        if ball.position_history.len() > BALL_HISTORY_STEPS {
            ball.position_history.pop_front();
        }
    }
}

/// Port of `Ball::Touch(target)`: a deliberate touch replaces the ball's momentum.
/// The prediction is refreshed on the next 10 ms tick (`ball_process` runs every tick).
pub fn touch_ball(ball: &mut Ball, position: &mut Position, target_momentum: Vec3) {
    if position.0.z < BALL_RADIUS {
        position.0.z = BALL_RADIUS;
    }
    ball.momentum = target_momentum;
}

/// Port of `Ball::CalculatePrediction()`: integrates 3 seconds of trajectory in
/// 10 ms steps and returns the state at the first step, which is the real ball
/// state for the next tick. Woodwork is only resolved on the first step
/// (`firstTime` in the original): post bounces are deliberately unpredictable
/// for the AI. Netting also only acts on the first step.
/// Solve the kick momentum that carries the ball from `from` to the 2D point
/// `to`, arriving with `pace_bonus` m/s of extra pace. The original resolves
/// pass power in `AI_GetAutoPass` + the kick animations; here we bisect the
/// initial speed against the real ball integrator, so the pass physically
/// arrives at the receiver instead of dying short or overshooting.
pub fn solve_pass_momentum(
    pitch: &PitchConfig,
    from: Vec3,
    to: Vec2,
    lift: f32,
    pace_bonus: f32,
) -> Vec3 {
    let dir2 = (to - Vec2::new(from.x, from.y)).normalize_or_zero();
    let dir = Vec3::new(dir2.x, dir2.y, lift).normalize_or_zero();
    let mut preds = vec![Vec3::ZERO; BALL_PREDICTION_STEPS];
    let mut lo = 6.0f32;
    let mut hi = 30.0f32;
    for _ in 0..7 {
        let mid = 0.5 * (lo + hi);
        let _ = calculate_prediction(
            from,
            Quat::IDENTITY,
            dir * mid,
            Quat::IDENTITY,
            false,
            pitch,
            &mut preds,
        );
        let reaches = preds.iter().any(|p| Vec2::new(p.x, p.y).distance(to) < 0.7);
        if reaches {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    dir * (hi + pace_bonus)
}

pub fn calculate_prediction(
    start_pos: Vec3,
    start_orientation: Quat,
    momentum: Vec3,
    rotation_ms: Quat,
    ball_is_in_goal: bool,
    pitch: &PitchConfig,
    predictions: &mut [Vec3],
) -> BallSpatialInfo {
    let pi = std::f32::consts::PI;

    let mut new_momentum = momentum;
    let mut new_rotation_ms = rotation_ms;
    let mut orient_prediction = start_orientation;

    let mut next_pos = start_pos;
    let mut next_orientation = start_orientation;
    let mut momentum_predict = momentum;
    let mut rotation_predict_ms = rotation_ms;

    predictions[0] = next_pos;

    let time_step = 0.01; // seconds
    let mut first_time = true;
    let mut ball_touches_net = false;

    let mut predict_time_ms: usize = 10;
    while predict_time_ms < BALL_PREDICTION_STEPS * 10 {
        let mut friction_factor = 0.0;

        // gravity
        momentum_predict.z += GRAVITY * time_step;

        // air resistance
        let momentum_velo = momentum_predict.length();
        let momentum_velo_dragged =
            momentum_velo - DRAG * momentum_velo * momentum_velo * time_step;
        momentum_predict = normalized_or(momentum_predict, Vec3::ZERO) * momentum_velo_dragged;

        let ball_bottom = next_pos.z - 0.11;
        let mut grass_influence_bias = (1.0 - (ball_bottom / GRASS_HEIGHT)).clamp(0.0, 1.0); // 0 == no friction, 1 == all friction
        // at half grass height, there's already a bigger amount of friction than 50%
        grass_influence_bias = grass_influence_bias.powf(0.7);

        // bounce
        if next_pos.z < 0.11 {
            if momentum_predict.z < 0.0 {
                // when the ball is slammed into the ground, there's gonna be more friction.
                // only set it here so it is only done once (on impact)
                friction_factor = normalized_clamp(-momentum_predict.z - 0.5, 0.0, 12.0);
                momentum_predict.z = -momentum_predict.z * BOUNCE;
                momentum_predict.z = (momentum_predict.z - LINEAR_BOUNCE).max(0.0); // linear bounce
            }
            next_pos.z = 0.11;
        }

        // ground friction
        if next_pos.z < 0.11 + GRASS_HEIGHT {
            let adapted_friction = FRICTION * grass_influence_bias;

            let mut xy = Vec3::new(momentum_predict.x, momentum_predict.y, 0.0);
            let velo = xy.length();
            let mut new_velo = velo - adapted_friction * velo * velo * time_step;
            // linear friction
            new_velo = (new_velo - LINEAR_FRICTION * grass_influence_bias * time_step)
                .clamp(0.0, 100000.0);

            xy = normalized_or(xy, Vec3::ZERO) * new_velo;
            momentum_predict.x = xy.x;
            momentum_predict.y = xy.y;
        }

        let net_absorb_inv = 0.95f32.powf(time_step * 100.0);
        let pow_factor = 2.6;
        let power_fac = 1.8; // lol varnames
        let post_absorb_inv = 0.8;
        let ball_radius = 0.11;
        let post_radius = pitch.post_radius;
        let pitch_half_w = pitch.half_width;
        let goal_half_width = pitch.goal_half_width;
        let goal_height = pitch.goal_height;
        let goal_depth = pitch.goal_depth;

        // woodwork
        if first_time {
            // posts
            let pos_2d = Vec3::new(next_pos.x, next_pos.y, 0.0);
            let pos_2d_abs = Vec3::new(next_pos.x.abs(), next_pos.y.abs(), 0.0);
            if next_pos.z < goal_height + ball_radius + post_radius
                && (pos_2d_abs - Vec3::new(pitch_half_w, goal_half_width, 0.0)).length()
                    < ball_radius + post_radius
            {
                let post_pos = Vec3::new(
                    pitch_half_w * sign_side(next_pos.x) as f32,
                    goal_half_width * sign_side(next_pos.y) as f32,
                    0.0,
                );
                let normal_fallback = Vec3::new(-sign_side(next_pos.x) as f32, 0.0, 0.0);
                let normal = normalized_or(pos_2d - post_pos, normal_fallback);
                let next_pos_z = next_pos.z;
                next_pos = post_pos + normal * (post_radius + ball_radius);
                next_pos.z = next_pos_z;

                let momentum_2d = Vec3::new(momentum_predict.x, momentum_predict.y, 0.0);
                momentum_predict = normalized_or(
                    normalized_or(momentum_2d, normal) + normal * 1.1,
                    Vec3::ZERO,
                ) * momentum_2d.length()
                    * post_absorb_inv
                    + Vec3::new(0.0, 0.0, momentum_predict.z);
            }

            // crossbar
            let next_pos_xz = Vec3::new(next_pos.x, 0.0, next_pos.z);
            let next_pos_xz_abs = Vec3::new(next_pos.x.abs(), 0.0, next_pos.z.abs());
            if (next_pos_xz_abs - Vec3::new(pitch_half_w, 0.0, goal_height)).length()
                < ball_radius + post_radius
                && next_pos.y.abs() < goal_half_width + ball_radius + post_radius
            {
                let bar_pos = Vec3::new(
                    pitch_half_w * sign_side(next_pos.x) as f32,
                    0.0,
                    goal_height,
                );
                let normal_fallback =
                    Vec3::new(0.0, 0.0, if next_pos.x < 0.0 { 1.0 } else { -1.0 });
                let normal = normalized_or(next_pos_xz - bar_pos, normal_fallback);
                let next_pos_y = next_pos.y;
                next_pos = bar_pos + normal * (post_radius + ball_radius);
                next_pos.y = next_pos_y;

                let momentum_xz = Vec3::new(momentum_predict.x, 0.0, momentum_predict.z);
                momentum_predict = normalized_or(
                    normalized_or(momentum_xz, normal) + normal * 1.1,
                    Vec3::ZERO,
                ) * momentum_xz.length()
                    * post_absorb_inv
                    + Vec3::new(0.0, momentum_predict.y, 0.0);
            }
        }

        // netting
        if predict_time_ms <= 10 {
            let in_goal: f32 = if ball_is_in_goal { 1.0 } else { -1.0 };

            let behind_backline = next_pos.x.abs() > pitch_half_w + 0.11;
            let before_goal_back = next_pos.x.abs() < pitch_half_w + goal_depth - 0.11;
            let below_goal_height = next_pos.z < goal_height + 0.11;
            let between_goal_width = next_pos.y.abs() < goal_half_width - 0.11;

            // side netting
            if ball_is_in_goal && !between_goal_width && behind_backline {
                let net_dist = (next_pos.y.abs() - goal_half_width).abs().clamp(0.0, 1.0);
                let power = net_dist.powf(pow_factor) * -sign_side(next_pos.y) as f32 * in_goal;

                // net is stuck to woodwork so lay off there
                let woodwork_tension_bias_inv =
                    ((momentum_predict.x.abs() - pitch_half_w) * 2.0).clamp(0.0, 1.0);
                let adapted_power_fac = power_fac + (1.0 - woodwork_tension_bias_inv) * 3.0;

                momentum_predict.y = momentum_predict.y * net_absorb_inv
                    + power * adapted_power_fac * (100.0 * time_step);

                if predict_time_ms == 10 {
                    ball_touches_net = true;
                }
            }

            // rear netting
            if ball_is_in_goal && !before_goal_back && behind_backline {
                let net_dist = (next_pos.x.abs() - (pitch_half_w + goal_depth))
                    .abs()
                    .clamp(0.0, 1.0);
                let power = net_dist.powf(pow_factor) * -sign_side(next_pos.x) as f32 * in_goal;
                momentum_predict.x =
                    momentum_predict.x * net_absorb_inv + power * power_fac * (100.0 * time_step);

                if predict_time_ms == 10 {
                    ball_touches_net = true;
                }
            }

            // top netting
            if ball_is_in_goal && !below_goal_height && behind_backline {
                let net_dist = (next_pos.z.abs() - goal_height).abs().clamp(0.0, 1.0);
                let power = net_dist.powf(pow_factor) * -in_goal;

                // net is stuck to woodwork so lay off there
                let woodwork_tension_bias_inv =
                    ((momentum_predict.x.abs() - pitch_half_w) * 2.0).clamp(0.0, 1.0);
                let adapted_power_fac = power_fac + (1.0 - woodwork_tension_bias_inv) * 3.0;

                momentum_predict.z = momentum_predict.z * net_absorb_inv
                    + power * adapted_power_fac * (100.0 * time_step);

                if predict_time_ms == 10 {
                    ball_touches_net = true;
                }
            }
        } // </goal collisions>

        // calculate rotation
        if next_pos.z < 0.11 + GRASS_HEIGHT {
            // ground friction induced rotation
            let radius = 0.11;
            // x movement causes roll over y axis.. so this is correct ;)
            let x_r = momentum_predict.y / radius;
            let y_r = momentum_predict.x / radius;

            // clamp, because we can not rotate faster than this or the maths don't
            // know what direction to rotate into anymore
            let rot_x =
                Quat::from_axis_angle(Vec3::NEG_X, (x_r * 0.001).clamp(-pi * 0.49, pi * 0.49));
            let rot_y = Quat::from_axis_angle(Vec3::Y, (y_r * 0.001).clamp(-pi * 0.49, pi * 0.49));
            let ground_rot = rot_x * rot_y;

            let mut old_to_new_rotation =
                rotation_predict_ms.get_rotation_to(ground_rot).normalize();
            let rotation_change_per_second =
                old_to_new_rotation.get_rotation_angle(Quat::IDENTITY).abs() * 1000.0;

            let mut max_rotation_change_per_second = pi * grass_influence_bias;
            // ball slams into ground; this happens only once per bounce
            if friction_factor > 0.0 {
                max_rotation_change_per_second += 4.0 * pi;
            }
            let mut factor = 1.0;
            if rotation_change_per_second > max_rotation_change_per_second {
                factor = max_rotation_change_per_second / rotation_change_per_second;
            }
            if factor < 1.0 {
                old_to_new_rotation = old_to_new_rotation.get_rotation_multiplied_by(factor);
            }

            let new_rotation_predict_ms = old_to_new_rotation * rotation_predict_ms;

            // rotation induced ground friction
            let (mut x_ang, y_ang, _z_ang) = rotation_predict_ms.get_angles();
            x_ang = -x_ang;

            // how fast the ball would move if we took 100% of its rotational velo
            let mut ball_rotation_momentum = Vec3::ZERO;
            ball_rotation_momentum.x = y_ang * radius * 1000.0;
            ball_rotation_momentum.y = x_ang * radius * 1000.0;

            // lower == ball is lighter. higher == pitch/ball contact seems more 'rubbery'
            let mut rot_bias = 0.01 * grass_influence_bias;
            if friction_factor > 0.0 {
                rot_bias += 0.5 * friction_factor;
            }
            rot_bias = rot_bias.clamp(0.0, 1.0);
            momentum_predict.x =
                momentum_predict.x * (1.0 - rot_bias) + ball_rotation_momentum.x * rot_bias;
            momentum_predict.y =
                momentum_predict.y * (1.0 - rot_bias) + ball_rotation_momentum.y * rot_bias;

            // finally, add the previously calculated ground friction induced rotation
            rotation_predict_ms = new_rotation_predict_ms;
        }

        // magnus effect (swerve)
        {
            let (rx, ry, rz) = rotation_predict_ms.get_angles();
            let rot_vec = Vec3::new(rx, ry, rz) * 10.0;

            // magnus effect has a strength curve that goes down after a certain velocity
            let swerve_amount = normalized_clamp(momentum_predict.length(), 0.0, 70.0);
            let swerve_amount = (swerve_amount * pi * 0.94).sin().powf(2.6);
            let adapted_momentum_predict =
                normalized_or(momentum_predict, Vec3::ZERO) * swerve_amount * 30.0;

            let swerve = adapted_momentum_predict.cross(-rot_vec) * 1.0;

            momentum_predict += swerve * time_step;
        }

        // predict next step
        next_pos += momentum_predict * time_step;

        let (rx, ry, rz) = rotation_predict_ms.get_angles();
        let rotation_vector = Vec3::new(rx, ry, rz) * (time_step / 0.001);
        let rotation_predict_time_stepped =
            Quat::from_angles(rotation_vector.x, rotation_vector.y, rotation_vector.z);
        next_orientation = rotation_predict_time_stepped * next_orientation;

        predictions[predict_time_ms / 10] = next_pos;

        if predict_time_ms == 10 {
            new_momentum = momentum_predict;
            new_rotation_ms = rotation_predict_ms;
            orient_prediction = next_orientation;
        }

        first_time = false;
        predict_time_ms += 10;
    }

    BallSpatialInfo {
        momentum: new_momentum,
        rotation_ms: new_rotation_ms,
        orient_prediction,
        touches_net: ball_touches_net,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn predict(start_pos: Vec3, start_vel: Vec3, spin_rad_s: Vec3) -> Vec<Vec3> {
        let pitch_config = PitchConfig::default();
        let mut predictions = vec![Vec3::ZERO; BALL_PREDICTION_STEPS];
        let mut ball = Ball::default();
        ball.set_rotation(spin_rad_s.x, spin_rad_s.y, spin_rad_s.z, 1.0);
        calculate_prediction(
            start_pos,
            Quat::IDENTITY,
            start_vel,
            ball.rotation_ms,
            false,
            &pitch_config,
            &mut predictions,
        );
        predictions
    }

    #[test]
    fn test_gravity_and_bounce() {
        let predictions = predict(Vec3::new(0.0, 0.0, 2.0), Vec3::ZERO, Vec3::ZERO);

        let mut dropped = false;
        let mut bounced = false;
        let mut previous_z = 2.0;

        for pos in predictions.iter() {
            if pos.z < previous_z {
                dropped = true;
            } else if dropped && pos.z > previous_z && pos.z > 0.1101 {
                bounced = true;
                break;
            }
            previous_z = pos.z;
        }

        assert!(dropped, "Ball should drop under gravity");
        assert!(bounced, "Ball should bounce off the ground");
    }

    #[test]
    fn test_magnus_effect() {
        let start_pos = Vec3::new(0.0, 0.0, 1.0);
        let start_vel = Vec3::new(20.0, 0.0, 5.0);

        let predictions = predict(start_pos, start_vel, Vec3::new(0.0, -100.0, 0.0));
        let predictions_no_spin = predict(start_pos, start_vel, Vec3::ZERO);

        let mut higher_count = 0;
        for i in 1..50 {
            if predictions[i].z > predictions_no_spin[i].z {
                higher_count += 1;
            }
        }

        assert!(
            higher_count > 40,
            "Spinning ball should experience lift from Magnus effect"
        );
    }

    /// The real ball state (step 1 of the prediction) must coincide with the
    /// prediction the AI reads — this is the invariant the original game relies on.
    #[test]
    fn test_process_matches_prediction() {
        let pitch_config = PitchConfig::default();
        let mut predictions = vec![Vec3::ZERO; BALL_PREDICTION_STEPS];
        let start_pos = Vec3::new(0.0, 0.0, 0.11);
        let momentum = Vec3::new(15.0, 3.0, 4.0);

        let full = calculate_prediction(
            start_pos,
            Quat::IDENTITY,
            momentum,
            Quat::IDENTITY,
            false,
            &pitch_config,
            &mut predictions,
        );
        let expected_path: Vec<Vec3> = predictions.clone();

        // step the "real" ball twice, 10 ms each, and compare against the
        // 3-second prediction computed up front
        let mut pos = predictions[1];
        let mut mom = full.momentum;
        let mut rot = full.rotation_ms;
        let mut orient = full.orient_prediction;
        for (step, &expected) in expected_path.iter().enumerate().take(4).skip(2) {
            let info = calculate_prediction(
                pos,
                orient,
                mom,
                rot,
                false,
                &pitch_config,
                &mut predictions,
            );
            pos = predictions[1];
            mom = info.momentum;
            rot = info.rotation_ms;
            orient = info.orient_prediction;
            assert!(
                (pos - expected).length() < 0.01,
                "Real ball diverged from prediction at step {step}: {pos:?} vs {expected:?}"
            );
        }
    }
}
