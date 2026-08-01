//! ¿Cuánta compañía tiene el atacante cerca del área?
//!
//! Con el peaje del retroceso, la proporción de tiros que acaban en gol saltó
//! del 25 % al 57 %, y el diagnóstico fue "el atacante llega solo". Esto lo
//! comprueba en vez de suponerlo: `NO_TURNING_COST=1` quita el peaje y deja
//! todo lo demás igual.

use bevy_math::Vec2;
use football_domain::{MatchState, MatchTuning, Player, Position, Scenario};
use football_simulation::ScenarioRunner;
use std::time::Duration;

pub fn run() {
    let mut chances = 0_u32;
    let mut total_company = 0.0_f32;
    let mut unmarked = 0_u32;
    let mut shots = 0_u32;
    let mut total_shot_range = 0.0_f32;
    let mut on_target = 0_u32;
    let mut goals = 0_u32;

    // un tiro por partido de diez minutos no dice nada de una media: la muestra
    // son las diez semillas de la envolvente
    for seed in (0..10).map(|i| 0xC0FFEE + i * 7919) {
        let sample = one_match(seed);
        chances += sample.0;
        total_company += sample.1;
        unmarked += sample.2;
        shots += sample.3;
        total_shot_range += sample.4;
        on_target += sample.5;
        goals += sample.6;
    }

    println!(
        "{chances} ticks con el balón a menos de 20 m de la portería contraria; \
         rival más cercano a {:.1} m de media, y en el {:.0} % de ellos a más de cinco.\n\
         {shots} tiros, desde {:.1} m de media.\n\
         {on_target} a puerta y {goals} goles: el portero para el {:.0} %",
        total_company / chances as f32,
        100.0 * f64::from(unmarked) / f64::from(chances),
        total_shot_range / shots as f32,
        100.0 * (1.0 - f64::from(goals) / f64::from(on_target))
    );

    crate::record(
        "defending",
        &[
            ("ocasiones", chances as f32),
            ("compania_media_m", total_company / chances.max(1) as f32),
            (
                "sin_marca_pct",
                100.0 * unmarked as f32 / chances.max(1) as f32,
            ),
            ("tiros", shots as f32),
            (
                "distancia_media_tiro_m",
                total_shot_range / shots.max(1) as f32,
            ),
            ("a_puerta", on_target as f32),
            ("goles", goals as f32),
            (
                "paradas_pct",
                100.0 * (1.0 - goals as f32 / on_target.max(1) as f32),
            ),
        ],
    );
}

type MatchSample = (u32, f32, u32, u32, f32, u32, u32);

fn one_match(seed: u32) -> MatchSample {
    let mut tuning = MatchTuning::default();
    if std::env::var("NO_TURNING_COST").is_ok() {
        tuning.turning.backpedal_pace = 1.0;
        tuning.turning.sideways_pace = 1.0;
    }
    let scenario = Scenario {
        seed,
        ..Scenario::kick_off()
            .for_duration(Duration::from_secs(10 * 60))
            .with_tuning(tuning)
    };
    let ticks = scenario.ticks();
    let mut runner = ScenarioRunner::headless(scenario);

    let mut chances = 0_u32;
    let mut total_company = 0.0_f32;
    let mut unmarked = 0_u32;
    let mut shots = 0_u32;
    let mut total_shot_range = 0.0_f32;
    let mut last_shot_count = 0;
    let mut last_holder_spot = None;

    for _ in 0..ticks {
        runner.advance();
        let world = runner.world_mut();

        // un tiro más en el libro: el disparo salió de donde estaba su dueño el
        // tick anterior, que es lo último que se supo de él
        let ledger = world.resource::<football_simulation::diagnostics::MatchLedger>();
        let shot_count = ledger.shots[football_domain::TeamId::Home]
            + ledger.shots[football_domain::TeamId::Away];
        if shot_count > last_shot_count
            && let Some((spot, goal_x)) = last_holder_spot
        {
            shots += 1;
            total_shot_range += Vec2::new(goal_x, 0.0).distance(spot);
        }
        last_shot_count = shot_count;

        let state = world.resource::<MatchState>();
        let (Some(holder), sides) = (state.possession_player, state.sides) else {
            last_holder_spot = None;
            continue;
        };
        let attacking_x = sides.attacking_x(holder.team) * 55.0;

        let mut bodies = world.query::<(&Position, &Player)>();
        let spot = bodies
            .iter(world)
            .find(|(_, player)| player.id == holder)
            .map(|(position, _)| position.on_pitch());
        let Some(spot) = spot else {
            last_holder_spot = None;
            continue;
        };
        last_holder_spot = Some((spot, attacking_x));
        if (spot.x - attacking_x).abs() > 20.0 {
            continue;
        }

        // `reduce` y no `fold(f32::MAX, ..)`: "no había ningún rival" es un caso,
        // no un valor centinela que luego haya que comparar contra un flotante.
        let nearest = bodies
            .iter(world)
            .filter(|(_, player)| player.id.team != holder.team)
            .map(|(position, _)| position.on_pitch().distance(spot))
            .reduce(f32::min);
        let Some(nearest) = nearest else {
            continue;
        };
        chances += 1;
        total_company += nearest;
        if nearest > 5.0 {
            unmarked += 1;
        }
    }

    let ledger = runner
        .world_mut()
        .resource::<football_simulation::diagnostics::MatchLedger>();
    let on_target = ledger.shots_on_target[football_domain::TeamId::Home]
        + ledger.shots_on_target[football_domain::TeamId::Away];
    let goals =
        ledger.goals[football_domain::TeamId::Home] + ledger.goals[football_domain::TeamId::Away];
    (
        chances,
        total_company,
        unmarked,
        shots,
        total_shot_range,
        on_target,
        goals,
    )
}
