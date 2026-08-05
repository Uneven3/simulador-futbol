//! El motor: lo que un cuerpo consigue de lo que su dueño le pidió.
//!
//! Hasta aquí la decisión escribía la velocidad y el cuerpo la tenía, así que
//! un jugador pasaba de parado a ocho metros por segundo en diez milisegundos y
//! giraba en redondo sin perder un céntimo de rapidez. Eso no es un jugador: es
//! un punto obedeciendo. Este módulo es el escalón que separa querer de poder
//! (§3), y todo lo que cuesta llegar sale de un solo presupuesto de metros por
//! segundo al cuadrado.

use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;

use football_domain::tuning::{StaminaTuning, TurningTuning};
use football_domain::{
    Attributes, Facing, FatigueState, Gaze, Looking, MatchTuning, MovementIntent, Position, Stance,
    Velocity,
};

/// La velocidad que un cuerpo alcanza este tick.
///
/// Un cuerpo no es una partícula: lo que puede hacer depende de hacia dónde,
/// porque no lo limita lo mismo. Acelerar de frente lo limita la potencia de la
/// pierna; frenar y cortar los limita el agarre del taco, que da mucho más. Con
/// un presupuesto único —el modelo anterior— cambiar de dirección salía un arco
/// amplio, y veintidós jugadores orbitando el balón.
pub fn reachable_velocity(current: Vec2, desired: Vec2, body: &Attributes, dt: f32) -> Vec2 {
    let change = desired - current;
    let Ok(heading) = Dir2::new(current) else {
        // parado no hay adelante ni lado: arrancar es arrancar
        return step_towards(current, change, body.acceleration * dt);
    };

    let along = change.dot(*heading);
    let across = change - *heading * along;
    let forward = along.clamp(-body.braking * dt, body.acceleration * dt);
    let sideways = across.clamp_length_max(body.grip * dt);

    // El agarre es uno y se reparte: lo que se gasta en cortar no está para
    // empujar. De ahí sale sola la pérdida de carrera al cambiar de dirección,
    // sin escribirla —y escribirla aparte los dejó clavados—.
    let step = (*heading * forward + sideways).clamp_length_max(body.grip * dt);
    current + step
}

/// Lo que se avanza hacia `change` sin pasarse del presupuesto.
fn step_towards(current: Vec2, change: Vec2, budget: f32) -> Vec2 {
    if change.length() <= budget {
        return current + change;
    }
    current + change.normalize_or_zero() * budget
}

/// Lo que queda del depósito tras un tick a esta velocidad. El gasto va con el
/// cuadrado del esfuerzo —correr al doble no cuesta el doble—, y por debajo del
/// trote no se gasta: se recupera.
pub fn drained(
    stamina: f32,
    speed: f32,
    body: &Attributes,
    tuning: &StaminaTuning,
    dt: f32,
) -> f32 {
    let change = if speed <= tuning.recovery_pace {
        tuning.recovery
    } else {
        let effort = (speed / body.top_speed).clamp(0.0, 1.0);
        -tuning.sprint_drain * effort * effort
    };
    (stamina + change * dt).clamp(0.0, 1.0)
}

/// Lo que este cuerpo puede hoy, que es lo suyo menos lo que lleva corrido.
///
/// Devuelve unos `Attributes` y no un factor porque lo que cambia es lo que el
/// cuerpo es capaz de hacer, y el motor no tiene por qué saber por qué.
pub fn capacity_of(body: &Attributes, fatigue: FatigueState, tuning: &StaminaTuning) -> Attributes {
    let scaled = |fresh: f32, spent: f32| fresh * (spent + (1.0 - spent) * fatigue.stamina);
    Attributes {
        top_speed: scaled(body.top_speed, tuning.spent_speed),
        acceleration: scaled(body.acceleration, tuning.spent_acceleration),
        braking: body.braking,
        ..*body
    }
}

