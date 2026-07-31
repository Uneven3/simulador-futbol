//! ¿Cómo llegan las piernas al final?
//!
//! La fatiga bajó el ritmo de gol de 14 a 4,5 por 90, y eso solo vale si los
//! jugadores acaban cansados y no fundidos: veintidós jugadores a cero es otra
//! forma de partido roto, no un partido con fatiga.

use bevy::app::TaskPoolPlugin;
use bevy::prelude::*;
use bevy::time::{TimePlugin, TimeUpdateStrategy};
use football_domain::scenario::TICK;
use football_domain::{FatigueState, Player, Scenario};
use football_simulation::MatchKernelPlugin;
use std::time::Duration;

#[test]
#[ignore = "medición, no una afirmación"]
fn how_much_is_left_in_the_legs() {
    let minutes = 45;
    let scenario = Scenario::kick_off().for_duration(Duration::from_secs(minutes * 60));
    let ticks = scenario.ticks();

    let mut app = App::new();
    app.add_plugins((TaskPoolPlugin::default(), TimePlugin));
    app.add_plugins(MatchKernelPlugin::new(scenario));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK));

    for tick in 0..ticks {
        app.update();
        let elapsed_minutes = tick / (60 * 100);
        if tick % (10 * 60 * 100) != 0 || tick == 0 {
            continue;
        }
        report(&mut app, elapsed_minutes.into());
    }
    report(&mut app, minutes);
}

fn report(app: &mut App, minute: u64) {
    let mut bodies = app.world_mut().query::<(&FatigueState, &Player)>();
    let legs: Vec<f32> = bodies
        .iter(app.world())
        .map(|(fatigue, _)| fatigue.stamina)
        .collect();
    let mean = legs.iter().sum::<f32>() / legs.len() as f32;
    let worst = legs.iter().copied().fold(1.0_f32, f32::min);
    let spent = legs.iter().filter(|s| **s < 0.05).count();
    println!(
        "minuto {minute}: piernas {:.0}% de media, el más gastado al {:.0}%, {spent} de {} vacíos",
        mean * 100.0,
        worst * 100.0,
        legs.len()
    );
}
