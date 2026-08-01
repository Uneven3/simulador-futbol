//! ¿Qué sabe cada jugador del campo?
//!
//! Los sensores están puestos pero todavía no los lee nadie. Esto comprueba que
//! recogen algo antes de enchufarlos: a cuánta gente ve un jugador de golpe, y
//! cuánto se le queda vieja la información del resto.

use bevy_time::Time;
use football_domain::{ObservationMemory, Player, Scenario};
use football_simulation::ScenarioRunner;
use football_simulation::perception::Beliefs;
use std::time::Duration;

pub fn run() {
    let scenario = Scenario::kick_off().for_duration(Duration::from_secs(60));
    let ticks = scenario.ticks();
    let mut runner = ScenarioRunner::headless(scenario);

    for _ in 0..ticks {
        runner.advance();
    }

    let world = runner.world_mut();
    let now = world.resource::<Time>().elapsed();
    let ids: Vec<_> = {
        let mut bodies = world.query::<&Player>();
        bodies.iter(world).map(|player| player.id).collect()
    };
    let ball_errors: Vec<f32> = {
        let beliefs = world.resource::<Beliefs>();
        ids.iter()
            .map(|id| beliefs.ball_error_of(*id).length())
            .collect()
    };

    let mut watchers = world.query::<(&ObservationMemory, &Player)>();
    let mut known = 0;
    let mut stale_total = 0.0_f32;
    let mut stale_count = 0;
    let mut ball_seen = 0;
    let mut people = 0;

    for (memory, _) in watchers.iter(world) {
        people += 1;
        known += memory.known_count();
        for (_, seen) in memory.everyone() {
            stale_total += seen.age(now).as_secs_f32();
            stale_count += 1;
        }
        if memory.ball.is_some() {
            ball_seen += 1;
        }
    }

    let worst_ball_error = ball_errors.iter().copied().fold(0.0_f32, f32::max);
    let mean_ball_error = ball_errors.iter().sum::<f32>() / ball_errors.len() as f32;
    println!(
        "el balón: se equivocan {mean_ball_error:.1} m de media, el peor {worst_ball_error:.1} m"
    );
    println!(
        "tras un minuto, cada jugador conoce a {:.1} de los otros 21, \
         con información de {:.1} s de antigüedad de media; \
         {ball_seen} de {people} tienen el balón en la cabeza",
        known as f64 / f64::from(people),
        stale_total / stale_count as f32
    );

    crate::record(
        "perception",
        &[
            ("error_balon_medio_m", mean_ball_error),
            ("error_balon_peor_m", worst_ball_error),
            ("companeros_conocidos", known as f32 / people.max(1) as f32),
            (
                "antiguedad_media_s",
                stale_total / stale_count.max(1) as f32,
            ),
            ("con_el_balon_en_la_cabeza", ball_seen as f32),
        ],
    );
}
