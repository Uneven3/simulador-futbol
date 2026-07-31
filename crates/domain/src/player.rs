//! A participant: who he is, what he is asked to do, what he can do and what
//! the match has done to him.
//!
//! These are four components rather than one struct because they age
//! differently. Identity and instruction are decided before kick-off; capacity
//! is a property of the person; the match state is rewritten every tick. Merged,
//! any read of a shirt number needs write access to the tick's scratch data.

use crate::identity::PlayerId;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::prelude::*;
use std::time::Duration;

/// Where a player lines up. Position is a place on the pitch, and it is
/// deliberately not the same thing as what he is asked to do there
/// (`TacticalRole`): a full back can be told to attack without becoming a
/// winger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum PlayingPosition {
    Goalkeeper,
    CentreBack,
    LeftBack,
    RightBack,
    DefensiveMidfielder,
    CentreMidfielder,
    LeftMidfielder,
    RightMidfielder,
    AttackingMidfielder,
    CentreForward,
}

impl PlayingPosition {
    /// The role a position is given when nobody has said otherwise. Reproduces
    /// the default 4-4-2 the port hardcoded; MVP 5 makes the pairing an
    /// instruction rather than a consequence.
    pub fn default_role(self) -> TacticalRole {
        match self {
            PlayingPosition::Goalkeeper | PlayingPosition::CentreBack => TacticalRole::Defending,
            PlayingPosition::LeftBack
            | PlayingPosition::RightBack
            | PlayingPosition::DefensiveMidfielder => TacticalRole::Holding,
            PlayingPosition::LeftMidfielder
            | PlayingPosition::CentreMidfielder
            | PlayingPosition::RightMidfielder => TacticalRole::Linking,
            PlayingPosition::AttackingMidfielder => TacticalRole::Creating,
            PlayingPosition::CentreForward => TacticalRole::Attacking,
        }
    }

    /// Whether this position is the one allowed to handle the ball (Law 12).
    pub fn is_goalkeeper(self) -> bool {
        self == PlayingPosition::Goalkeeper
    }
}

/// What a player is asked to do, along the one axis the simulation currently
/// reads: how far up the pitch his instructions push him.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
pub enum TacticalRole {
    Defending,
    Holding,
    Linking,
    Creating,
    Attacking,
}

impl TacticalRole {
    /// 0 = keeps his position behind the ball, 1 = plays on the last line.
    /// Drives how far a player drops, how far he hunts and how he biases a
    /// pass (the port read this off the position and called it `mind_set`).
    pub fn attacking_bias(self) -> f32 {
        match self {
            TacticalRole::Defending => 0.0,
            TacticalRole::Holding => 0.25,
            TacticalRole::Linking => 0.5,
            TacticalRole::Creating => 0.75,
            TacticalRole::Attacking => 1.0,
        }
    }
}

/// Identity and instruction: decided before kick-off, unchanged by play.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct Player {
    pub id: PlayerId,
    pub position: PlayingPosition,
    pub role: TacticalRole,
    /// Where this player sits in the team block, -1..1 in both axes
    /// (own goal → forward, team-space left → right). The team AI scales it
    /// into the block's actual shape each tick.
    pub formation_slot: Vec2,
}

impl Player {
    /// A player in his position's default role.
    pub fn new(id: PlayerId, position: PlayingPosition, formation_slot: Vec2) -> Self {
        Self {
            id,
            position,
            role: position.default_role(),
            formation_slot,
        }
    }
}

/// Medio ancho de un cuerpo, en metros. Es lo que dos jugadores no pueden
/// compartir, y lo que uno puede interponer entre un rival y el balón.
pub const PLAYER_BODY_RADIUS: f32 = 0.35;

