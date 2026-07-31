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

use football_domain::{MatchTuning, Scenario, TeamId};
use football_simulation::envelope::{EnvelopeReport, EnvelopeSpec};
use std::time::Duration;

/// Seis semillas de cinco minutos: suficiente muestra para una dirección,
/// suficientemente barato para correrse al tocar el tuning.
fn situation(tuning: MatchTuning) -> EnvelopeSpec {
    EnvelopeSpec {
        scenario: Scenario::kick_off()
            .for_duration(Duration::from_secs(5 * 60))
            .with_tuning(tuning),
        seeds: vec![0xC0FFEE, 1, 7, 42, 1234, 99],
    }
}

/// Dos equipos idénticos terminan parejos.
///
/// Es la propiedad más barata y la que más cosas descarta: si el modelo tuviera
/// un sesgo de lado —una asimetría en la formación, en el saque, en el signo de
/// un `team_side`— aparecería aquí y en ninguna otra medida, porque todas las
/// demás suman los dos equipos.
#[test]
#[ignore]
fn two_identical_teams_finish_level() {
    let report = EnvelopeReport::run(&situation(MatchTuning::default()));

    let home: u32 = report.matches.iter().map(|m| m.goals[TeamId::Home]).sum();
    let away: u32 = report.matches.iter().map(|m| m.goals[TeamId::Away]).sum();
    let share = home as f32 / (home + away) as f32;
    println!("local {home} - {away} visitante  (cuota local {share:.3})");

    assert!(
        (0.35..=0.65).contains(&share),
        "un lado domina sin razón: {home}-{away} sobre {} partidos",
        report.matches.len()
    );
}

/// Subir el listón del tiro reduce los tiros.
///
/// Afirma que la perilla manda sobre lo que dice mandar. Si un día deja de ser
/// cierto, el umbral habrá dejado de ser el que decide disparar y habrá que
/// buscar quién lo decide de verdad.
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

/// El alcance del receptor está muerto: triplicarlo no cambia ni un bit.
///
/// Descubierto el 2026-07-30 al intentar afirmar lo contrario ("más alcance,
/// más pases completados"). `receiver_trap_reach` existe para sustituir las
/// animaciones de control del original —el receptor estira la pierna— y de él
/// depende que un pase se complete contra un marcador que está uno o dos metros
/// por detrás. Pero pasar de 1,1 m a 3,0 m produce partidos **idénticos**, así
/// que la rama que lo lee no se toma nunca en el momento que importa.
///
/// La sospecha: `update_possession_designation` borra `pass_target` cuando el
/// balón baja de 0,3 m/s, y los pases resueltos llegan agonizando al receptor —
/// justo antes de la disputa, que corre después. El propio código ya avisaba de
/// esta trampa con un umbral anterior. Sin confirmar.
///
/// Se afirma como está para que el día que se arregle este test falle y haya
/// que venir aquí. Es candidato número uno a explicar el 11 % de pases
/// completados, muy por encima de "falta percepción parcial".
#[test]
#[ignore]
fn the_receiver_reach_is_dead_code_in_practice() {
    let baseline = EnvelopeReport::run(&situation(MatchTuning::default()));

    let mut generous = MatchTuning::default();
    generous.contest.receiver_trap_reach = 3.0;
    let variant = EnvelopeReport::run(&situation(generous));

    let completion = |r: &EnvelopeReport| r.mean_of(|m| m.pass_completion());
    let goals = |r: &EnvelopeReport| r.mean_of(|m| m.total_goals() as f32);
    println!(
        "alcance 1,1 → 3,0 m: pases {:.4}% → {:.4}%, goles {:.2} → {:.2}",
        completion(&baseline) * 100.0,
        completion(&variant) * 100.0,
        goals(&baseline),
        goals(&variant),
    );

    assert_eq!(
        completion(&baseline),
        completion(&variant),
        "el alcance del receptor ha empezado a hacer algo: si es a propósito, \
         este test debe convertirse en la propiedad causal que intentaba ser"
    );
}

/// Hoy el balón no se roba: se recoge del suelo.
///
/// Medido el 2026-07-30: **22 robos contra 2015 recogidas** cada 90 minutos. El
/// mecanismo de entrada —con sus enfriamientos, su duelo y su protección del
/// cuerpo— decide el 1 % de los cambios de posesión; el otro 99 % es balón
/// suelto que alguien alcanza primero. Por eso `steal_cooldown` no mueve
/// ninguna métrica agregada: gobierna una centésima parte del partido.
///
/// Esto se afirma como está, no como debería ser: es la forma de que MVP 3
/// (motor y contacto) y MVP 4 (percepción) noten el día que lo cambien. Un
/// partido real reparte esto muchísimo más parejo.
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
