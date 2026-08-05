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

/// Hacia dónde apunta el cuerpo, que es lo que decide a qué ritmo se corre. La
/// escribe el motor con su propio límite de giro, que es lento: un cuerpo
/// lanzado tarda en encarar otra cosa.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct Facing(pub Dir2);

impl Default for Facing {
    fn default() -> Self {
        Self(Dir2::X)
    }
}

/// Hacia dónde miran los ojos, que no es hacia dónde apunta el cuerpo: el
/// cuello da un margen y la cabeza gira dentro de él mucho más rápido. De aquí
/// cuelga el campo visual, y por eso mirar alrededor ya no cuesta velocidad.
///
/// Es a `Gaze` lo que `Velocity` a `MovementIntent`: lo conseguido, no lo
/// pedido (§3).
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct Looking(pub Dir2);

impl Default for Looking {
    fn default() -> Self {
        Self(Dir2::X)
    }
}

/// El punto que un cuerpo quiere mirar, si quiere mirar alguno.
///
/// Lo escribe quien decide y lo obedece el motor: la cabeza si le llega el
/// cuello, y el cuerpo entero cuando no. `None` es mirar hacia donde se va, que
/// es lo que hace quien esprinta.
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
