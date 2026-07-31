//! ¿Cuántos golpeos se quedan sin dar?
//!
//! El compromiso solo cuesta algo si a veces muere sin llegar al balón. Si
//! ninguno muere, lo único que hizo fue mejorar la puntería, y eso no era lo
//! que se buscaba.

use bevy::app::TaskPoolPlugin;
use bevy::prelude::*;
use bevy::time::{TimePlugin, TimeUpdateStrategy};
use football_domain::Scenario;
use football_domain::scenario::TICK;
use football_simulation::MatchKernelPlugin;
use football_simulation::ball_release::ActionCommitment;
use football_simulation::diagnostics::MatchLedger;
use std::time::Duration;

#[test]
#[ignore = "medición, no una afirmación"]
fn how_many_strikes_die_before_the_boot_arrives() {
    let scenario = Scenario::kick_off().for_duration(Duration::from_secs(10 * 60));
    let ticks = scenario.ticks();

    let mut app = App::new();
    app.add_plugins((TaskPoolPlugin::default(), TimePlugin));
    app.add_plugins(MatchKernelPlugin::new(scenario));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK));

    let mut started = 0_u32;
    let mut committed_last_tick = 0;
    for _ in 0..ticks {
        app.update();
        let mut query = app.world_mut().query::<&ActionCommitment>();
        let alive = query.iter(app.world()).count();
        // cada subida del recuento es un golpeo que alguien acaba de empezar
        started += u32::try_from(alive.saturating_sub(committed_last_tick)).unwrap_or(0);
        committed_last_tick = alive;
    }

    let ledger = app.world().resource::<MatchLedger>();
    let struck = ledger.passes[football_domain::TeamId::Home]
        + ledger.passes[football_domain::TeamId::Away]
        + ledger.shots[football_domain::TeamId::Home]
        + ledger.shots[football_domain::TeamId::Away];
    println!(
        "{started} golpeos empezados, {struck} pases y tiros dados: \
         {} se quedaron sin dar",
        started.saturating_sub(struck)
    );
}
