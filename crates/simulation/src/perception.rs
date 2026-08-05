//! Los ojos: qué ve cada jugador y qué recuerda de lo que dejó de ver.
//!
//! Es el único que escribe `ObservationMemory` y `Beliefs`, y desde que
//! `select_player_movement` lee lo segundo, los veintidós deciden sobre campos
//! distintos. Ver pide ángulo, alcance y línea —un cuerpo delante esconde lo que
//! hay detrás—, y enterarse de lo visto cuesta `reaction` más. Lo que queda de
//! omnisciencia es la forma de `Ball.predictions`: la trayectoria futura real,
//! que cada uno persigue desviada por lo que cree, pero no calcula.

use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use std::time::Duration;

use crate::SimulationSet;
use crate::team_tactics::PlayerReading;
use football_domain::{
    Ball, HIDDEN_BLUR, Looking, MatchTuning, Observation, ObservationMemory, Player, PlayerId,
    Position, SHOUTED_BLUR, TOTAL_LOSS, Velocity, Vision, can_see, hidden_by, misjudged_pace,
};

pub struct PerceptionPlugin;

impl Plugin for PerceptionPlugin {
    fn build(&self, app: &mut App) {
        // Se mira, se hace uno una idea, y después se decide (§3).
        app.init_resource::<Beliefs>().add_systems(
            FixedUpdate,
            (observe_the_pitch, hear_the_others, believe_the_pitch)
                .chain()
                .in_set(SimulationSet::Players)
                .before(crate::player_decisions::select_player_movement),
        );
    }
}

/// El campo tal y como se lo imagina cada jugador: uno por cabeza y no uno para
/// todos, que era la omnisciencia. Dos compañeros pueden decidir sobre campos
/// distintos, y esa diferencia es el error. Los `Vec` se reutilizan (§12).
#[derive(Resource, Debug, Default)]
pub struct Beliefs {
    by_player: Vec<(PlayerId, Vec<PlayerReading>)>,
    ball_position: Vec<(PlayerId, Vec2)>,
    ball_error: Vec<(PlayerId, Vec2)>,
    ball_uncertainty: Vec<(PlayerId, f32)>,
}

impl Beliefs {
    pub fn of(&self, who: PlayerId) -> &[PlayerReading] {
        self.by_player
            .iter()
            .find(|(id, _)| *id == who)
            .map_or(&[], |(_, readings)| readings.as_slice())
    }

    /// Lo que este jugador se equivoca sobre dónde está el balón, en metros.
    ///
    /// Hoy solo gobierna hacia dónde mira y qué se dibuja: sumarlo también a la
    /// trayectoria que persigue dejó el partido sin un solo tiro, porque un
    /// error de un metro impide un contacto que se decide en sesenta y cinco.
    pub fn ball_error_of(&self, who: PlayerId) -> Vec2 {
        self.ball_error
            .iter()
            .find(|(id, _)| *id == who)
            .map_or(Vec2::ZERO, |(_, error)| *error)
    }

    /// Dónde sitúa este jugador el balón ahora. No devuelve la verdad como
    /// respaldo: no haberlo observado significa no saber dónde está (§3).
    pub fn ball_of(&self, who: PlayerId) -> Option<Vec2> {
        self.ball_position
            .iter()
            .find(|(id, _)| *id == who)
            .map(|(_, position)| *position)
    }

    /// Cuánto duda este jugador de dónde está el balón, en metros.
    ///
    /// Es lo que separa «lo tengo ahí» de «lo perdí», y a diferencia del error
    /// —que solo se puede calcular desde fuera, comparando con la verdad— esto
    /// el jugador sí lo sabe: hace cuánto que no mira y qué tan rápido iba.
    /// No haberlo visto nunca es la duda máxima, no la ausencia de duda.
    pub fn ball_uncertainty_of(&self, who: PlayerId) -> f32 {
        self.ball_uncertainty
            .iter()
            .find(|(id, _)| *id == who)
            .map_or(TOTAL_LOSS, |(_, doubt)| *doubt)
    }

