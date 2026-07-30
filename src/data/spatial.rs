use bevy::prelude::*;

/// Authoritative position on the pitch, in metres, in the simulation's Z-up
/// frame: `x` runs from goal to goal, `y` from touchline to touchline and `z` is
/// height above the grass.
///
/// This is the only spatial truth. No `Transform` is ever read as authoritative
/// state: presentation owns transforms and derives them from this component
/// (architecture law 2, "authoritative entity without visuals").
///
/// Conventions, because the two bodies anchor differently:
/// - a player's position is his support point on the ground, so `z == 0.0`;
///   the body is raised by presentation.
/// - the ball's position is its centre, so a ball at rest sits at
///   `z == BALL_RADIUS`.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Reflect)]
pub struct Position(pub Vec3);

impl Position {
    pub fn from_pitch(pitch_point: Vec2, height_metres: f32) -> Self {
        Self(Vec3::new(pitch_point.x, pitch_point.y, height_metres))
    }

    /// Projection onto the pitch plane. Most football reasoning (distances,
    /// interceptions, marking) happens here and ignores height.
    pub fn on_pitch(self) -> Vec2 {
        Vec2::new(self.0.x, self.0.y)
    }
}

/// Direction the body faces on the pitch plane.
///
/// Derived from movement for now. Turning is not yet a limited capability: a
/// rate-limited turn (and the difference between facing and running direction)
/// belongs to the motor model of MVP 3, so nothing may treat this as a
/// physically constrained orientation yet.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct Facing(pub Dir2);

impl Default for Facing {
    fn default() -> Self {
        Self(Dir2::X)
    }
}

/// Kinematic velocity in m/s. Players are not physics bodies: this is
/// integrated into [`Position`] once per fixed tick.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
pub struct Velocity(pub Vec3);
