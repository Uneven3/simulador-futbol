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
use football_domain::{Attributes, Facing, FatigueState, MatchTuning, MovementIntent, Velocity};

/// La velocidad que un cuerpo alcanza este tick.
///
/// El presupuesto es uno solo y se reparte solo: cambiar de dirección gasta lo
/// mismo que cambiar de rapidez, porque las dos cosas son la misma —empujar el
/// suelo en alguna dirección—. De ahí sale gratis lo que antes no existía: para
/// darse la vuelta a la carrera hay que frenar primero, y girar sin frenar
/// describe una curva de radio proporcional al cuadrado de la rapidez.
///
/// Frenar cuesta menos que acelerar, y el que decide cuál es el caso es el
/// cambio proyectado sobre la carrera: lo que le quita al avance es freno,
/// venga acompañado de lo que venga.
pub fn reachable_velocity(current: Vec2, desired: Vec2, body: &Attributes, dt: f32) -> Vec2 {
    let change = desired - current;
    let slowing_down = Dir2::new(current).is_ok_and(|heading| change.dot(*heading) < 0.0);
    let budget = if slowing_down {
        body.braking
    } else {
        body.acceleration
    } * dt;

    if change.length() <= budget {
        return desired;
    }
    current + change.normalize_or_zero() * budget
}

/// Lo que queda del depósito después de un tick a esta velocidad.
///
/// El gasto va con el cuadrado del esfuerzo, que es lo que hace que el trote
/// salga casi gratis y el sprint no: correr al doble no cuesta el doble. Por
/// debajo del trote no se gasta, se recupera.
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

/// Qué fracción de su velocidad alcanza quien corre hacia donde no mira.
///
/// De frente, toda; de lado y de espaldas, lo que dice el cuerpo humano. Sin
/// esto un defensor retrocede tan rápido como corre de cara, que es la razón
/// por la que nadie desbordaba a nadie.
pub fn pace_towards(facing: Dir2, heading: Dir2, body: &Attributes, tuning: &TurningTuning) -> f32 {
    let alignment = facing.dot(*heading);
    let raw = if alignment >= 0.0 {
        tuning.sideways_pace + (1.0 - tuning.sideways_pace) * alignment
    } else {
        tuning.backpedal_pace + (tuning.sideways_pace - tuning.backpedal_pace) * (1.0 + alignment)
    };
    raw + (1.0 - raw) * body.lateral_technique
}

/// Cada cuerpo persigue la velocidad que le pidieron, dentro de lo que puede —y
/// lo que puede se le va gastando, y depende de hacia dónde esté mirando.
pub fn drive_bodies(
    time: Res<Time>,
    tuning: Res<MatchTuning>,
    mut bodies: Query<(
        &MovementIntent,
        &Attributes,
        &mut FatigueState,
        &mut Facing,
        &mut Velocity,
    )>,
) {
    let dt = time.delta_secs();
    let (stamina_tuning, turning) = (&tuning.stamina, &tuning.turning);
    for (intent, body, mut fatigue, mut facing, mut velocity) in bodies.iter_mut() {
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

        // se mira hacia donde se quiere ir, no hacia donde se acabó yendo: un
        // cuerpo empujado de lado no gira la cabeza por eso
        if let Ok(towards) = Dir2::new(asked) {
            facing.0 = turned(facing.0, towards, speed, body, turning, dt);
        }
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

    /// Y girar sin frenar sale curvo: pedir el mismo módulo noventa grados a un
    /// lado no lo consigue en un tick, y lo que se consigue es intermedio.
    #[test]
    fn a_turn_at_speed_comes_out_as_a_curve() {
        let body = Attributes::default();
        let running = Vec2::new(6.0, 0.0);
        let sideways = Vec2::new(0.0, 6.0);

        let after_one_tick = reachable_velocity(running, sideways, &body, TICK);

        assert!(after_one_tick.x > 5.0, "perdió toda la carrera de golpe");
        assert!(after_one_tick.y > 0.0, "no giró nada");
        assert!(after_one_tick.y < 1.0, "giró demasiado para un tick");
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
}
