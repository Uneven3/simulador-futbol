//! ¿Qué sabe cada jugador del campo?
//!
//! A cuánta gente llega a conocer un jugador, cuánto envejece esa información y
//! cuánto se aparta su balón creído del real cuando deja de mirarlo.

use bevy_ecs::prelude::{With, Without};
use bevy_math::Vec2;
use bevy_time::Time;
use football_domain::{
    Ball, Looking, MatchTuning, ObservationMemory, Player, PlayerId, Position, Scenario, Vision,
    can_see, hidden_by,
};
use football_simulation::ScenarioRunner;
use football_simulation::perception::Beliefs;
use std::time::Duration;

/// Cada cuántos ticks se pregunta quién estorba a quién. El estorbo dura
/// décimas: muestrear más espaciado mediría otra cosa.
const SHADOW_SAMPLE: u32 = 10;

pub fn run() {
    let scenario = Scenario::kick_off().for_duration(Duration::from_secs(60));
    let ticks = scenario.ticks();
    let mut runner = ScenarioRunner::headless(scenario);

    // Lo que la corrida no puede decir: cuánto tapa la gente. El error del balón
    // al final del minuto es una trayectoria, no una medida del mecanismo; esto
    // se pregunta sobre la geometría de cada instante y no depende de dónde
    // acabó el partido.
    let mut in_cone = 0_u64;
    let mut shadowed = 0_u64;
    let mut glimpsed = 0_u64;
    let mut ball_in_cone = 0_u64;
    let mut ball_shadowed = 0_u64;

    for tick in 0..ticks {
        runner.advance();
        if tick % SHADOW_SAMPLE != 0 {
            continue;
        }
        let (cone, hidden, part, ball_cone, ball_hidden) = who_is_in_the_way(runner.world_mut());
        in_cone += cone;
        shadowed += hidden;
        glimpsed += part;
        ball_in_cone += ball_cone;
        ball_shadowed += ball_hidden;
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
        if memory.ball().is_some() {
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

    let shadowed_pct = 100.0 * shadowed as f32 / in_cone.max(1) as f32;
    let glimpsed_pct = 100.0 * glimpsed as f32 / in_cone.max(1) as f32;
    let ball_shadowed_pct = 100.0 * ball_shadowed as f32 / ball_in_cone.max(1) as f32;
    println!(
        "de todo lo que cae dentro del cono, el {shadowed_pct:.0} % está tapado del todo \
         y el {glimpsed_pct:.0} % se ve a medias; el balón se pierde detrás de alguien \
         el {ball_shadowed_pct:.0} % de las veces que cae en un cono"
    );

    crate::record(
        "perception",
        &[
            ("error_balon_medio_m", mean_ball_error),
            ("error_balon_peor_m", worst_ball_error),
            ("duda_balon_media_m", mean_ball_doubt),
            ("tapados_pct", shadowed_pct),
            ("entrevistos_pct", glimpsed_pct),
            ("balon_tapado_pct", ball_shadowed_pct),
            ("companeros_conocidos", known as f32 / people.max(1) as f32),
            (
                "antiguedad_media_s",
                stale_total / stale_count.max(1) as f32,
            ),
            ("con_el_balon_en_la_cabeza", ball_seen as f32),
        ],
    );
}

/// Cuántos cuerpos caen dentro de un cono, cuántos están tapados del todo por un
/// tercero y cuántos se ven a medias, sumado sobre los veintidós; y del balón,
/// cuántas veces cae en un cono y cuántas está tapado del todo.
fn who_is_in_the_way(world: &mut bevy_ecs::world::World) -> (u64, u64, u64, u64, u64) {
    let ball = {
        let mut ball_query = world.query_filtered::<&Position, (With<Ball>, Without<Player>)>();
        ball_query
            .iter(world)
            .next()
            .map(|position| position.on_pitch())
    };
    let crowd: Vec<(PlayerId, Vec2)> = {
        let mut bodies = world.query::<(&Position, &Player)>();
        bodies
            .iter(world)
            .map(|(position, player)| (player.id, position.on_pitch()))
            .collect()
    };

    let mut in_cone = 0;
    let mut shadowed = 0;
    let mut glimpsed = 0;
    let mut ball_in_cone = 0;
    let mut ball_shadowed = 0;

    let mut watchers = world.query::<(&Position, &Looking, &Player, &Vision)>();
    for (position, looking, watcher, vision) in watchers.iter(world) {
        let eyes = position.on_pitch();
        for (id, spot) in &crowd {
            if *id == watcher.id || !can_see(eyes, looking.0, *spot, vision) {
                continue;
            }
            in_cone += 1;
            match is_hidden(eyes, *spot, &crowd, watcher.id, Some(*id)) {
                hidden if hidden >= 1.0 => shadowed += 1,
                hidden if hidden > 0.0 => glimpsed += 1,
                _ => {}
            }
        }
        if let Some(spot) = ball
            && can_see(eyes, looking.0, spot, vision)
        {
            ball_in_cone += 1;
            if is_hidden(eyes, spot, &crowd, watcher.id, None) >= 1.0 {
                ball_shadowed += 1;
            }
        }
    }

    (in_cone, shadowed, glimpsed, ball_in_cone, ball_shadowed)
}

fn is_hidden(
    eyes: Vec2,
    target: Vec2,
    crowd: &[(PlayerId, Vec2)],
    watcher: PlayerId,
    seen: Option<PlayerId>,
) -> f32 {
    crowd
        .iter()
        .filter(|(id, _)| *id != watcher && Some(*id) != seen)
        .map(|(_, spot)| hidden_by(eyes, target, *spot))
        .fold(0.0, f32::max)
}
