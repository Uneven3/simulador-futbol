//! ¿Llega alguien a tener un cuerpo delante del balón?
//!
//! Dos veces seguidas la protección del balón dio delta cero en la envolvente.
//! Antes de hacerla más generosa hay que saber si la situación existe: cuántas
//! veces un rival está cerca del balón que lleva otro, y en cuántas de ellas
//! tiene el cuerpo del portador en medio.

use bevy::app::TaskPoolPlugin;
use bevy::prelude::*;
use bevy::time::{TimePlugin, TimeUpdateStrategy};
use football_domain::scenario::TICK;
use football_domain::{Ball, MatchState, Player, Position, Scenario};
use football_simulation::MatchKernelPlugin;
use football_simulation::ball_contest::shields_the_ball;
use std::time::Duration;

#[test]
#[ignore = "medición, no una afirmación"]
fn how_often_is_the_ball_actually_shielded() {
    let scenario = Scenario::kick_off().for_duration(Duration::from_secs(5 * 60));
    let ticks = scenario.ticks();

    let mut app = App::new();
    app.add_plugins((TaskPoolPlugin::default(), TimePlugin));
    app.add_plugins(MatchKernelPlugin::new(scenario));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK));

    let mut ticks_with_a_carrier = 0_u32;
    let mut opponents_within_two_metres = 0_u32;
    let mut opponents_shielded_off = 0_u32;

    for _ in 0..ticks {
        app.update();

        let Some(holder) = app.world().resource::<MatchState>().possession_player else {
            continue;
        };
        let mut balls = app.world_mut().query_filtered::<&Position, With<Ball>>();
        let Ok(ball) = balls.single(app.world()) else {
            continue;
        };
        let ball_spot = ball.on_pitch();

        let mut bodies = app.world_mut().query::<(&Position, &Player)>();
        let carrier = bodies
            .iter(app.world())
            .find(|(_, player)| player.id == holder)
            .map(|(position, _)| position.on_pitch());
        let Some(carrier) = carrier else {
            continue;
        };
        ticks_with_a_carrier += 1;

        for (position, player) in bodies.iter(app.world()) {
            if player.id.team == holder.team {
                continue;
            }
            let spot = position.on_pitch();
            if spot.distance(ball_spot) > 2.0 {
                continue;
            }
            opponents_within_two_metres += 1;
            if shields_the_ball(spot, carrier, ball_spot) {
                opponents_shielded_off += 1;
            }
        }
    }

    println!(
        "{ticks_with_a_carrier} ticks con portador; \
         {opponents_within_two_metres} veces un rival a menos de dos metros del balón, \
         y en {opponents_shielded_off} tenía el cuerpo del portador en medio"
    );
}