/// Hacia dónde mira un cuerpo después de un tick.
///
/// Girar es más lento a la carrera que parado, así que la vuelta completa que
/// se da desde quieto se convierte en un arco largo si viene corriendo.
pub fn turned(
    facing: Dir2,
    towards: Dir2,
    speed: f32,
    body: &Attributes,
    tuning: &TurningTuning,
    dt: f32,
) -> Dir2 {
    let effort = (speed / body.top_speed).clamp(0.0, 1.0);
    let rate = body.turn_rate * (1.0 - effort * (1.0 - tuning.turn_at_speed));
    let angle = facing.angle_to(*towards);
    let step = angle.clamp(-rate * dt, rate * dt);
    Dir2::new(Vec2::from_angle(step).rotate(*facing)).unwrap_or(facing)
}

/// Hasta dónde llega la vista sin girarse: lo que se quiere mirar si el cuello
/// da para ello, y el tope del cuello si no.
pub fn within_the_neck(towards: Dir2, facing: Dir2, tuning: &TurningTuning) -> Dir2 {
    let aside = facing.angle_to(*towards);
    if aside.abs() <= tuning.neck_range {
        return towards;
    }
    let held = aside.clamp(-tuning.neck_range, tuning.neck_range);
    Dir2::new(Vec2::from_angle(held).rotate(*facing)).unwrap_or(facing)
}

/// Adónde llegan los ojos este tick. La cabeza gira sola y deprisa —de ahí que
/// mirar alrededor no cueste velocidad—, pero el cuello la sujeta: para ver más
/// allá de su margen hay que girar el cuerpo, y eso sí cuesta.
pub fn turned_head(
    looking: Dir2,
    towards: Dir2,
    facing: Dir2,
    tuning: &TurningTuning,
    dt: f32,
) -> Dir2 {
    let reachable = within_the_neck(towards, facing, tuning);
    let angle = looking.angle_to(*reachable);
    let step = angle.clamp(-tuning.neck_rate * dt, tuning.neck_rate * dt);
    Dir2::new(Vec2::from_angle(step).rotate(*looking)).unwrap_or(looking)
}

/// Qué fracción de su velocidad alcanza quien corre hacia donde no mira: de
/// frente toda, y de lado o de espaldas lo que da el cuerpo humano. Sin esto un
/// defensor retrocede tan rápido como corre de cara.
pub fn pace_towards(facing: Dir2, heading: Dir2, body: &Attributes, tuning: &TurningTuning) -> f32 {
    let alignment = facing.dot(*heading);
    let raw = if alignment >= 0.0 {
        tuning.sideways_pace + (1.0 - tuning.sideways_pace) * alignment
    } else {
        tuning.backpedal_pace + (tuning.sideways_pace - tuning.backpedal_pace) * (1.0 + alignment)
    };
    raw + (1.0 - raw) * body.lateral_technique
}

/// Lo que el motor necesita de un cuerpo: lo que pidió, lo que puede, lo que
/// quiere mirar y dónde está —y lo que el motor le escribe, que es en qué se
/// queda todo eso.
type DrivenBody = (
    &'static MovementIntent,
    &'static Attributes,
    &'static Gaze,
    &'static Stance,
    &'static Position,
    &'static mut FatigueState,
    &'static mut Facing,
    &'static mut Looking,
    &'static mut Velocity,
);