    fn remember_ball_uncertainty(&mut self, who: PlayerId, doubt: f32) {
        match self.ball_uncertainty.iter_mut().find(|(id, _)| *id == who) {
            Some((_, known)) => *known = doubt,
            None => self.ball_uncertainty.push((who, doubt)),
        }
    }

    fn remember_ball(&mut self, who: PlayerId, position: Vec2) {
        match self.ball_position.iter_mut().find(|(id, _)| *id == who) {
            Some((_, known)) => *known = position,
            None => self.ball_position.push((who, position)),
        }
    }

    fn remember_ball_error(&mut self, who: PlayerId, error: Vec2) {
        match self.ball_error.iter_mut().find(|(id, _)| *id == who) {
            Some((_, known)) => *known = error,
            None => self.ball_error.push((who, error)),
        }
    }

    fn slot_for(&mut self, who: PlayerId) -> &mut Vec<PlayerReading> {
        if let Some(index) = self.by_player.iter().position(|(id, _)| *id == who) {
            let readings = &mut self.by_player[index].1;
            readings.clear();
            return readings;
        }
        self.by_player.push((who, Vec::with_capacity(22)));
        &mut self.by_player.last_mut().expect("acaba de insertarse").1
    }
}

/// El grito: lo que un compañero de al lado sabe del balón y uno no.
///
/// Es el sensor que no necesita cono ni línea, y por eso es el que compensa la
/// oclusión: al que se le pierde el balón detrás de un cuerpo se lo cantan. No
/// es telepatía —llega con `SHOUTED_BLUR` encima, porque un aviso dice por
/// dónde y no dónde—, ni es gratis: entra por la misma cola que lo visto, así
/// que el que oye paga su reacción sobre la del que ya la pagó.
///
/// Solo se grita lo que mejora: nadie avisa de algo más viejo de lo que el otro
/// ya tiene, y por eso esto no puede empeorar una creencia.
pub fn hear_the_others(
    time: Res<Time>,
    tuning: Res<MatchTuning>,
    mut voices: Query<(&Player, &Position, &mut ObservationMemory)>,
    mut said: Local<Vec<(PlayerId, Vec2, Option<Observation>)>>,
    mut warned: Local<Vec<(PlayerId, Vec2, PlayerId, Observation)>>,
) {
    let range = tuning.perception.shout_range;
    let now = time.elapsed();
    said.clear();
    warned.clear();
    // Solo habla el que le toca hablar. Los avisos se reparten en el tiempo con
    // el dorsal, como los barridos, para que no canten los once a la vez ni
    // haga falta un RNG para decidirlo (§5).
    for (player, position, memory) in voices
        .iter()
        .filter(|(player, ..)| speaks_now(now, player.id, tuning.perception.shout_interval))
    {
        let mouth = position.on_pitch();
        said.push((player.id, mouth, memory.ball()));
        // «¡hombre!»: lo que se canta de un rival es de los rivales que uno ve,
        // y solo importa a quien lo tiene encima, así que el aviso viaja con el
        // nombre de a quién le va dirigido, no a voces.
        warned.extend(
            memory
                .everyone()
                .filter(|(id, _)| id.team != player.id.team)
                .map(|(id, seen)| (player.id, mouth, id, seen)),
        );
    }

    for (player, position, mut memory) in voices.iter_mut() {
        let ears = position.on_pitch();
        let within_earshot = said
            .iter()
            .filter(|(who, spot, _)| *who != player.id && spot.distance(ears) <= range)
            .filter_map(|(_, _, ball)| *ball);

        if let Some(shouted) = worth_hearing(memory.ball(), within_earshot) {
            memory.saw_ball(shouted);
        }

        // De los rivales solo se avisa del que se le viene encima: cantar los
        // once sería pasarle el campo entero, y en el campo se grita «¡hombre!»
        // por el que llega, no por los demás.
        let closing_in = warned
            .iter()
            .filter(|(who, mouth, about, seen)| {
                *who != player.id
                    && about.team != player.id.team
                    && mouth.distance(ears) <= range
                    && seen.spot.distance(ears) <= WARNING_DISTANCE
            })
            .min_by(|left, right| {
                left.3
                    .spot
                    .distance(ears)
                    .total_cmp(&right.3.spot.distance(ears))
            })
            .map(|(_, _, about, seen)| (*about, *seen));

        if let Some((about, seen)) = closing_in
            && let Some(shouted) = worth_hearing(memory.of(about), [seen].into_iter())
        {
            memory.saw(about, shouted);
        }
    }
}

