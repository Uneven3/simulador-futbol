//! ¿Qué sabe cada jugador del campo?
//!
//! Los sensores están puestos pero todavía no los lee nadie. Esto comprueba que
//! recogen algo antes de enchufarlos: a cuánta gente ve un jugador de golpe, y
//! cuánto se le queda vieja la información del resto.

use bevy::app::TaskPoolPlugin;
use bevy::prelude::*;
use bevy::time::{TimePlugin, TimeUpdateStrategy};
use football_domain::scenario::TICK;
use football_domain::{ObservationMemory, Player, Scenario};
use football_simulation::MatchKernelPlugin;
use football_simulation::perception::Beliefs;
use std::time::Duration;

#[test]
#[ignore = "medición, no una afirmación"]
fn what_does_a_player_know() {
    let scenario = Scenario::kick_off().for_duration(Duration::from_secs(60));
    let ticks = scenario.ticks();

    let mut app = App::new();
    app.add_plugins((TaskPoolPlugin::default(), TimePlugin));
    app.add_plugins(MatchKernelPlugin::new(scenario));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK));

    for _ in 0..ticks {
        app.update();
    }

    let now = app.world().resource::<Time>().elapsed();
    let ids: Vec<_> = {
        let mut bodies = app.world_mut().query::<&Player>();
        bodies.iter(app.world()).map(|player| player.id).collect()
    };
    let ball_errors: Vec<f32> = {
        let beliefs = app.world().resource::<Beliefs>();
        ids.iter()
            .map(|id| beliefs.ball_error_of(*id).length())
            .collect()
    };

    let mut watchers = app.world_mut().query::<(&ObservationMemory, &Player)>();
    let mut known = 0;
    let mut stale_total = 0.0_f32;
    let mut stale_count = 0;
    let mut ball_seen = 0;
    let mut people = 0;

    for (memory, _) in watchers.iter(app.world()) {
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
        f64::from(known as u32) / f64::from(people as u32),
        stale_total / stale_count as f32
    );
}