/// Cada cuerpo persigue la velocidad que le pidieron, dentro de lo que puede —y
/// lo que puede se le va gastando, y depende de hacia dónde esté mirando.
pub fn drive_bodies(time: Res<Time>, tuning: Res<MatchTuning>, mut bodies: Query<DrivenBody>) {
    let dt = time.delta_secs();
    let (stamina_tuning, turning) = (&tuning.stamina, &tuning.turning);
    for (
        intent,
        body,
        gaze,
        stance,
        position,
        mut fatigue,
        mut facing,
        mut looking,
        mut velocity,
    ) in bodies.iter_mut()
    {
        let running = velocity.0.truncate();
        let speed = running.length();
        fatigue.stamina = drained(fatigue.stamina, speed, body, stamina_tuning, dt);

        let can = capacity_of(body, *fatigue, stamina_tuning);
        let asked = intent.0.truncate();
        let reachable_pace = match Dir2::new(asked) {
            Ok(heading) => can.top_speed * pace_towards(facing.0, heading, body, turning),
            Err(_) => can.top_speed,
        };
        let reached = reachable_velocity(running, asked.clamp_length_max(reachable_pace), &can, dt);
        velocity.0 = Vec3::new(reached.x, reached.y, velocity.0.z);

        // Se mira lo que se quiso mirar, y si no se quiso nada, hacia donde se
        // va: un cuerpo empujado de lado no gira la cabeza por eso.
        let towards = |spot: Vec2| spot - position.on_pitch();
        let wanted = Dir2::new(gaze.0.map_or(asked, towards)).unwrap_or(looking.0);

        // El cuerpo se planta donde la decisión dijo —el balón, casi siempre— y
        // ahí sigue pagando el peaje de correr hacia otro lado. Lo que no lo
        // arrastra es el barrido: para eso está el cuello, y solo cuando la
        // mirada no le cabe se gira el torso a acompañarla.
        let planted = Dir2::new(stance.0.map_or(asked, towards)).unwrap_or(facing.0);
        let body_target = if facing.0.angle_to(*wanted).abs() > turning.neck_range {
            wanted
        } else {
            planted
        };
        facing.0 = turned(facing.0, body_target, speed, body, turning, dt);
        looking.0 = turned_head(looking.0, wanted, facing.0, turning, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: f32 = 0.01;

    /// Salir de parado lleva tiempo: la punta no se alcanza en un tick.
    #[test]
    fn a_standing_body_does_not_reach_top_speed_in_one_tick() {
        let body = Attributes::default();
        let sprint = Vec2::new(body.top_speed, 0.0);

        let after_one_tick = reachable_velocity(Vec2::ZERO, sprint, &body, TICK);

        assert!(
            after_one_tick.length() < 0.1,
            "salió disparado a {} m/s",
            after_one_tick.length()
        );
    }

    /// Y lleva el tiempo que dice el atributo: ocho metros por segundo a seis
    /// de aceleración son un segundo y un tercio, y no lo que quede bonito.
    #[test]
    fn the_time_to_top_speed_is_the_attribute_and_nothing_else() {
        let body = Attributes::default();
        let sprint = Vec2::new(body.top_speed, 0.0);
        let mut velocity = Vec2::ZERO;
        let mut ticks = 0;

        while velocity.length() < body.top_speed - 0.01 {
            velocity = reachable_velocity(velocity, sprint, &body, TICK);
            ticks += 1;
        }

        let expected = body.top_speed / body.acceleration;
        let took = ticks as f32 * TICK;
        assert!(
            (took - expected).abs() < 0.02,
            "tardó {took} s en llegar a la punta y debía tardar {expected} s"
        );
    }

    /// Frenar es más rápido que acelerar, que es lo que separa a un cuerpo de
    /// un punto: los dos gastos no son el mismo.
    #[test]
    fn stopping_is_quicker_than_starting() {
        let body = Attributes::default();
        let sprint = Vec2::new(body.top_speed, 0.0);

        let started = reachable_velocity(Vec2::ZERO, sprint, &body, TICK).length();
        let braked = sprint.length() - reachable_velocity(sprint, Vec2::ZERO, &body, TICK).length();

        assert!(
            braked > started,
            "frenó {braked} m/s y arrancó {started} m/s en el mismo tiempo"
        );
    }

    /// Darse la vuelta a la carrera pasa por pararse. Nadie va a la vez a siete
    /// metros por segundo hacia un lado y hacia el otro.
    #[test]
    fn turning_around_at_speed_goes_through_a_stop() {
        let body = Attributes::default();
        let sprinting = Vec2::new(7.0, 0.0);
        let backwards = Vec2::new(-7.0, 0.0);

        let mut velocity = sprinting;
        let mut slowest = f32::MAX;
        for _ in 0..300 {
            velocity = reachable_velocity(velocity, backwards, &body, TICK);
            slowest = slowest.min(velocity.length());
        }

        assert!(slowest < 0.5, "se dio la vuelta sin bajar de {slowest} m/s");
        assert!(velocity.x < -6.0, "no acabó yendo hacia el otro lado");
    }

    /// Cortar es mucho más rápido que acelerar, porque no lo limita la pierna
    /// sino el agarre. Un futbolista planta el pie y sale; no describe una curva.
    #[test]
    fn cutting_is_quicker_than_accelerating() {
        let body = Attributes::default();
        let running = Vec2::new(6.0, 0.0);

        let cut = reachable_velocity(running, Vec2::new(0.0, 6.0), &body, TICK);
        let sprint = reachable_velocity(running, Vec2::new(12.0, 0.0), &body, TICK);

        assert!(
            cut.y > (sprint.x - running.x) * 2.0,
            "cortar dio {} m/s de lado y acelerar {} de frente",
            cut.y,
            sprint.x - running.x
        );
    }

    /// Y cortar cuesta carrera, sin que nadie lo haya escrito: el agarre es uno
    /// y lo que se gasta en cambiar de dirección no está para empujar.
    #[test]
    fn cutting_costs_pace_by_itself() {
        let body = Attributes::default();
        let running = Vec2::new(6.0, 0.0);

        let mut velocity = running;
        for _ in 0..40 {
            velocity = reachable_velocity(velocity, Vec2::new(0.0, 6.0), &body, TICK);
        }

        assert!(
            velocity.length() < running.length(),
            "cambió de dirección sin perder un metro por segundo"
        );
        assert!(velocity.y > 1.0, "en cuatro décimas no había girado nada");
    }

    /// Correr de espaldas es más lento que de cara, y de lado, intermedio.
    #[test]
    fn running_where_you_are_not_looking_is_slower() {
        let tuning = TurningTuning::default();
        let looking = Dir2::X;

        let body = Attributes::default();
        let ahead = pace_towards(looking, Dir2::X, &body, &tuning);
        let sideways = pace_towards(looking, Dir2::Y, &body, &tuning);
        let backwards = pace_towards(looking, Dir2::NEG_X, &body, &tuning);

        assert!((ahead - 1.0).abs() < 0.01, "de frente no iba a tope");
        assert!(backwards < sideways && sideways < ahead);
        assert!(backwards > 0.4, "de espaldas quedó por debajo de andar");
    }

    /// Darse la vuelta lleva su tiempo, y parado se hace antes que corriendo.
    #[test]
    fn turning_around_takes_longer_at_speed() {
        let body = Attributes::default();
        let tuning = TurningTuning::default();

        let turn_from = |speed: f32| {
            let mut facing = Dir2::X;
            let mut ticks = 0;
            while facing.dot(*Dir2::NEG_X) < 0.99 {
                facing = turned(facing, Dir2::NEG_X, speed, &body, &tuning, TICK);
                ticks += 1;
            }
            ticks as f32 * TICK
        };

        let standing = turn_from(0.0);
        let sprinting = turn_from(body.top_speed);

        assert!(
            (0.3..0.8).contains(&standing),
            "media vuelta parado tardó {standing} s"
        );
        assert!(
            sprinting > standing * 2.0,
            "girar a la carrera ({sprinting} s) tenía que costar bastante más que parado"
        );
    }

    /// El trote se sostiene noventa minutos, que es lo que hace un futbolista.
    #[test]
    fn a_jog_can_be_held_for_a_whole_match() {
        let body = Attributes::default();
        let tuning = StaminaTuning::default();
        let mut stamina = 1.0;

        for _ in 0..(90 * 60 * 100) {
            stamina = drained(stamina, tuning.recovery_pace, &body, &tuning, TICK);
        }

        assert!(stamina > 0.99, "el trote lo dejó en {stamina}");
    }

    /// Y el sprint no: medio minuto largo a punta y hay que bajar el ritmo.
    #[test]
    fn a_flat_out_sprint_empties_the_tank_in_under_a_minute() {
        let body = Attributes::default();
        let tuning = StaminaTuning::default();
        let mut stamina = 1.0;
        let mut seconds = 0.0;

        while stamina > 0.0 {
            stamina = drained(stamina, body.top_speed, &body, &tuning, TICK);
            seconds += TICK;
        }

        assert!(
            (20.0..90.0).contains(&seconds),
            "aguantó {seconds} s a velocidad punta"
        );
    }

    /// Y lo que se pierde cansado es sobre todo el arranque, no la punta: un
    /// jugador vacío sigue corriendo, pero le cuesta ponerse.
    #[test]
    fn an_empty_tank_costs_more_acceleration_than_top_speed() {
        let body = Attributes::default();
        let tuning = StaminaTuning::default();

        let spent = capacity_of(&body, FatigueState { stamina: 0.0 }, &tuning);
        let fresh = capacity_of(&body, FatigueState::default(), &tuning);

        assert!(
            (fresh.top_speed - body.top_speed).abs() < 0.01,
            "fresco no es él mismo"
        );
        assert!(spent.top_speed < body.top_speed);
        assert!(
            spent.acceleration / body.acceleration < spent.top_speed / body.top_speed,
            "el arranque tenía que caer más que la punta"
        );
    }

    /// Lo que cabe en el presupuesto se consigue entero: un cuerpo casi parado
    /// al que le piden casi nada no tiene por qué quedarse a medias.
    #[test]
    fn a_small_change_is_reached_outright() {
        let body = Attributes::default();
        let crawl = Vec2::new(0.01, 0.0);

        assert_eq!(reachable_velocity(Vec2::ZERO, crawl, &body, TICK), crawl);
    }

    /// El cuello llega hasta donde llega: mirar al costado es gratis y mirar a
    /// la espalda no, porque eso ya es girarse.
    #[test]
    fn the_neck_reaches_the_shoulder_and_no_further() {
        let tuning = TurningTuning::default();
        let facing = Dir2::X;
        let aside = Dir2::from_angle(tuning.neck_range * 0.5);
        let behind = Dir2::NEG_X;

        assert_eq!(within_the_neck(aside, facing, &tuning), aside);
        let held = within_the_neck(behind, facing, &tuning);
        assert!(
            (facing.angle_to(*held).abs() - tuning.neck_range).abs() < 0.001,
            "el cuello se quedó en {} rad",
            facing.angle_to(*held).abs()
        );
    }

    /// Y la cabeza llega antes que el cuerpo: en el tiempo que un barrido dura,
    /// los ojos han hecho el recorrido y el cuerpo apenas se ha movido.
    #[test]
    fn the_head_gets_there_before_the_body_does() {
        let tuning = TurningTuning::default();
        let body = Attributes::default();
        let scan = std::time::Duration::from_millis(400).as_secs_f32();
        let towards = Dir2::from_angle(tuning.neck_range);

        let mut looking = Dir2::X;
        let mut facing = Dir2::X;
        for _ in 0..(scan / TICK) as u32 {
            facing = turned(facing, Dir2::X, 0.0, &body, &tuning, TICK);
            looking = turned_head(looking, towards, facing, &tuning, TICK);
        }

        assert!(
            looking.angle_to(*towards).abs() < 0.01,
            "los ojos se quedaron a {} rad del sitio",
            looking.angle_to(*towards).abs()
        );
        assert!(
            facing.angle_to(*Dir2::X).abs() < 0.01,
            "el cuerpo giró para mirar al costado, que es lo que había que evitar"
        );
    }
}