/// A qué distancia de uno un rival deja de ser un dato y pasa a ser un aviso.
/// Por dentro de esto es el que llega, y es del único del que se grita.
const WARNING_DISTANCE: f32 = 8.0;

/// Si a este le toca hablar en este tick. Un tick de cada `interval`, y cada
/// uno el suyo: repartido por dorsal, reproducible y sin estado que guardar.
fn speaks_now(now: Duration, who: PlayerId, interval: Duration) -> bool {
    let interval = interval.as_millis();
    if interval == 0 {
        return true;
    }
    let slot = (who.team.index() * 11 + usize::from(who.shirt.saturating_sub(1))) % 22;
    let turn = interval * slot as u128 / 22;
    (now.as_millis() + turn) % interval < TICK_MILLIS
}

/// A qué altura se mira a un cuerpo: al tronco, que es lo que se busca para
/// saber dónde está y hacia dónde va.
const TORSO_HEIGHT: f32 = 1.2;

/// Lo que dura un tick, que es la ventana en la que cae un aviso suelto. Va
/// atado a `SIMULATION_HZ` por un test: una constante duplicada que nadie
/// comprueba es una mentira esperando su turno.
const TICK_MILLIS: u128 = 10;

/// Qué aviso vale la pena: el más fresco de los que dicen algo más nuevo de lo
/// que uno ya sabe, con el precio de haberlo oído en vez de verlo.
///
/// Nadie grita lo viejo, así que esto no puede empeorar una creencia: o llega
/// algo más reciente, o no llega nada.
fn worth_hearing(
    known: Option<Observation>,
    around: impl Iterator<Item = Observation>,
) -> Option<Observation> {
    let mine = known.map(|known| known.seen_at);
    around
        .filter(|shouted| mine.is_none_or(|when| shouted.seen_at > when))
        .max_by_key(|shouted| shouted.seen_at)
        .map(|shouted| Observation {
            blur: shouted.blur + SHOUTED_BLUR,
            ..shouted
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use football_domain::scenario::SIMULATION_HZ;
    use std::time::Duration;

    fn ball_seen_at(seconds: u64, blur: f32) -> Observation {
        Observation {
            spot: Vec2::X * seconds as f32,
            velocity: Vec2::ZERO,
            seen_at: Duration::from_secs(seconds),
            blur,
        }
    }

    /// Se oye lo que se sabe más nuevo, y se oye peor de lo que se ve: nadie
    /// grita a cuántos metros está el balón.
    #[test]
    fn a_shout_is_worth_hearing_only_when_it_is_newer() {
        let mine = ball_seen_at(5, 0.5);
        let older = ball_seen_at(2, 0.0);
        let newer = ball_seen_at(9, 0.0);

        assert!(worth_hearing(Some(mine), [older].into_iter()).is_none());
        assert!(worth_hearing(Some(mine), [].into_iter()).is_none());

        let heard = worth_hearing(Some(mine), [older, newer].into_iter()).expect("alguien lo vio");
        assert_eq!(heard.seen_at, newer.seen_at);
        assert!(
            heard.blur > newer.blur,
            "lo oído tenía que valer menos que lo visto"
        );
    }

    /// El tick que se supone que dura un tick.
    #[test]
    fn the_tick_window_matches_the_simulation_rate() {
        assert!((1000.0 / SIMULATION_HZ - 10.0).abs() < f64::EPSILON);
    }

    /// Se habla por turnos y no a la vez: cada uno tiene su momento dentro del
    /// intervalo, y en un intervalo entero habla todo el mundo una vez.
    #[test]
    fn everybody_gets_their_turn_to_speak_and_only_one_tick() {
        let interval = Duration::from_millis(1500);
        let who = PlayerId::home(3);

        let turns = (0..150)
            .map(|tick| Duration::from_millis(tick * 10))
            .filter(|now| speaks_now(*now, who, interval))
            .count();

        assert_eq!(turns, 1, "habló {turns} veces en un intervalo");
        assert_ne!(
            (0..150)
                .map(|tick| Duration::from_millis(tick * 10))
                .position(|now| speaks_now(now, who, interval)),
            (0..150)
                .map(|tick| Duration::from_millis(tick * 10))
                .position(|now| speaks_now(now, PlayerId::home(4), interval)),
            "dos compañeros hablaban en el mismo tick"
        );
    }

    /// Y al que no lo ha visto nunca se lo cantan igual: es lo que compensa
    /// haberlo perdido detrás de un cuerpo.
    #[test]
    fn somebody_who_never_saw_it_gets_told() {
        let told = worth_hearing(None, [ball_seen_at(3, 0.0)].into_iter());

        assert_eq!(
            told.map(|heard| heard.seen_at),
            Some(Duration::from_secs(3))
        );
    }
}

/// Lo que cada jugador cree del resto: donde los vio, adelantados hasta ahora.
/// A quien nunca ha visto no está —no puede recibir un pase suyo ni contar como
/// marca—, y eso es lo que separa esto de la verdad que se leía antes.
pub fn believe_the_pitch(
    time: Res<Time>,
    mut beliefs: ResMut<Beliefs>,
    ball_query: Query<&Position, (With<Ball>, Without<Player>)>,
    who_is_who: Query<(&Player, &ObservationMemory, &Position, &Velocity)>,
) {
    let now = time.elapsed();
    let ball_now = ball_query.single().ok().map(|position| position.on_pitch());
    // los datos que no se observan porque no cambian: quién es cada dorsal
    let identities: Vec<(PlayerId, &Player)> = who_is_who
        .iter()
        .map(|(player, ..)| (player.id, player))
        .collect();

    for (player, memory, position, velocity) in who_is_who.iter() {
        // quien no lo ve lo sitúa donde lo dejó, adelantado a ojo
        let believed_ball = memory.ball().map(|seen| seen.projected_to(now));
        let error = match (believed_ball, ball_now) {
            (Some(believed), Some(actual)) => believed - actual,
            // sin haberlo visto nunca no hay creencia que corregir
            _ => Vec2::ZERO,
        };
        beliefs.remember_ball_error(player.id, error);
        beliefs.remember_ball_uncertainty(
            player.id,
            memory
                .ball()
                .map_or(TOTAL_LOSS, |seen| seen.uncertainty(now)),
        );
        if let Some(position) = believed_ball {
            beliefs.remember_ball(player.id, position);
        }

        let readings = beliefs.slot_for(player.id);
        // uno se conoce a sí mismo sin tener que mirarse
        readings.push(PlayerReading {
            id: player.id,
            playing_position: player.position,
            role: player.role,
            pos: position.on_pitch(),
            vel: velocity.0.truncate(),
            formation_slot: player.formation_slot,
            doubt: 0.0,
        });
        for (seen_id, seen) in memory.everyone() {
            let Some((_, known)) = identities.iter().find(|(id, _)| *id == seen_id) else {
                continue;
            };
            readings.push(PlayerReading {
                id: seen_id,
                playing_position: known.position,
                role: known.role,
                pos: seen.projected_to(now),
                vel: seen.velocity,
                formation_slot: known.formation_slot,
                doubt: seen.uncertainty(now),
            });
        }
    }
}

/// Un desplazamiento de `blur` metros que solo depende del sitio: así el mismo
/// cuerpo se sitúa mal siempre igual mientras no se mueva, en vez de temblar.
fn blurred_by(spot: Vec2, blur: f32) -> Vec2 {
    if blur <= 0.0 {
        return Vec2::ZERO;
    }
    let angle = (spot.x * 12.9898 + spot.y * 78.233).sin() * 43758.547;
    Vec2::from_angle(angle - angle.floor()) * blur
}

/// Cuánto lo esconde el que más lo esconda, de 0 a 1. Ni quien mira ni lo
/// mirado cuentan como estorbo: uno no se tapa a sí mismo.
///
/// El máximo y no la suma: dos cuerpos tapando el mismo trozo no tapan el doble,
/// y quien asoma por un lado asoma igual haya uno o cinco detrás.
fn hidden_behind_somebody(
    eyes: Vec2,
    target: Vec2,
    height: f32,
    crowd: &[(PlayerId, Vec2)],
    watcher: PlayerId,
    seen: Option<PlayerId>,
) -> f32 {
    crowd
        .iter()
        .filter(|(id, _)| *id != watcher && Some(*id) != seen)
        .map(|(_, spot)| hidden_by(eyes, target, height, *spot))
        .fold(0.0, f32::max)
}

/// Lo que cada jugador ve este tick entra en su memoria; lo que no, se queda
/// como estaba y envejece solo.
pub fn observe_the_pitch(
    time: Res<Time>,
    tuning: Res<MatchTuning>,
    ball_query: Query<(&Position, &Ball), Without<Player>>,
    bodies: Query<(&Position, &Player, &Velocity), Without<Ball>>,
    // el cono cuelga de los ojos y no del pecho: `Looking`, no `Facing`
    mut watchers: Query<(
        &Position,
        &Looking,
        &Player,
        &Vision,
        &mut ObservationMemory,
    )>,
    // quién estorba a quién se pregunta 22 × 21 veces por tick: la plantilla se
    // recoge una vez en un buffer que se reutiliza, no una por observador (§12)
    mut crowd: Local<Vec<(PlayerId, Vec2)>>,
) {
    let now = time.elapsed();
    // la altura del balón entra en lo que se ve: un centro pasa por encima de
    // las cabezas que taparían un balón raso
    let ball = ball_query
        .single()
        .ok()
        .map(|(position, ball)| (position.on_pitch(), position.0.z, ball.momentum.truncate()));

    crowd.clear();
    crowd.extend(
        bodies
            .iter()
            .map(|(position, player, _)| (player.id, position.on_pitch())),
    );

    for (position, looking, watcher, vision, mut memory) in watchers.iter_mut() {
        let eyes = position.on_pitch();
        // primero uno se entera de lo que vio hace un momento, y luego mira
        memory.settle(now, tuning.perception.reaction);

        for (other_position, other, velocity) in bodies.iter() {
            if other.id == watcher.id {
                continue;
            }
            let spot = other_position.on_pitch();
            if !can_see(eyes, looking.0, spot, vision) {
                continue;
            }
            // ver medio cuerpo por encima de un hombro es verlo, y es situarlo
            // peor: solo desaparece quien está tapado del todo
            // un cuerpo se mira al tronco, y a esa altura no hay cabeza que
            // salvar: lo que vuela es el balón y no la gente
            let hidden = hidden_behind_somebody(
                eyes,
                spot,
                TORSO_HEIGHT,
                &crowd,
                watcher.id,
                Some(other.id),
            );
            if hidden >= 1.0 {
                continue;
            }
            // lo lejano se sitúa a bulto: el desenfoque es determinista y sale
            // de dónde está, o el mismo cuerpo bailaría cada tick
            let blur = vision.blur_at(eyes.distance(spot)) + hidden * HIDDEN_BLUR;
            memory.saw(
                other.id,
                Observation {
                    spot: spot + blurred_by(spot, blur),
                    // la dirección de la carrera se acierta y el ritmo no, y
                    // por eso lo que se cree se adelanta o se queda corto
                    velocity: velocity.0.truncate() * misjudged_pace(watcher.id, Some(other.id)),
                    seen_at: now,
                    blur,
                },
            );
        }

        let ball_hidden = ball.map_or(1.0, |(spot, height, _)| {
            hidden_behind_somebody(eyes, spot, height, &crowd, watcher.id, None)
        });
        if let Some((spot, _, momentum)) = ball
            && can_see(eyes, looking.0, spot, vision)
            && ball_hidden < 1.0
        {
            // El balón se sitúa donde está y se declara con cuánta duda. Meter
            // el desenfoque también en el punto es una fuente de error nueva y
            // va aparte: aquí solo se dice lo que ya se sabía mal.
            memory.saw_ball(Observation {
                spot,
                velocity: momentum * misjudged_pace(watcher.id, None),
                seen_at: now,
                blur: vision.blur_at(eyes.distance(spot)) + ball_hidden * HIDDEN_BLUR,
            });
        }
    }
}
