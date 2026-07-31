//! Propiedades causales: dirección de efecto, no valores.
//!
//! Son la forma de test que resiste en un modelo determinista pero caótico. Un
//! marcador no sobrevive a una perturbación de un milisegundo; "subir este
//! umbral reduce los disparos" sobrevive a cualquier refactor que no cambie el
//! modelo (`docs/VALIDATION.md`).
//!
//! Y sobreviven a que el modelo aún no sea realista: que el umbral de tiro
//! mande sobre los tiros es cierto con 51 goles por partido y con 2,7. Por eso
//! estas propiedades se pueden afirmar hoy y la calibración espera a que
//! existan portero, faltas, fatiga y percepción (`docs/NORTE.md`).
//!
//! Corren explícitamente, porque cada una simula varios partidos:
//! `cargo test --release -p gameplayfootball_simulation --test causal_properties -- --ignored --nocapture`

use football_domain::{ByTeam, MatchTuning, Scenario, TeamId};
use football_simulation::envelope::{EnvelopeReport, EnvelopeSpec, MatchSummary};
use std::time::Duration;

/// Doce semillas de cinco minutos: suficiente muestra para una dirección,
/// suficientemente barato para correrse al tocar el tuning.
fn situation(tuning: MatchTuning) -> EnvelopeSpec {
    EnvelopeSpec {
        scenario: Scenario::kick_off()
            .for_duration(Duration::from_secs(5 * 60))
            .with_tuning(tuning),
        seeds: vec![
            0xC0FFEE, 1, 7, 42, 1234, 99, 2718, 31415, 5, 777, 8191, 6553,
        ],
    }
}

/// Dos equipos idénticos terminan parejos: un sesgo de lado solo aparece aquí.
/// Banda estrecha solo sobre la posesión, que promedia el tick; pases y tiros
/// llegan en rachas, y sobre ellos tres sigma atrapa un sesgo grosero y nada más.
#[test]
#[ignore]
fn two_identical_teams_finish_level() {
    let report = EnvelopeReport::run(&situation(MatchTuning::default()));

    let share_of = |of: fn(&MatchSummary) -> ByTeam<u32>| {
        let home: u32 = report.matches.iter().map(|m| of(m)[TeamId::Home]).sum();
        let away: u32 = report.matches.iter().map(|m| of(m)[TeamId::Away]).sum();
        (home, away, home as f32 / (home + away) as f32)
    };

    let (home_goals, away_goals, goal_share) = share_of(|m| m.goals);
    let (home_shots, away_shots, shot_share) = share_of(|m| m.shots);
    let (home_passes, away_passes, pass_share) = share_of(|m| m.passes);
    let possession = report.mean_of(|m| m.possession[TeamId::Home]);
    println!(
        "goles {home_goals}-{away_goals} ({goal_share:.3}), \
         tiros {home_shots}-{away_shots} ({shot_share:.3}), \
         pases {home_passes}-{away_passes} ({pass_share:.3}), \
         posesión {possession:.3}"
    );

    assert!(
        (0.45..=0.55).contains(&possession),
        "un lado tiene el balón y el otro lo persigue: posesión {possession:.3}"
    );
    assert!(
        (0.40..=0.60).contains(&pass_share),
        "un lado juega y el otro mira: {home_passes}-{away_passes} pases"
    );
    assert!(
        (0.32..=0.68).contains(&shot_share),
        "un lado llega al área y el otro no: {home_shots}-{away_shots} tiros"
    );
    assert!(
        (0.19..=0.81).contains(&goal_share),
        "un lado domina sin razón: {home_goals}-{away_goals} sobre {} partidos",
        report.matches.len()
    );
}

/// Subir el listón del tiro reduce los tiros: la perilla manda sobre lo que dice
/// mandar. Si deja de ser cierto, habrá que buscar quién decide disparar.
#[test]
#[ignore]
fn a_higher_shooting_gate_produces_fewer_shots() {
    let baseline = EnvelopeReport::run(&situation(MatchTuning::default()));

    let mut strict = MatchTuning::default();
    strict.shooting.ideal_position_gate = 0.45;
    let variant = EnvelopeReport::run(&situation(strict));

    let shots = |r: &EnvelopeReport| r.mean_of(|m| m.per_90(m.total_shots() as f32));
    println!(
        "tiros/90: base {:.0} → umbral estricto {:.0}",
        shots(&baseline),
        shots(&variant)
    );

    assert!(
        shots(&variant) < shots(&baseline),
        "subir el umbral no redujo los disparos: {:.0} vs {:.0}",
        shots(&variant),
        shots(&baseline)
    );
}

/// Un receptor que alcanza más lejos completa más pases: es el sustituto de las
/// animaciones de control del original, y de él depende que un pase se complete
/// contra el marcador que llega un metro por detrás.
#[test]
#[ignore]
fn a_longer_receiver_reach_completes_more_passes() {
    let baseline = EnvelopeReport::run(&situation(MatchTuning::default()));

    let mut generous = MatchTuning::default();
    generous.contest.receiver_trap_reach = 3.0;
    let variant = EnvelopeReport::run(&situation(generous));

    let completion = |r: &EnvelopeReport| r.mean_of(|m| m.pass_completion());
    println!(
        "alcance 1,1 → 3,0 m: pases completados {:.1}% → {:.1}%",
        completion(&baseline) * 100.0,
        completion(&variant) * 100.0,
    );

    assert!(
        completion(&variant) > completion(&baseline),
        "alargar el alcance del receptor no completó más pases: {:.1}% vs {:.1}%",
        completion(&variant) * 100.0,
        completion(&baseline) * 100.0
    );
}

/// Hoy el balón no se roba: se recoge del suelo, y la entrada decide una
/// cincuentava parte de los cambios de posesión. Se afirma como está para que el
/// día que eso cambie haya que venir aquí a decidirlo.
#[test]
#[ignore]
fn the_ball_changes_hands_by_being_dropped_not_taken() {
    let report = EnvelopeReport::run(&situation(MatchTuning::default()));

    let tackles: u32 = report.matches.iter().map(|m| m.tackles).sum();
    let loose: u32 = report.matches.iter().map(|m| m.loose_balls).sum();
    let tackle_share = tackles as f32 / (tackles + loose) as f32;
    println!(
        "robos {tackles}, recogidas {loose} (robos {:.1}%)",
        tackle_share * 100.0
    );

    assert!(
        tackle_share < 0.05,
        "los robos han dejado de ser marginales ({:.1}%): el modelo cambió, \
         actualiza esta caracterización",
        tackle_share * 100.0
    );
}
