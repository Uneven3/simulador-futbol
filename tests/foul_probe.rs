//! ¿Cuántas faltas ve el árbitro con el criterio de hoy?
//!
//! Tres semillas, que para un conteo que iba por 168 basta: lo que se mira es
//! el orden de magnitud contra las ~22 reales, no la tercera cifra.

use football_domain::{MatchTuning, Scenario};
use football_simulation::envelope::{EnvelopeReport, EnvelopeSpec};
use std::time::Duration;

#[test]
#[ignore = "medición, no una afirmación"]
fn how_many_fouls_does_the_referee_see() {
    let mut tuning = MatchTuning::default();
    tuning.refereeing.whistles_fouls = std::env::var("WHISTLE").is_ok();
    let spec = EnvelopeSpec {
        scenario: Scenario::kick_off()
            .for_duration(Duration::from_secs(10 * 60))
            .with_tuning(tuning),
        seeds: vec![0xC0FFEE, 0x7AB7, 0x309],
    };
    let report = EnvelopeReport::run(&spec);

    for m in &report.matches {
        println!(
            "seed {:#x}: {:.0} faltas/90  ({} en diez minutos)",
            m.seed,
            m.per_90(m.fouls as f32),
            m.fouls
        );
    }
    println!(
        "media: {:.0} faltas/90 (real ~22), {:.0} pitadas, \
         y el partido tuvo {:.0} tiros/90 y {:.0} goles/90",
        report.mean_of(|m| m.per_90(m.fouls as f32)),
        report.mean_of(|m| m.per_90(m.free_kicks as f32)),
        report.mean_of(|m| m.per_90(m.total_shots() as f32)),
        report.mean_of(|m| m.per_90(m.total_goals() as f32)),
    );
}
