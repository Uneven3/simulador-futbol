use football_domain::Scenario;
use gameplayfootball::ScenarioRunner;
use std::time::{Duration, Instant};

#[test]
#[ignore = "medición de rendimiento, no una afirmación"]
fn how_fast_can_a_match_run() {
    let window = Duration::from_secs(60);
    let scenario = Scenario::kick_off().for_duration(window);
    let started = Instant::now();
    let runner = ScenarioRunner::headless(scenario);
    let outcome = runner.run();
    let wall = started.elapsed();
    println!(
        "{} ticks / {:.1} s simulados en {:.2} s de pared = {:.0}x tiempo real",
        outcome.ticks_simulated,
        window.as_secs_f32(),
        wall.as_secs_f32(),
        window.as_secs_f32() / wall.as_secs_f32()
    );
}
