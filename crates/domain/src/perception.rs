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

/// Lo visto que todavía no se sabe: entre que la luz entra y la cabeza se
/// entera pasa un tiempo, y mientras tanto lo que se cree del mundo es lo de
/// antes.
#[derive(Debug, Clone, Copy, Default, Reflect)]
struct Delayed {
    known: Option<Observation>,
    on_the_way: Option<Observation>,
}

impl Delayed {
    /// Lo que se acaba de ver entra a la cola, y solo si la cola está vacía: lo
    /// que llega después no adelanta a lo que ya venía. Por eso la creencia se
    /// refresca a golpes y no de forma continua, que es lo que hace un ojo.
    fn saw(&mut self, what: Observation) {
        if self.on_the_way.is_none() {
            self.on_the_way = Some(what);
        }
    }

    /// Enterarse: lo que se vio hace más de `reaction` ya es sabido, con la hora
    /// a la que se vio y no la de ahora. Así la latencia paga dos veces, que es
    /// lo justo: la creencia nace vieja y nace dudada.
    fn settle(&mut self, now: Duration, reaction: Duration) {
        if let Some(seen) = self.on_the_way
            && seen.age(now) >= reaction
        {
            self.known = Some(seen);
            self.on_the_way = None;
        }
    }
}

/// Lo que un jugador sabe del resto del campo. Es memoria y no una foto: lo que
/// no se ve no desaparece, se queda como estaba y envejece. Un `Vec` porque son
/// veintiuno y se recorre entero cada tick (§12).
#[derive(Component, Debug, Clone, Default, Reflect)]
pub struct ObservationMemory {
    seen: Vec<(PlayerId, Delayed)>,
    /// El balón, que es el cuerpo que todo el mundo mira.
    ball: Delayed,
}

impl ObservationMemory {
    pub fn saw(&mut self, who: PlayerId, what: Observation) {
        match self.seen.iter_mut().find(|(id, _)| *id == who) {
            Some((_, slot)) => slot.saw(what),
            None => {
                let mut slot = Delayed::default();
                slot.saw(what);
                self.seen.push((who, slot));
            }
        }
    }

    pub fn saw_ball(&mut self, what: Observation) {
        self.ball.saw(what);
    }

    /// Todo lo que estaba de camino y ya ha llegado. Se llama una vez por tick,
    /// antes de mirar: primero se entera uno de lo de antes y luego ve.
    pub fn settle(&mut self, now: Duration, reaction: Duration) {
        for (_, slot) in &mut self.seen {
            slot.settle(now, reaction);
        }
        self.ball.settle(now, reaction);
    }

    pub fn of(&self, who: PlayerId) -> Option<Observation> {
        self.seen
            .iter()
            .find(|(id, _)| *id == who)
            .and_then(|(_, slot)| slot.known)
    }

    pub fn ball(&self) -> Option<Observation> {
        self.ball.known
    }

