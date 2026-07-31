use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::prelude::*;

/// The only spatial truth, in metres, Z-up: `x` goal to goal, `y` touchline to
/// touchline, `z` above the grass. A `Transform` is never authoritative — those
/// belong to presentation, derived from this (§1).
///
/// The two bodies anchor differently: a player's position is his support point
/// (`z == 0.0`, presentation raises the body), the ball's is its centre, so a
/// ball at rest sits at `z == BALL_RADIUS`.
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
/// Derivada del movimiento, y por ahí heredada del límite del motor: la
/// velocidad ya no salta, así que esto tampoco. Lo que sigue sin existir es la
/// diferencia entre hacia dónde se mira y hacia dónde se corre, así que nadie
/// puede tratarla todavía como una orientación con vida propia.
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

/// La velocidad que la decisión pide, que no es la que el cuerpo consigue.
///
/// Son dos componentes y no uno porque son dos escalones distintos: decidir e
/// ir (§3). Quien decide escribe aquí y nunca en [`Velocity`]; entre los dos
/// está el motor, que es lo único que convierte lo pedido en lo alcanzable.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
pub struct MovementIntent(pub Vec3);
