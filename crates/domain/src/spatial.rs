use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::prelude::*;

/// The only spatial truth, in metres, Z-up: `x` goal to goal, `y` touchline to
/// touchline, `z` above the grass; a `Transform` is never authoritative (§1).
/// A player anchors at his support point and the ball at its centre.
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

/// Hacia dónde mira el cuerpo. La escribe el motor con su propio límite de
/// giro. `Gaze` pide una dirección distinta de la carrera; mientras no haya una
/// cabeza separada, apartar la vista también gira el cuerpo.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct Facing(pub Dir2);

impl Default for Facing {
    fn default() -> Self {
        Self(Dir2::X)
    }
}

/// El punto que un cuerpo quiere mirar, si quiere mirar alguno.
///
/// Lo escribe quien decide y lo obedece el motor dentro de su límite de giro.
/// `None` es mirar hacia donde se va, que es lo que hace quien esprinta.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
pub struct Gaze(pub Option<Vec2>);

/// Kinematic velocity in m/s. Players are not physics bodies: this is
/// integrated into [`Position`] once per fixed tick.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
pub struct Velocity(pub Vec3);

/// La velocidad que la decisión pide, que no es la que el cuerpo consigue: dos
/// componentes porque son dos escalones (§3). Quien decide escribe aquí y nunca
/// en [`Velocity`]; en medio está el motor.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
pub struct MovementIntent(pub Vec3);
