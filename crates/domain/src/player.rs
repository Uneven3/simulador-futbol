//! A participant: who he is, what he is asked to do, what he can do and what
//! the match has done to him.
//!
//! These are four components rather than one struct because they age
//! differently. Identity and instruction are decided before kick-off; capacity
//! is a property of the person; the match state is rewritten every tick. Merged,
//! any read of a shirt number needs write access to the tick's scratch data.

use crate::identity::{ByTeam, PlayerId, TeamId};
use crate::math::XorShift32;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::prelude::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Where a player lines up. Position is a place on the pitch, and it is
/// deliberately not the same thing as what he is asked to do there
/// (`TacticalRole`): a full back can be told to attack without becoming a
/// winger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, Serialize, Deserialize)]
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

/// Lo que mide un cuerpo, en metros, mientras la antropometría no sea un dato
/// por jugador. Es lo que tapa: por encima de esto la vista pasa.
pub const PLAYER_HEIGHT: f32 = 1.8;

/// A qué altura están los ojos. Un poco por debajo de la coronilla, que es la
/// diferencia entre ver por encima del que tienes delante y no verlo.
pub const EYE_HEIGHT: f32 = PLAYER_HEIGHT * 0.94;

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
    /// Cuánto agarra el pie contra el suelo (m/s²), que es el techo de todo lo
    /// que puede hacer un cuerpo a la vez. Dos g largos: por eso se corta en
    /// seco y no en curva, aunque acelerar de frente sea mucho más lento.
    pub grip: f32,
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
            grip: 18.0,
            turn_rate: 6.0,
            lateral_technique: 0.0,
            height: 1.8,
            shot_technique: 0.5,
        }
    }
}

/// Capacidades espejo de los once dorsales. La misma persona nominal tiene las
/// mismas piernas en ambos equipos para que la variación individual no rompa la
/// simetría local/visita de un escenario.
pub fn player_attributes(seed: u32, shirt: u8, position: PlayingPosition) -> Attributes {
    let shirt_key = u32::from(shirt).wrapping_mul(0xD1B5_4A35);
    let mut rng = XorShift32(seed ^ shirt_key);
    let mut attributes = Attributes {
        top_speed: rng.range(7.1, 8.5),
        acceleration: rng.range(5.0, 7.0),
        braking: rng.range(7.5, 10.0),
        grip: rng.range(15.0, 20.0),
        turn_rate: rng.range(5.0, 7.0),
        shot_technique: rng.range(0.3, 0.8),
        ..Default::default()
    };
    if position.is_goalkeeper() {
        attributes.lateral_technique = 1.0;
        attributes.top_speed = rng.range(6.0, 7.0);
        attributes.shot_technique = rng.range(0.2, 0.5);
    }
    attributes
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
}

/// La acción defensiva elegida para este tick. Contener no autoriza a atravesar
/// el radio de contacto para robar: la entrada es una decisión distinta que el
/// motor puede fallar o convertir en falta.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
pub enum DefensiveAction {
    #[default]
    Contain,
    Tackle,
}

/// Sanción individual acumulada durante este partido. Es decisión del árbitro,
/// no una propiedad estable del jugador ni una consecuencia del motor.
#[derive(Component, Debug, Clone, Copy, Default, Reflect)]
pub struct Discipline {
    pub yellow_cards: u8,
    pub sent_off: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Card {
    Yellow,
    Red,
}

/// Lo que el equipo ha pedido explícitamente a este jugador para este tick.
/// La responsabilidad no mueve un cuerpo: la decisión individual la consume
/// junto con su propia creencia, y el motor decide después qué alcanza (§3).
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct TacticalResponsibility {
    pub kind: ResponsibilityKind,
    /// Persona a quien cubre, presiona o apoya. Una identidad viene del
    /// registro, no de observar un `Entity`.
    pub target: Option<PlayerId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum ResponsibilityKind {
    Occupy,
    Cover,
    Press,
    Support,
}

impl Default for TacticalResponsibility {
    fn default() -> Self {
        Self {
            kind: ResponsibilityKind::Occupy,
            target: None,
        }
    }
}

/// La política que un equipo lleva al escenario. Es instrucción, no habilidad:
/// cambiarla altera responsabilidades sin convertir a nadie en otro jugador.
#[derive(Debug, Clone, Copy, PartialEq, Reflect, Serialize, Deserialize)]
pub struct TacticalPlan {
    /// Profundidad prepartido de la línea defensiva, 0 = cerca del propio arco
    /// y 1 = bloque alto. Es una instrucción, no una lectura del rival.
    pub defensive_line_depth: f32,
    /// Hasta esta distancia la cobertura se transforma en presión.
    pub press_distance: f32,
    /// Más allá de esto un rival conocido no reclama una cobertura individual.
    pub cover_distance: f32,
    /// Distancia a la que un compañero con balón merece un apoyo cercano.
    pub support_distance: f32,
}

impl Default for TacticalPlan {
    fn default() -> Self {
        Self {
            defensive_line_depth: 0.8,
            press_distance: 8.0,
            cover_distance: 35.0,
            support_distance: 18.0,
        }
    }
}

/// Planes configurables de los dos equipos, instalados por el escenario para
/// que una comparación contrafactual cambie una política y nada más.
#[derive(Resource, Debug, Clone, PartialEq, Reflect, Serialize, Deserialize)]
pub struct TacticalPlans {
    pub team: ByTeam<TacticalPlan>,
}

impl Default for TacticalPlans {
    fn default() -> Self {
        Self {
            team: ByTeam::splat(TacticalPlan::default()),
        }
    }
}

impl TacticalPlans {
    pub fn for_team(&self, team: TeamId) -> TacticalPlan {
        self.team[team]
    }
}

/// 0..1: cuánto conoce un jugador los movimientos de su posición nominal.
/// Solo reduce el radio de cobertura que puede asumir; no suma una bonificación
/// global a cada decisión.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct PositionFamiliarity(pub f32);

/// 0..1: cuánto conoce la función táctica que le pidió el plan actual.
/// Su mecanismo es el mismo que el de posición, pero sigue siendo un dato
/// separado: cambiar de rol no borra la experiencia de ser lateral.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct RoleFamiliarity(pub f32);

impl Default for PositionFamiliarity {
    fn default() -> Self {
        Self(1.0)
    }
}

impl Default for RoleFamiliarity {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Familiaridades espejo de los once dorsales. El azar del escenario hace
/// jugadores distintos sin hacer distinto a un equipo por ser local o visita.
pub fn tactical_familiarity(seed: u32, shirt: u8) -> (PositionFamiliarity, RoleFamiliarity) {
    let shirt_key = u32::from(shirt).wrapping_mul(0x9E37_79B9);
    let mut rng = XorShift32(seed ^ shirt_key);
    (
        PositionFamiliarity(rng.range(0.75, 1.0)),
        RoleFamiliarity(rng.range(0.75, 1.0)),
    )
}

#[cfg(test)]
#[expect(
    clippy::float_cmp,
    reason = "la misma semilla debe reproducir exactamente el mismo perfil"
)]
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

    #[test]
    fn seeded_attributes_are_individual_but_mirrored_by_shirt() {
        let six = player_attributes(7, 6, PlayingPosition::CentreMidfielder);
        let six_again = player_attributes(7, 6, PlayingPosition::CentreMidfielder);
        let seven = player_attributes(7, 7, PlayingPosition::CentreMidfielder);

        assert_eq!(six.top_speed, six_again.top_speed);
        assert_eq!(six.acceleration, six_again.acceleration);
        assert_ne!(six.top_speed, seven.top_speed);
    }
}
