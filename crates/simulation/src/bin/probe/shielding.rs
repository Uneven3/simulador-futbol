//! ¿Llega alguien a tener un cuerpo delante del balón?
//!
//! Dos veces seguidas la protección del balón dio delta cero en la envolvente.
//! Antes de hacerla más generosa hay que saber si la situación existe: cuántas
//! veces un rival está cerca del balón que lleva otro, y en cuántas de ellas
//! tiene el cuerpo del portador en medio.

use bevy_ecs::prelude::With;
use football_domain::{Ball, MatchState, Player, Position, Scenario};
use football_simulation::ScenarioRunner;
use football_simulation::ball_contest::shields_the_ball;
use std::time::Duration;

pub fn run() {
    let scenario = Scenario::kick_off().for_duration(Duration::from_secs(5 * 60));
    let ticks = scenario.ticks();
    let mut runner = ScenarioRunner::headless(scenario);

    let mut ticks_with_a_carrier = 0_u32;
    let mut opponents_within_two_metres = 0_u32;
    let mut opponents_shielded_off = 0_u32;

    for _ in 0..ticks {
        runner.advance();
        let world = runner.world_mut();

        let Some(holder) = world.resource::<MatchState>().possession_player else {
            continue;
        };
        let mut balls = world.query_filtered::<&Position, With<Ball>>();
        let Ok(ball) = balls.single(world) else {
            continue;
        };
        let ball_spot = ball.on_pitch();

        let mut bodies = world.query::<(&Position, &Player)>();
        let carrier = bodies
            .iter(world)
            .find(|(_, player)| player.id == holder)
            .map(|(position, _)| position.on_pitch());
        let Some(carrier) = carrier else {
            continue;
        };
        ticks_with_a_carrier += 1;

        for (position, player) in bodies.iter(world) {
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

    crate::record(
        "shielding",
        &[
            ("ticks_con_portador", ticks_with_a_carrier as f32),
            ("rivales_a_dos_metros", opponents_within_two_metres as f32),
            ("con_el_cuerpo_en_medio", opponents_shielded_off as f32),
            (
                "protegido_pct",
                100.0 * opponents_shielded_off as f32 / opponents_within_two_metres.max(1) as f32,
            ),
        ],
    );
}
