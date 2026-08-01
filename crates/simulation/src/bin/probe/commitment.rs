//! ¿Cuántos golpeos se quedan sin dar?
//!
//! El compromiso solo cuesta algo si a veces muere sin llegar al balón. Si
//! ninguno muere, lo único que hizo fue mejorar la puntería, y eso no era lo
//! que se buscaba.

use football_domain::Scenario;
use football_simulation::ScenarioRunner;
use football_simulation::ball_release::ActionCommitment;
use football_simulation::diagnostics::MatchLedger;
use std::time::Duration;

pub fn run() {
    let scenario = Scenario::kick_off().for_duration(Duration::from_secs(10 * 60));
    let ticks = scenario.ticks();
    let mut runner = ScenarioRunner::headless(scenario);

    let mut started = 0_u32;
    let mut committed_last_tick = 0;
    for _ in 0..ticks {
        runner.advance();
        let world = runner.world_mut();
        let mut query = world.query::<&ActionCommitment>();
        let alive = query.iter(world).count();
        // cada subida del recuento es un golpeo que alguien acaba de empezar
        started += u32::try_from(alive.saturating_sub(committed_last_tick)).unwrap_or(0);
        committed_last_tick = alive;
    }

    let ledger = runner.world_mut().resource::<MatchLedger>();
    let struck = ledger.passes[football_domain::TeamId::Home]
        + ledger.passes[football_domain::TeamId::Away]
        + ledger.shots[football_domain::TeamId::Home]
        + ledger.shots[football_domain::TeamId::Away];
    println!(
        "{started} golpeos empezados, {struck} pases y tiros dados: \
         {} se quedaron sin dar",
        started.saturating_sub(struck)
    );

    crate::record(
        "commitment",
        &[
            ("golpeos_empezados", started as f32),
            ("golpeos_dados", struck as f32),
            ("golpeos_sin_dar", started.saturating_sub(struck) as f32),
        ],
    );
}