/// What a player is physically capable of: stable for the match.
///
/// Admission rule (`AHORA.md`): an attribute lives here only once a mechanism
/// reads it, it has a real unit and something calibrates it.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct Attributes {
    /// Top speed in m/s (the port's `sprintVelocity`).
    pub top_speed: f32,
    /// Cuánto puede ganar de velocidad en un segundo (m/s²). Un futbolista sale
    /// de parado a unos 6 y sostiene bastante menos; lo segundo pide la fatiga,
    /// que todavía no está.
    pub acceleration: f32,
    /// Y cuánto puede perder (m/s²), que es más: frenar es apoyar contra el
    /// suelo y acelerar es empujarlo. Es también el presupuesto con el que se
    /// cambia de dirección, y por eso girar a la carrera describe una curva.
    pub braking: f32,
    /// A qué velocidad da la vuelta al cuerpo (rad/s). Seis son unos 340 grados
    /// por segundo: media vuelta en medio segundo, parado.
    pub turn_rate: f32,
    /// 0..1: cuánto de su velocidad conserva yendo hacia donde no mira. Cero es
    /// un jugador de campo, que para correr en serio se gira; uno es un portero,
    /// que se pasa el partido desplazándose de lado sin perder de vista el
    /// balón, y a quien eso se le da igual de bien que ir de frente.
    pub lateral_technique: f32,
    /// Standing height in metres. Used for the body's collision capsule and,
    /// eventually, for aerial duels.
    pub height: f32,
    /// 0..1: how tightly a shot lands where it was aimed.
    pub shot_technique: f32,
}

impl Default for Attributes {
    fn default() -> Self {
        Self {
            top_speed: 8.0,
            acceleration: 6.0,
            braking: 9.0,
            turn_rate: 6.0,
            lateral_technique: 0.0,
            height: 1.8,
            shot_technique: 0.5,
        }
    }
}

/// Lo que le queda de lo que salió teniendo: 1 fresco, 0 vacío. Es capacidad y
/// no disposición —un jugador vacío no es que no quiera—, así que lo lee el
/// motor. Solo baja: los cambios y el descanso son de MVP 2.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct FatigueState {
    pub stamina: f32,
}

impl Default for FatigueState {
    fn default() -> Self {
        Self { stamina: 1.0 }
    }
}

/// Disposition: what a player is inclined to do, as opposed to what he can do.
/// MVP 5 fills this out (discipline, risk, familiarity with the instruction).
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct Mentality {
    /// 0..1: willingness to run when running is not required of him.
    pub work_rate: f32,
}

impl Default for Mentality {
    fn default() -> Self {
        Self { work_rate: 0.5 }
    }
}

/// What this match has done to him so far. Rewritten during play; meaningless
/// outside the match it belongs to.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
pub struct PlayerMatchState {
    /// Rolling ~10 s average of his own speed in m/s. Stands in for breath:
    /// a player who has been running flat out does not immediately sprint again.
    pub recent_speed: f32,
    /// Match time of his last touch, for the touch and collision windows.
    pub last_touch_at: Duration,
    /// Opponent he is currently man-marking, assigned by the team AI each tick.
    pub marking: Option<PlayerId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The port derived attacking intent from the position. That mapping is now
    /// a default rather than an identity, but it has to still produce the same
    /// team shape, or every movement envelope silently changes.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "los sesgos son constantes de la tabla, no cálculos"
    )]
    fn default_roles_reproduce_the_inherited_bias() {
        let expected = [
            (PlayingPosition::Goalkeeper, 0.0),
            (PlayingPosition::CentreBack, 0.0),
            (PlayingPosition::LeftBack, 0.25),
            (PlayingPosition::RightBack, 0.25),
            (PlayingPosition::DefensiveMidfielder, 0.25),
            (PlayingPosition::LeftMidfielder, 0.5),
            (PlayingPosition::CentreMidfielder, 0.5),
            (PlayingPosition::RightMidfielder, 0.5),
            (PlayingPosition::AttackingMidfielder, 0.75),
            (PlayingPosition::CentreForward, 1.0),
        ];

        for (position, bias) in expected {
            assert_eq!(
                position.default_role().attacking_bias(),
                bias,
                "{position:?} changed how far up the pitch it plays"
            );
        }
    }

    /// The separation is only worth anything if a role can contradict the
    /// position it came from.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "los sesgos son constantes de la tabla, no cálculos"
    )]
    fn a_role_can_be_given_against_the_position() {
        let mut player = Player::new(
            crate::identity::PlayerId::home(2),
            PlayingPosition::RightBack,
            Vec2::new(-1.0, 1.0),
        );
        player.role = TacticalRole::Attacking;

        assert_eq!(player.role.attacking_bias(), 1.0);
        assert_eq!(player.position, PlayingPosition::RightBack);
    }
}
