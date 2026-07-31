//! ¿El sesgo es de lado o de rol?
//!
//! Con `KICKOFF_AWAY` saca el visitante en vez del local. Si las cifras se
//! reflejan, lo que está torcido no es el campo sino el papel de quien saca —y
//! eso no lo distingue ninguna medida agregada, porque suman los dos equipos.

use football_domain::scenario::PlayState;
use football_domain::{MatchTuning, Scenario, SetPiece, TeamId};
use football_simulation::envelope::{EnvelopeReport, EnvelopeSpec};
use std::time::Duration;

#[test]
#[ignore = "medición, no una afirmación"]
fn how_lopsided_is_it_really() {
    let mut scenario = Scenario::kick_off()
        .for_duration(Duration::from_secs(5 * 60))
        .with_tuning(MatchTuning::default());
    if std::env::var("KICKOFF_AWAY").is_ok() {
        scenario.play_state = PlayState::AwaitingRestart {
            set_piece: SetPiece::KickOff,
            team: TeamId::Away,
            delay: Duration::from_secs(2),
        };
    }
    let spec = EnvelopeSpec {
        scenario,
        seeds: (0..12).map(|i| 0xC0FFEE + i * 7919).collect(),
    };
    let report = EnvelopeReport::run(&spec);
    let home: u32 = report.matches.iter().map(|m| m.goals[TeamId::Home]).sum();
    let away: u32 = report.matches.iter().map(|m| m.goals[TeamId::Away]).sum();
    let wins = report
        .matches
        .iter()
        .filter(|m| m.goals[TeamId::Home] > m.goals[TeamId::Away])
        .count();
    let losses = report
        .matches
        .iter()
        .filter(|m| m.goals[TeamId::Home] < m.goals[TeamId::Away])
        .count();
    let shots_home: u32 = report.matches.iter().map(|m| m.shots[TeamId::Home]).sum();
    let shots_away: u32 = report.matches.iter().map(|m| m.shots[TeamId::Away]).sum();
    let passes_home: u32 = report.matches.iter().map(|m| m.passes[TeamId::Home]).sum();
    let passes_away: u32 = report.matches.iter().map(|m| m.passes[TeamId::Away]).sum();
    let possession: f32 = report
        .matches
        .iter()
        .map(|m| m.possession[TeamId::Home])
        .sum::<f32>()
        / report.matches.len() as f32;
    println!(
        "{} partidos\n  goles     {home:>5} - {away:<5} (cuota local {:.3})\n  \
         tiros     {shots_home:>5} - {shots_away:<5}\n  \
         pases     {passes_home:>5} - {passes_away:<5}\n  \
         posesión  {:.3} local\n  victorias {wins} - {losses}",
        report.matches.len(),
        home as f32 / (home + away) as f32,
        possession
    );
}
