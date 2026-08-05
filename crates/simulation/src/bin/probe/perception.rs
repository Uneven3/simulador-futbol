//! ¿Qué sabe cada jugador del campo?
//!
//! A cuánta gente llega a conocer un jugador, cuánto envejece esa información y
//! cuánto se aparta su balón creído del real cuando deja de mirarlo.

use bevy_time::Time;
use football_domain::{MatchTuning, ObservationMemory, Player, Scenario};
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
    let threshold = world.resource::<MatchTuning>().perception.lost_ball_doubt;
    let ids: Vec<_> = {
        let mut bodies = world.query::<&Player>();
        bodies.iter(world).map(|player| player.id).collect()
    };
    // Solo quien tiene una creencia del balón entra en la comparación: a quien
    // no lo ha visto nunca el error le sale cero y la duda máxima, y mezclarlos
    // haría parecer certero al que no sabe nada.
    let (ball_errors, ball_doubts): (Vec<f32>, Vec<f32>) = {
        let beliefs = world.resource::<Beliefs>();
        ids.iter()
            .filter(|id| beliefs.ball_of(**id).is_some())
            .map(|id| {
                (
                    beliefs.ball_error_of(*id).length(),
                    beliefs.ball_uncertainty_of(*id),
                )
            })
            .unzip()
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
    let mean_ball_doubt = ball_doubts.iter().sum::<f32>() / ball_doubts.len() as f32;
    println!(
        "el balón: se equivocan {mean_ball_error:.1} m de media, el peor {worst_ball_error:.1} m"
    );
    // Lo que se falla frente a lo que se cree fallar: el jugador no puede medir
    // lo primero —haría falta la verdad— pero sí lo segundo, y que no cuadren es
    // lo que hace que un optimista se lance a un balón que no llega a disputar.
    let searching = ball_doubts
        .iter()
        .filter(|doubt| **doubt >= threshold)
        .count();
    println!(
        "y dudan {mean_ball_doubt:.1} m de media de dónde está; \
         {searching} de {} lo dudan tanto que lo buscan en vez de reconocer",
        ball_doubts.len()
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
            ("duda_balon_media_m", mean_ball_doubt),
            ("companeros_conocidos", known as f32 / people.max(1) as f32),
            (
                "antiguedad_media_s",
                stale_total / stale_count.max(1) as f32,
            ),
            ("con_el_balon_en_la_cabeza", ball_seen as f32),
        ],
    );
}