    pub fn everyone(&self) -> impl Iterator<Item = (PlayerId, Observation)> + '_ {
        self.seen
            .iter()
            .filter_map(|(id, slot)| slot.known.map(|known| (*id, known)))
    }

    /// A cuánta gente se conoce: haber visto a alguien hace un instante y no
    /// haberse enterado todavía es no conocerlo.
    pub fn known_count(&self) -> usize {
        self.seen
            .iter()
            .filter(|(_, slot)| slot.known.is_some())
            .count()
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

/// Dónde acaba la sombra, en anchos de cuerpo: hasta la mitad no se ve nada, y
/// desde ahí hasta el borde se ve un trozo. Un cuerpo no es una pared opaca de
/// contorno exacto —asoma la cabeza, asoma un hombro, y los dos se mueven—, así
/// que entre tapado y despejado hay penumbra.
pub const SHADOW_CORE: f32 = 0.5;
pub const SHADOW_EDGE: f32 = 1.5;

/// Lo que se falla de más al situar a alguien del que solo se ve un trozo, en
/// metros. Sin referencia: es el ancho de un cuerpo, que es lo que puede haberse
/// desplazado el que está detrás sin que se le vea hacerlo.
pub const HIDDEN_BLUR: f32 = 0.7;

/// Cuánto esconde el cuerpo plantado en `blocker` lo que hay en `target`, de 0
/// —despejado— a 1 —tapado del todo—.
///
/// Es la sombra de un cilindro vista desde un punto, y por eso se ensancha con
/// la distancia: alguien pegado a uno tapa un sector enorme del campo y el mismo
/// cuerpo a veinte metros no tapa casi nada. Esconder no es borrar —lo que se
/// deja de ver se queda en la memoria y envejece—, y taparse a medias tampoco:
/// eso es verlo peor situado, que es lo que ocurre casi siempre en el campo.
pub fn hidden_by(eyes: Vec2, target: Vec2, blocker: Vec2) -> f32 {
    let to_target = target - eyes;
    let distance = to_target.length();
    let Ok(direction) = Dir2::new(to_target) else {
        return 0.0;
    };
    let to_blocker = blocker - eyes;
    let along = to_blocker.dot(*direction);
    // ni a la espalda de quien mira ni junto a lo que taparía
    if along <= 0.0 || along >= distance - SHADOW_NEEDS_DEPTH {
        return 0.0;
    }
    // el `max` es tener a alguien encima: la sombra no se dispara al infinito,
    // se queda en tapar todo lo que hay detrás de él
    let shadow = PLAYER_BODY_RADIUS * distance / along.max(PLAYER_BODY_RADIUS);
    let aside = to_blocker.perp_dot(*direction).abs();
    let core = shadow * SHADOW_CORE;
    let edge = shadow * SHADOW_EDGE;
    if aside <= core {
        return 1.0;
    }
    ((edge - aside) / (edge - core)).clamp(0.0, 1.0)
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

    /// Lo que se acaba de ver no se sabe todavía, y cuando se sabe se sabe
    /// viejo: la creencia nace con la edad que tardó en llegar.
    #[test]
    fn what_you_just_saw_is_not_yet_known() {
        let reaction = Duration::from_millis(200);
        let mut memory = ObservationMemory::default();
        let who = PlayerId::home(7);
        let seen = Observation {
            spot: Vec2::new(5.0, 0.0),
            velocity: Vec2::ZERO,
            seen_at: Duration::ZERO,
            blur: 0.0,
        };

        memory.saw(who, seen);
        memory.settle(Duration::ZERO, reaction);
        assert!(memory.of(who).is_none(), "verlo no es enterarse");
        assert_eq!(memory.known_count(), 0);

        memory.settle(reaction, reaction);
        assert_eq!(memory.of(who).map(|known| known.spot), Some(seen.spot));
        assert_eq!(
            memory.of(who).map(|known| known.age(reaction)),
            Some(reaction),
            "se sabe con la hora en que se vio, no con la de ahora"
        );
    }

    /// Mientras algo viene de camino, lo que se ve después no lo adelanta: la
    /// creencia se refresca a golpes, y entre golpe y golpe está vieja.
    #[test]
    fn a_newer_sight_does_not_overtake_the_one_on_the_way() {
        let reaction = Duration::from_millis(200);
        let mut memory = ObservationMemory::default();
        let first = Observation {
            spot: Vec2::X,
            velocity: Vec2::ZERO,
            seen_at: Duration::ZERO,
            blur: 0.0,
        };
        let later = Observation {
            spot: Vec2::X * 9.0,
            seen_at: Duration::from_millis(100),
            ..first
        };

        memory.saw_ball(first);
        memory.saw_ball(later);
        memory.settle(reaction, reaction);

        assert_eq!(memory.ball().map(|known| known.spot), Some(first.spot));
    }

    /// Un cuerpo en medio esconde lo que hay detrás, y solo lo que hay detrás:
    /// ni lo que está a su lado, ni lo que está más cerca que él.
    #[test]
    fn a_body_in_the_way_hides_what_is_behind_it() {
        let eyes = Vec2::ZERO;
        let target = Vec2::new(20.0, 0.0);

        assert_eq!(hidden_by(eyes, target, Vec2::new(10.0, 0.0)), 1.0);
        assert_eq!(hidden_by(eyes, target, Vec2::new(10.0, 5.0)), 0.0);
        assert_eq!(hidden_by(eyes, target, Vec2::new(-5.0, 0.0)), 0.0);
        assert_eq!(
            hidden_by(eyes, target, Vec2::new(25.0, 0.0)),
            0.0,
            "detrás de lo que se mira no se tapa nada"
        );
    }

    /// Y entre tapado y despejado hay penumbra: por el borde de la sombra se ve
    /// un trozo, que es lo que pasa casi siempre en un campo con veintidós.
    #[test]
    fn the_edge_of_a_shadow_is_seeing_part_of_somebody() {
        let eyes = Vec2::ZERO;
        let target = Vec2::new(20.0, 0.0);
        let shadow = PLAYER_BODY_RADIUS * 2.0; // el bloqueador está a media vía

        let core = hidden_by(eyes, target, Vec2::new(10.0, shadow * SHADOW_CORE * 0.5));
        let penumbra = hidden_by(eyes, target, Vec2::new(10.0, shadow));
        let clear = hidden_by(eyes, target, Vec2::new(10.0, shadow * SHADOW_EDGE * 1.1));

        assert_eq!(core, 1.0);
        assert!(
            penumbra > 0.0 && penumbra < 1.0,
            "el borde tapaba {penumbra} en vez de un trozo"
        );
        assert_eq!(clear, 0.0);
    }

    /// La sombra se ensancha con la distancia: el mismo cuerpo tapa un sector
    /// enorme desde cerca de los ojos y casi nada desde lejos.
    #[test]
    fn the_shadow_widens_with_distance() {
        let eyes = Vec2::ZERO;
        let target = Vec2::new(30.0, 0.0);
        let aside = 1.5;

        assert_eq!(hidden_by(eyes, target, Vec2::new(2.0, aside)), 1.0);
        assert_eq!(hidden_by(eyes, target, Vec2::new(25.0, aside)), 0.0);
    }

    /// Nadie tapa lo que lleva al lado: el balón que uno conduce se ve junto a
    /// él, o conducir dejaría el balón invisible para los otros veintiuno.
    #[test]
    fn nobody_hides_what_they_carry() {
        let eyes = Vec2::ZERO;
        let carrier = Vec2::new(15.0, 0.0);
        let ball_at_his_feet = carrier + Vec2::new(0.4, 0.0);

        assert_eq!(hidden_by(eyes, ball_at_his_feet, carrier), 0.0);
        assert_eq!(
            hidden_by(
                eyes,
                carrier + Vec2::new(SHADOW_NEEDS_DEPTH + 1.0, 0.0),
                carrier
            ),
            1.0
        );
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
