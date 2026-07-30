use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::prelude::*;
use std::collections::VecDeque;

/// Radius of a match ball in metres (IFAB Law 2: circumference 68-70 cm).
pub const BALL_RADIUS: f32 = 0.11;

/// Original: `ballPredictionSize_ms = 3000` (gamedefines.hpp), one entry per 10 ms.
pub const BALL_PREDICTION_STEPS: usize = 300;
/// Original: `ballHistorySize_ms = 4000`, one entry per 10 ms tick.
pub const BALL_HISTORY_STEPS: usize = 400;

/// The ball is NOT simulated by a physics engine; like the original game, the
/// analytical integrator in `simulation::ball_physics` is the real physics: each
/// 10 ms tick the state at prediction step 1 becomes the actual ball state.
#[derive(Component, Debug, Clone, Reflect)]
pub struct Ball {
    // gameplay bookkeeping
    pub last_touch_team: Option<crate::identity::TeamId>,
    pub last_touch_player: Option<crate::identity::PlayerId>,
    pub last_touch_time_ms: u64,
    pub touches_net: bool,

    // physical state (port of Ball's momentum / rotation_ms / orientation buffers)
    /// Linear velocity in m/s.
    pub momentum: Vec3,
    /// Rotation quaternion applied per 1 ms (the engine's `rotation_ms` convention:
    /// X spin is built around the -X axis, see `Ball::set_rotation`).
    pub rotation_ms: Quat,
    /// Visual orientation of the ball.
    pub orientation: Quat,
    /// Position at the previous 10 ms tick (used for swept goal detection).
    pub previous_position: Vec3,

    /// 3 seconds of trajectory prediction, 10 ms apart. `predictions[0]` is the
    /// current position, `predictions[1]` the state the ball will adopt next tick.
    pub predictions: Vec<Vec3>,
    pub position_history: VecDeque<Vec3>,
}

impl Default for Ball {
    fn default() -> Self {
        Self {
            last_touch_team: None,
            last_touch_player: None,
            last_touch_time_ms: 0,
            touches_net: false,
            momentum: Vec3::ZERO,
            rotation_ms: Quat::IDENTITY,
            orientation: Quat::IDENTITY,
            previous_position: Vec3::new(0.0, 0.0, 0.11),
            predictions: vec![Vec3::new(0.0, 0.0, 0.11); BALL_PREDICTION_STEPS],
            position_history: VecDeque::new(),
        }
    }
}

impl Ball {
    /// A ball at `position` moving at `momentum`.
    ///
    /// The prediction IS the physics (see the type's note), so the buffer has to
    /// start from where the ball actually is: a ball built with `default()` and
    /// then moved would spend its first tick teleporting back to the centre spot.
    pub fn placed_at(position: Vec3, momentum: Vec3) -> Self {
        Self {
            momentum,
            previous_position: position,
            predictions: vec![position; BALL_PREDICTION_STEPS],
            ..Default::default()
        }
    }

    /// Port of `Ball::SetRotation(x, y, z, bias)` — radians per second per axis.
    /// Note the engine builds the X component around the -X axis; every consumer
    /// (Magnus effect, ground roll) assumes this convention.
    pub fn set_rotation(&mut self, x: f32, y: f32, z: f32, bias: f32) {
        let pi = std::f32::consts::PI;
        let rot_x = Quat::from_axis_angle(Vec3::NEG_X, (x * 0.001).clamp(-pi * 0.49, pi * 0.49));
        let rot_y = Quat::from_axis_angle(Vec3::Y, (y * 0.001).clamp(-pi * 0.49, pi * 0.49));
        let rot_z = Quat::from_axis_angle(Vec3::Z, (z * 0.001).clamp(-pi * 0.49, pi * 0.49));
        let tmp_rotation_ms = rot_x * rot_y * rot_z;
        self.rotation_ms = self.rotation_ms.slerp(tmp_rotation_ms, bias);
    }

    /// Port of `Ball::GetAveragePosition(duration_ms)`: average over the last
    /// `duration_ms` of position history (one sample per 10 ms tick).
    pub fn average_position(&self, duration_ms: f32) -> Vec3 {
        let mut total = 0u32;
        let mut sum = Vec3::ZERO;
        for p in self.position_history.iter().rev() {
            sum += *p;
            total += 1;
            if (total * 10) as f32 > duration_ms {
                break;
            }
        }
        if total > 0 {
            sum / total as f32
        } else {
            self.predictions[0]
        }
    }

    /// Port of `Ball::ResetSituation(focusPos)`.
    pub fn reset(&mut self, focus_pos: Vec3) {
        let rest = focus_pos + Vec3::new(0.0, 0.0, 0.11);
        self.momentum = Vec3::ZERO;
        self.rotation_ms = Quat::IDENTITY;
        self.orientation = Quat::IDENTITY;
        for p in self.predictions.iter_mut() {
            *p = rest;
        }
        self.position_history.clear();
        self.previous_position = rest;
        self.touches_net = false;
        self.last_touch_team = None;
        self.last_touch_player = None;
        self.last_touch_time_ms = 0;
    }
}
