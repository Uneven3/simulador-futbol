//! Lo que un jugador sabe, que no es lo que pasa.
//!
//! Hasta aquí todo el mundo leía la verdad: posiciones exactas de los
//! veintiuno restantes y la trayectoria futura real del balón. Esto es el
//! escalón que MVP 4 mete en medio (§3): un sensor con campo visual, una
//! memoria que envejece, y decisiones que leen creencias.

use crate::identity::PlayerId;
use crate::player::PLAYER_BODY_RADIUS;
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
    /// Lo que se falló al situarlo en el momento de verlo, en metros: el
    /// `blur_at` de la distancia a la que estaba. Un punto sin esto es una
    /// creencia que no sabe cuánto vale.
    pub blur: f32,
}

/// Cuánto se le escapa a uno la velocidad de lo que mira: se ve que alguien va
/// lanzado, no a cuántos metros por segundo. Por eso una creencia vieja de algo
/// que corre vale menos que una igual de vieja de algo parado.
pub const VELOCITY_MISJUDGED: f32 = 0.25;

/// Dónde deja de importar seguir dudando, en metros. Pasado esto la respuesta ya
/// es la misma —no se sabe dónde está— y un número más grande no dice más.
pub const TOTAL_LOSS: f32 = 30.0;

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

    /// Cuántos metros puede estar equivocada esta creencia ahora mismo: lo que
    /// se falló al verlo más lo que se ha escapado desde entonces.
    ///
    /// La edad entra entera y no recortada al horizonte, al revés que en
    /// `projected_to`: dejar de mover un punto no es dejar de dudar de él, y esa
    /// asimetría es justo lo que distingue «lo tengo ahí» de «lo perdí».
    pub fn uncertainty(self, now: Duration) -> f32 {
        let drift = self.velocity.length() * VELOCITY_MISJUDGED * self.age(now).as_secs_f32();
        (self.blur + drift).min(TOTAL_LOSS)
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

/// Cuánta perspectiva hace falta entre dos cuerpos para que uno esconda al otro,
/// en metros. Nadie tapa lo que lleva al lado: el balón que uno conduce se ve
/// junto a él, no detrás de él, y sin esto conducir volvería invisible el balón
/// para los veintiuno restantes.
pub const SHADOW_NEEDS_DEPTH: f32 = 1.0;

/// Si el cuerpo plantado en `blocker` esconde `target` de quien mira desde
/// `eyes`.
///
/// Es la sombra de un cilindro vista desde un punto, y por eso se ensancha con
/// la distancia: alguien pegado a uno tapa un sector enorme del campo y el mismo
/// cuerpo a veinte metros no tapa casi nada. Esconder no es borrar —lo que se
/// deja de ver se queda en la memoria y envejece—, que es lo que impide que un
/// cruce delante de los ojos evapore a un compañero.
pub fn blocks_the_view(eyes: Vec2, target: Vec2, blocker: Vec2) -> bool {
    let to_target = target - eyes;
    let distance = to_target.length();
    let Ok(direction) = Dir2::new(to_target) else {
        return false;
    };
    let to_blocker = blocker - eyes;
    let along = to_blocker.dot(*direction);
    // ni a la espalda de quien mira ni junto a lo que taparía
    if along <= 0.0 || along >= distance - SHADOW_NEEDS_DEPTH {
        return false;
    }
    let aside = to_blocker.perp_dot(*direction).abs();
    // el `max` es tener a alguien encima: la sombra no se dispara al infinito,
    // se queda en tapar todo lo que hay detrás de él
    aside <= PLAYER_BODY_RADIUS * distance / along.max(PLAYER_BODY_RADIUS)
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
            blur: 0.0,
        };

        let far_future = seen.projected_to(Duration::from_secs(30));
        assert!(
            far_future.x <= 20.0 * EXTRAPOLATION_HORIZON.as_secs_f32() + 0.01,
            "medio minuto después lo situaba a {} m",
            far_future.x
        );
    }

    /// Dejar de mover un punto no es dejar de dudar de él: pasado el horizonte
    /// la creencia se queda quieta y la incertidumbre sigue subiendo.
    #[test]
    fn doubt_keeps_growing_after_the_point_stops_moving() {
        let seen = Observation {
            spot: Vec2::ZERO,
            velocity: Vec2::new(10.0, 0.0),
            seen_at: Duration::ZERO,
            blur: 0.0,
        };
        let horizon = EXTRAPOLATION_HORIZON;
        let later = horizon * 2;

        assert_eq!(seen.projected_to(horizon), seen.projected_to(later));
        assert!(seen.uncertainty(later) > seen.uncertainty(horizon));
        assert!(seen.uncertainty(Duration::from_secs(300)) <= TOTAL_LOSS);
    }

    /// Un cuerpo en medio esconde lo que hay detrás, y solo lo que hay detrás:
    /// ni lo que está a su lado, ni lo que está más cerca que él.
    #[test]
    fn a_body_in_the_way_hides_what_is_behind_it() {
        let eyes = Vec2::ZERO;
        let target = Vec2::new(20.0, 0.0);

        assert!(blocks_the_view(eyes, target, Vec2::new(10.0, 0.0)));
        assert!(!blocks_the_view(eyes, target, Vec2::new(10.0, 5.0)));
        assert!(!blocks_the_view(eyes, target, Vec2::new(-5.0, 0.0)));
        assert!(
            !blocks_the_view(eyes, target, Vec2::new(25.0, 0.0)),
            "detrás de lo que se mira no se tapa nada"
        );
    }

    /// La sombra se ensancha con la distancia: el mismo cuerpo tapa un sector
    /// enorme desde cerca de los ojos y casi nada desde lejos.
    #[test]
    fn the_shadow_widens_with_distance() {
        let eyes = Vec2::ZERO;
        let target = Vec2::new(30.0, 0.0);
        let aside = 1.5;

        assert!(blocks_the_view(eyes, target, Vec2::new(2.0, aside)));
        assert!(!blocks_the_view(eyes, target, Vec2::new(25.0, aside)));
    }

    /// Nadie tapa lo que lleva al lado: el balón que uno conduce se ve junto a
    /// él, o conducir dejaría el balón invisible para los otros veintiuno.
    #[test]
    fn nobody_hides_what_they_carry() {
        let eyes = Vec2::ZERO;
        let carrier = Vec2::new(15.0, 0.0);
        let ball_at_his_feet = carrier + Vec2::new(0.4, 0.0);

        assert!(!blocks_the_view(eyes, ball_at_his_feet, carrier));
        assert!(blocks_the_view(
            eyes,
            carrier + Vec2::new(SHADOW_NEEDS_DEPTH + 1.0, 0.0),
            carrier
        ));
    }

    /// Lo que se falló al verlo no se olvida, y lo que corre se escapa más
    /// rápido que lo que está parado.
    #[test]
    fn what_runs_is_lost_faster_than_what_stands_still() {
        let now = Duration::from_secs(1);
        let standing = Observation {
            spot: Vec2::ZERO,
            velocity: Vec2::ZERO,
            seen_at: Duration::ZERO,
            blur: 1.5,
        };
        let running = Observation {
            velocity: Vec2::new(8.0, 0.0),
            ..standing
        };

        assert!((standing.uncertainty(Duration::ZERO) - 1.5).abs() < f32::EPSILON);
        assert!((standing.uncertainty(now) - 1.5).abs() < f32::EPSILON);
        assert!(running.uncertainty(now) > standing.uncertainty(now));
    }
}
