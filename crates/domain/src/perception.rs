//! Lo que un jugador sabe, que no es lo que pasa.
//!
//! Hasta aquí todo el mundo leía la verdad: posiciones exactas de los
//! veintiuno restantes y la trayectoria futura real del balón. Esto es el
//! escalón que MVP 4 mete en medio (§3): un sensor con campo visual, una
//! memoria que envejece, y decisiones que leen creencias.

use crate::identity::PlayerId;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_reflect::prelude::*;
use std::time::Duration;

/// Qué alcanza a ver un cuerpo, y con qué detalle.
///
/// Ver medio campo no es enterarse de medio campo: lo que se sitúa con
/// precisión es el balón y lo cercano, y lo demás se sabe a grandes rasgos. Por
/// eso hay dos distancias y no una.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct Vision {
    /// Medio ángulo del campo visual útil, en radianes.
    pub half_angle: f32,
    /// Hasta dónde se sitúa a alguien con precisión, en metros.
    pub sharp_range: f32,
    /// Y hasta dónde se le ve del todo: entre las dos, se sabe que está y no
    /// exactamente dónde.
    pub range: f32,
}

impl Default for Vision {
    fn default() -> Self {
        Self {
            // unos 100 grados de campo útil en total, no los 180 de antes
            half_angle: std::f32::consts::PI * 0.28,
            sharp_range: 12.0,
            range: 30.0,
        }
    }
}

impl Vision {
    /// Cuánto se equivoca al situar algo a esta distancia, en metros. Dentro de
    /// lo cercano, nada; más allá crece hasta el borde de lo que se distingue.
    pub fn blur_at(&self, distance: f32) -> f32 {
        let beyond = (distance - self.sharp_range).max(0.0);
        let reach = (self.range - self.sharp_range).max(0.01);
        BLUR_AT_THE_EDGE * (beyond / reach).min(1.0)
    }
}

/// Lo que se falla al situar a alguien en el límite de lo que se distingue, en
/// metros. Es un cuerpo y medio: se sabe que está ahí, no en qué pie apoya.
pub const BLUR_AT_THE_EDGE: f32 = 2.5;

/// Lo último que un jugador supo de otro cuerpo, con cuándo lo supo: una
/// posición de hace tres segundos no es una posición, es un punto de partida.
#[derive(Debug, Clone, Copy, Reflect)]
pub struct Observation {
    pub spot: Vec2,
    pub velocity: Vec2,
    pub seen_at: Duration,
}

/// Hasta dónde se extrapola lo que se dejó de ver. Pasado esto uno sabe que ya
/// no sabe: seguir tirando de la recta pone un balón visto a veinte metros por
/// segundo a trescientos metros del campo.
pub const EXTRAPOLATION_HORIZON: Duration = Duration::from_millis(1000);

impl Observation {
    /// Dónde estaría si hubiera seguido igual. Es lo que hace un futbolista con
    /// lo que dejó de ver: no lo olvida, lo extrapola —y se equivoca—.
    pub fn projected_to(self, now: Duration) -> Vec2 {
        let stale_for = self.age(now).min(EXTRAPOLATION_HORIZON).as_secs_f32();
        self.spot + self.velocity * stale_for
    }

    pub fn age(self, now: Duration) -> Duration {
        now.saturating_sub(self.seen_at)
    }
}

/// Lo que un jugador sabe del resto del campo. Es memoria y no una foto: lo que
/// no se ve no desaparece, se queda como estaba y envejece. Un `Vec` porque son
/// veintiuno y se recorre entero cada tick (§12).
#[derive(Component, Debug, Clone, Default, Reflect)]
pub struct ObservationMemory {
    seen: Vec<(PlayerId, Observation)>,
    /// El balón, que es el cuerpo que todo el mundo mira.
    pub ball: Option<Observation>,
}

impl ObservationMemory {
    pub fn remember(&mut self, who: PlayerId, what: Observation) {
        match self.seen.iter_mut().find(|(id, _)| *id == who) {
            Some((_, known)) => *known = what,
            None => self.seen.push((who, what)),
        }
    }

    pub fn of(&self, who: PlayerId) -> Option<Observation> {
        self.seen
            .iter()
            .find(|(id, _)| *id == who)
            .map(|(_, known)| *known)
    }

    pub fn everyone(&self) -> impl Iterator<Item = (PlayerId, Observation)> + '_ {
        self.seen.iter().copied()
    }

    pub fn known_count(&self) -> usize {
        self.seen.len()
    }
}

/// Si un punto cae dentro del campo visual de alguien que mira hacia `facing`.
///
/// Dos cosas y no una: el ángulo y la distancia. Un cuerpo a la espalda no se
/// ve por cerca que esté, y uno de frente a sesenta metros tampoco.
pub fn can_see(from: Vec2, facing: Dir2, target: Vec2, vision: &Vision) -> bool {
    let to_target = target - from;
    let distance = to_target.length();
    if distance > vision.range {
        return false;
    }
    // encima de uno: no hay dirección que mirar, y desde luego se sabe que está
    let Ok(direction) = Dir2::new(to_target) else {
        return true;
    };
    facing.angle_to(*direction).abs() <= vision.half_angle
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lo cercano se sitúa exacto y lo lejano a bulto: ver medio campo no es
    /// enterarse de medio campo.
    #[test]
    fn detail_fades_with_distance() {
        let vision = Vision::default();

        assert!(vision.blur_at(vision.sharp_range * 0.5) < f32::EPSILON);
        assert!(vision.blur_at(vision.sharp_range) < f32::EPSILON);
        assert!(vision.blur_at(vision.range) >= BLUR_AT_THE_EDGE - 0.01);
        assert!(vision.blur_at(vision.range * 0.5) < vision.blur_at(vision.range));
    }

    /// A la espalda no se ve nada, por cerca que esté.
    #[test]
    fn nothing_behind_you_is_seen() {
        let vision = Vision::default();

        assert!(can_see(Vec2::ZERO, Dir2::X, Vec2::new(3.0, 0.0), &vision));
        assert!(!can_see(Vec2::ZERO, Dir2::X, Vec2::new(-1.0, 0.0), &vision));
        assert!(!can_see(
            Vec2::ZERO,
            Dir2::X,
            Vec2::new(vision.range + 5.0, 0.0),
            &vision
        ));
    }

    /// Y lo que se dejó de ver no se extrapola sin fin: pasado el horizonte, la
    /// creencia se queda donde estaba.
    #[test]
    fn a_stale_observation_stops_running_away() {
        let seen = Observation {
            spot: Vec2::ZERO,
            velocity: Vec2::new(20.0, 0.0),
            seen_at: Duration::ZERO,
        };

        let far_future = seen.projected_to(Duration::from_secs(30));
        assert!(
            far_future.x <= 20.0 * EXTRAPOLATION_HORIZON.as_secs_f32() + 0.01,
            "medio minuto después lo situaba a {} m",
            far_future.x
        );
    }
}
