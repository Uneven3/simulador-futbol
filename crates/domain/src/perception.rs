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

/// Qué alcanza a ver un cuerpo, y desde dónde.
///
/// El campo visual humano útil son unos 180 grados en total; la visión nítida
/// es mucho más estrecha, pero para saber dónde está un cuerpo que se mueve
/// basta la periférica.
#[derive(Component, Debug, Clone, Copy, Reflect)]
pub struct Vision {
    /// Medio ángulo del campo visual, en radianes: lo que hay a cada lado de
    /// donde se mira.
    pub half_angle: f32,
    /// Hasta dónde se distingue a alguien, en metros. Más allá se sabe que hay
    /// gente, no quién ni exactamente dónde.
    pub range: f32,
}

impl Default for Vision {
    fn default() -> Self {
        Self {
            half_angle: std::f32::consts::FRAC_PI_2,
            range: 40.0,
        }
    }
}

/// Lo último que un jugador supo de otro cuerpo.
///
/// Guarda cuándo lo supo, porque una posición de hace tres segundos no es una
/// posición: es un punto de partida. Quien decide con esto tiene que poder
/// preguntar cuánto hace.
#[derive(Debug, Clone, Copy, Reflect)]
pub struct Observation {
    pub spot: Vec2,
    pub velocity: Vec2,
    pub seen_at: Duration,
}

impl Observation {
    /// Dónde estaría si hubiera seguido igual. Es lo que hace un futbolista con
    /// lo que dejó de ver: no lo olvida, lo extrapola —y se equivoca—.
    pub fn projected_to(self, now: Duration) -> Vec2 {
        let stale_for = now.saturating_sub(self.seen_at).as_secs_f32();
        self.spot + self.velocity * stale_for
    }

    pub fn age(self, now: Duration) -> Duration {
        now.saturating_sub(self.seen_at)
    }
}

/// Lo que un jugador sabe del resto del campo, uno por uno.
///
/// Es memoria y no una foto: lo que no se ve no desaparece, se queda como
/// estaba y envejece. Un `Vec` y no un mapa porque son veintiuno como mucho y
/// se recorre entero cada tick (§12).
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
