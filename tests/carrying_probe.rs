//! ¿Se orbita el balón, y se pierde sin que nadie apriete?
//!
//! Los dos defectos que se ven mirando y que ninguna métrica de resultado
//! delata: un portador que gira alrededor del balón en vez de llevarlo, y
//! pérdidas sin un rival cerca.

use bevy::app::TaskPoolPlugin;
use bevy::prelude::*;
use bevy::time::{TimePlugin, TimeUpdateStrategy};
use football_domain::scenario::TICK;
use football_domain::{Ball, MatchState, Player, Position, Scenario, Velocity};
use football_simulation::MatchKernelPlugin;
use std::time::Duration;

/// Un rival a más de esto no está apretando a nadie.
const NO_PRESSURE: f32 = 5.0;

#[test]
#[ignore = "medición, no una afirmación"]
fn how_badly_is_the_ball_carried() {
    let mut carried = 0_u32;
    let mut orbiting = 0_u32;
    let mut at_the_foot = 0_u32;
    let mut losses = 0_u32;
    let mut losses_unpressed = 0_u32;

    for seed in (0..6).map(|i| 0xC0FFEE + i * 7919) {
        let scenario = Scenario {
            seed,
            ..Scenario::kick_off().for_duration(Duration::from_secs(5 * 60))
        };
        let ticks = scenario.ticks();

        let mut app = App::new();
        app.add_plugins((TaskPoolPlugin::default(), TimePlugin));
        app.add_plugins(MatchKernelPlugin::new(scenario));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK));

        let mut had: Option<football_domain::PlayerId> = None;
        for _ in 0..ticks {
            app.update();

            let holder = app.world().resource::<MatchState>().possession_player;
            let mut balls = app
                .world_mut()
                .query_filtered::<&Position, (With<Ball>, Without<Player>)>();
            let Ok(ball) = balls.single(app.world()) else {
                continue;
            };
            let ball_spot = ball.on_pitch();

            if let (Some(lost), None) = (had, holder) {
                losses += 1;
                let mut bodies = app.world_mut().query::<(&Position, &Player)>();
                let nearest_rival = bodies
                    .iter(app.world())
                    .filter(|(_, player)| player.id.team != lost.team)
                    .map(|(position, _)| position.on_pitch().distance(ball_spot))
                    .fold(f32::MAX, f32::min);
                if nearest_rival > NO_PRESSURE {
                    losses_unpressed += 1;
                }
            }
            had = holder;

            let Some(holder) = holder else { continue };
            let mut bodies = app.world_mut().query::<(&Position, &Velocity, &Player)>();
            let Some((spot, running)) = bodies
                .iter(app.world())
                .find(|(_, _, player)| player.id == holder)
                .map(|(position, velocity, _)| (position.on_pitch(), velocity.0.truncate()))
            else {
                continue;
            };
            carried += 1;
            let apart = spot.distance(ball_spot);
            if apart < 0.7 {
                at_the_foot += 1;
                continue;
            }
            // orbitar es, con el balón ya separado, correr de través en vez de
            // hacia él: pegado al pie ir de lado es geometría, no defecto
            if let (Ok(heading), Ok(to_ball)) = (Dir2::new(running), Dir2::new(ball_spot - spot))
                && heading.dot(*to_ball) < 0.5
            {
                orbiting += 1;
            }
        }
    }

    println!(
        "con el balón {carried} ticks: {:.0} % con el balón en el pie, \
         y con él suelto, {:.0} % corriendo de través en vez de a por él.\n\
         {losses} pérdidas, {losses_unpressed} de ellas sin un rival a cinco metros ({:.0} %)",
        100.0 * f64::from(at_the_foot) / f64::from(carried),
        100.0 * f64::from(orbiting) / f64::from(carried),
        100.0 * f64::from(losses_unpressed) / f64::from(losses.max(1))
    );
}
