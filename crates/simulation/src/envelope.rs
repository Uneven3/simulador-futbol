//! N partidos, reportados como distribución.
//!
//! Una corrida es una trayectoria, no una métrica: determinista pero caótica,
//! una perturbación de un milisegundo da otro partido sin que el modelo haya
//! cambiado. Solo la envolvente sobre varias semillas se puede comparar, y por
//! la distribución antes que por la media: un modelo puede acertar la media y
//! no parecerse a un partido (`docs/VALIDATION.md`).

use bevy_app::TaskPoolPlugin;
use bevy_app::prelude::*;
use bevy_time::{TimePlugin, TimeUpdateStrategy};
use football_domain::math::rounded_count;
use football_domain::scenario::TICK;
use football_domain::tuning::TuningVersion;
use football_domain::{ByTeam, MatchTuning, Scenario, TeamId};
use std::fmt::Write as _;
use std::time::Duration;

use crate::MatchKernelPlugin;
use crate::diagnostics::MatchLedger;

/// Goles por partido de un partido real, para tener contra qué comparar
/// (`docs/VALIDATION.md`). Es la referencia externa más barata que existe.
pub const REAL_GOALS_PER_MATCH: f32 = 2.7;

/// Filas del histograma antes de agrupar la cola en un "N+".
const HISTOGRAM_ROWS: u32 = 8;

/// Qué situación se corre, y bajo cuántas semillas.
#[derive(Debug, Clone)]
pub struct EnvelopeSpec {
    /// La situación, con su tuning y su ventana. La semilla la pone el barrido.
    pub scenario: Scenario,
    pub seeds: Vec<u32>,
}

impl EnvelopeSpec {
    /// Las diez semillas de diez minutos: barata, y la forma válida de comparar
    /// dos builds. No sirve para comparar contra el fútbol real — diez minutos
    /// no son un partido.
    pub fn comparing_builds() -> Self {
        Self {
            scenario: Scenario::kick_off().for_duration(Duration::from_secs(10 * 60)),
            seeds: vec![0xC0FFEE, 1, 7, 42, 1234, 99, 2718, 31415, 5, 777],
        }
    }

    /// `matches` partidos completos, que es lo único comparable con la
    /// distribución real de goles. Cuesta unos dos minutos por partido.
    pub fn against_the_real_game(matches: u32) -> Self {
        Self {
            scenario: Scenario::kick_off(),
            seeds: (0..matches).map(|i| 0xC0FFEE + i * 7919).collect(),
        }
    }

    pub fn with_tuning(mut self, tuning: MatchTuning) -> Self {
        self.scenario = self.scenario.with_tuning(tuning);
        self
    }
}

/// Lo que dejó un partido. Todo sale del ledger: nadie recuenta por su cuenta.
#[derive(Debug, Clone)]
pub struct MatchSummary {
    pub seed: u32,
    pub goals: ByTeam<u32>,
    pub shots: ByTeam<u32>,
    pub shots_on_target: ByTeam<u32>,
    pub passes: ByTeam<u32>,
    pub passes_completed: ByTeam<u32>,
    pub possession: ByTeam<f32>,
    pub possession_changes_per_minute: f32,
    /// Cómo cambió de manos: quitado a un rival, o recogido del suelo.
    pub tackles: u32,
    pub loose_balls: u32,
    pub fouls: u32,
    /// La racha de posesión más larga y cuántos jugadores distintos tocaron el
    /// balón: las dos señales de que el partido degeneró en un duelo entre dos.
    pub longest_spell: Duration,
    pub distinct_touchers: usize,
    pub elapsed: Duration,
}

impl MatchSummary {
    pub fn total_goals(&self) -> u32 {
        self.goals[TeamId::Home] + self.goals[TeamId::Away]
    }

    pub fn total_shots(&self) -> u32 {
        self.shots[TeamId::Home] + self.shots[TeamId::Away]
    }

    /// Lo que ese número sería en noventa minutos **de juego**. Una ventana de
    /// diez minutos no se puede comparar con nada real sin esto.
    pub fn per_90(&self, value: f32) -> f32 {
        let minutes = self.elapsed.as_secs_f32() / 60.0;
        if minutes <= 0.0 {
            0.0
        } else {
            value * 90.0 / minutes
        }
    }

    /// Fracción de pases que llegaron, sumando ambos equipos.
    pub fn pass_completion(&self) -> f32 {
        let attempted = (self.passes[TeamId::Home] + self.passes[TeamId::Away]) as f32;
        if attempted <= 0.0 {
            return 0.0;
        }
        (self.passes_completed[TeamId::Home] + self.passes_completed[TeamId::Away]) as f32
            / attempted
    }
}

/// El resultado de un barrido, con el tuning que lo produjo.
#[derive(Debug, Clone)]
pub struct EnvelopeReport {
    pub tuning: TuningVersion,
    pub window: Duration,
    pub matches: Vec<MatchSummary>,
}

impl EnvelopeReport {
    /// Corre la especificación entera. Cada partido es una `App` nueva: dos
    /// corridas no pueden compartir estado o dejarían de ser reproducibles.
    pub fn run(spec: &EnvelopeSpec) -> Self {
        let matches = spec
            .seeds
            .iter()
            .map(|seed| run_one_match(&spec.scenario, *seed))
            .collect();
        Self {
            tuning: spec.scenario.tuning.version,
            window: spec.scenario.window,
            matches,
        }
    }

    /// Goles por partido, llevados a noventa minutos.
    pub fn goals_per_match(&self) -> Distribution {
        Distribution::of(
            self.matches
                .iter()
                .map(|m| rounded_count(m.per_90(m.total_goals() as f32))),
        )
    }

    /// Goles por equipo y partido, que es la forma en que se publica la
    /// referencia real (~1,35, casi Poisson).
    pub fn goals_per_team(&self) -> Distribution {
        Distribution::of(self.matches.iter().flat_map(|m| {
            TeamId::BOTH
                .into_iter()
                .map(move |team| rounded_count(m.per_90(m.goals[team] as f32)))
        }))
    }

    pub fn mean_of(&self, value: impl Fn(&MatchSummary) -> f32) -> f32 {
        if self.matches.is_empty() {
            return 0.0;
        }
        self.matches.iter().map(&value).sum::<f32>() / self.matches.len() as f32
    }

    /// El informe completo: la tabla por partido, los agregados y el histograma
    /// contra la Poisson real.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let minutes = self.window.as_secs_f32() / 60.0;
        let _ = writeln!(
            out,
            "=== {} partidos de {minutes:.0} min · tuning {:?} ===",
            self.matches.len(),
            self.tuning
        );
        for m in &self.matches {
            let _ = writeln!(
                out,
                "seed {:#x}: {}-{}  ({:.0} goles/90)  {:.0} tiros ({:.0} a puerta)  \
                 posesión {:.0}/{:.0}  {:.0}% pases  {:.1} cambios/min  \
                 racha {:.1}s  {} tocadores",
                m.seed,
                m.goals[TeamId::Home],
                m.goals[TeamId::Away],
                m.per_90(m.total_goals() as f32),
                m.per_90(m.total_shots() as f32),
                m.per_90(
                    (m.shots_on_target[TeamId::Home] + m.shots_on_target[TeamId::Away]) as f32
                ),
                m.possession[TeamId::Home] * 100.0,
                m.possession[TeamId::Away] * 100.0,
                m.pass_completion() * 100.0,
                m.possession_changes_per_minute,
                m.longest_spell.as_secs_f32(),
                m.distinct_touchers,
            );
        }

        let goals = self.goals_per_match();
        let _ = writeln!(
            out,
            "\ngoles/90min: media {:.1} (real {REAL_GOALS_PER_MATCH})  rango {}-{}",
            goals.mean(),
            goals.min(),
            goals.max()
        );
        let _ = writeln!(
            out,
            "tiros/90min: {:.0} ({:.0}% a puerta, real ~33%)   \
             pases/90min: {:.0} ({:.0}% completados, real ~80%)\n\
             el balón cambia de manos: {:.0} robos/90min, {:.0} recogidas/90min\n\
             faltas señaladas: {:.0}/90min (real ~22)",
            self.mean_of(|m| m.per_90(m.total_shots() as f32)),
            self.mean_of(|m| {
                let on_target =
                    (m.shots_on_target[TeamId::Home] + m.shots_on_target[TeamId::Away]) as f32;
                let shots = m.total_shots() as f32;
                if shots > 0.0 { on_target / shots } else { 0.0 }
            }) * 100.0,
            self.mean_of(|m| m.per_90((m.passes[TeamId::Home] + m.passes[TeamId::Away]) as f32)),
            self.mean_of(|m| m.pass_completion()) * 100.0,
            self.mean_of(|m| m.per_90(m.tackles as f32)),
            self.mean_of(|m| m.per_90(m.loose_balls as f32)),
            self.mean_of(|m| m.per_90(m.fouls as f32)),
        );

        // El histograma solo dice algo sobre partidos completos: escalar diez
        // minutos por nueve conserva la media y destruye la forma, que es
        // justamente lo que el histograma existe para mostrar.
        if self.window >= Duration::from_secs(45 * 60) {
            let _ = write!(
                out,
                "\n{}",
                goals.render_against_poisson("goles por partido", REAL_GOALS_PER_MATCH)
            );
        } else {
            let _ = writeln!(
                out,
                "\n(sin histograma: {minutes:.0} min escalados a 90 conservan la media y \
                 no la forma; para la distribución, `goal_distribution`)"
            );
        }
        out
    }
}

/// Cuántas veces salió cada valor entero. Es lo que un histograma dibuja y lo
/// que una media esconde.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Distribution {
    counts: Vec<u32>,
}

impl Distribution {
    pub fn of(samples: impl Iterator<Item = u32>) -> Self {
        let mut counts = Vec::new();
        for value in samples {
            let index = value as usize;
            if counts.len() <= index {
                counts.resize(index + 1, 0);
            }
            counts[index] += 1;
        }
        Self { counts }
    }

    pub fn samples(&self) -> u32 {
        self.counts.iter().sum()
    }

    pub fn count_of(&self, value: u32) -> u32 {
        self.counts.get(value as usize).copied().unwrap_or(0)
    }

    pub fn mean(&self) -> f32 {
        let samples = self.samples();
        if samples == 0 {
            return 0.0;
        }
        let total: u32 = self
            .counts
            .iter()
            .enumerate()
            .map(|(value, count)| u32::try_from(value).unwrap_or(u32::MAX) * count)
            .sum();
        total as f32 / samples as f32
    }

    pub fn min(&self) -> u32 {
        u32::try_from(self.counts.iter().position(|count| *count > 0).unwrap_or(0)).unwrap_or(u32::MAX)
    }

    pub fn max(&self) -> u32 {
        self.counts
            .iter()
            .rposition(|count| *count > 0)
            .map_or(0, |value| u32::try_from(value).unwrap_or(u32::MAX))
    }

    /// El histograma observado junto al que produciría una Poisson de media
    /// `lambda`, que es la forma de la distribución real. Dos modelos con la
    /// misma media y distinta forma se distinguen aquí y en ningún otro sitio.
    pub fn render_against_poisson(&self, label: &str, lambda: f32) -> String {
        let samples = self.samples();
        if samples == 0 {
            return format!("{label}: sin muestras\n");
        }
        let mut out = format!("{label:>16}   observado             real (Poisson λ={lambda})\n");
        // La cola se agrupa: con un modelo a un orden de magnitud de la
        // referencia, dibujar una fila por gol daría ochenta filas vacías y
        // ninguna comparación.
        let top = poisson_tail(lambda).max(HISTOGRAM_ROWS);
        for value in 0..=top {
            let observed = self.count_of(value) as f32 / samples as f32;
            let expected = poisson_probability(lambda, value);
            let _ = writeln!(
                out,
                "{value:>14}   {:>5.1}% {:<14}  {:>5.1}% {}",
                observed * 100.0,
                bar(observed),
                expected * 100.0,
                bar(expected),
            );
        }
        let beyond: u32 = (top + 1..=self.max()).map(|v| self.count_of(v)).sum();
        if beyond > 0 {
            let observed = beyond as f32 / samples as f32;
            // la cola, por complemento: sumar términos de Poisson hasta el
            // infinito desborda el factorial y devuelve NaN
            let expected = 1.0
                - (0..=top)
                    .map(|v| poisson_probability(lambda, v))
                    .sum::<f32>();
            let _ = writeln!(
                out,
                "{:>13}+   {:>5.1}% {:<14}  {:>5.1}% {}   ← hasta {}",
                top + 1,
                observed * 100.0,
                bar(observed),
                expected * 100.0,
                bar(expected),
                self.max(),
            );
        }
        out
    }
}

/// Barra proporcional, en décimas. Diez caracteres es el 100 %.
fn bar(fraction: f32) -> String {
    "█".repeat(rounded_count(fraction * 20.0) as usize)
}

fn poisson_probability(lambda: f32, k: u32) -> f32 {
    let factorial: f32 = (1..=k).map(|i| i as f32).product::<f32>().max(1.0);
    (-lambda).exp() * lambda.powi(k as i32) / factorial
}

/// Hasta dónde vale la pena dibujar la Poisson: el último valor con al menos un
/// 1 % de probabilidad.
fn poisson_tail(lambda: f32) -> u32 {
    (0..40)
        .rev()
        .find(|k| poisson_probability(lambda, *k) >= 0.01)
        .unwrap_or(0)
}

/// Un partido headless, con su semilla, hasta agotar la ventana del escenario.
fn run_one_match(scenario: &Scenario, seed: u32) -> MatchSummary {
    let scenario = Scenario {
        seed,
        ..scenario.clone()
    };
    let ticks = scenario.ticks();

    let mut app = App::new();
    app.add_plugins((TaskPoolPlugin::default(), TimePlugin));
    app.add_plugins(MatchKernelPlugin::new(scenario));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK));
    for _ in 0..ticks {
        app.update();
    }

    let ledger = app.world().resource::<MatchLedger>();
    // el tiempo de juego lo lleva el ledger: `period_elapsed` es solo el periodo
    // en curso, y en un partido completo dividir por él infla cada tasa
    let elapsed = ledger.played_time;
    MatchSummary {
        seed,
        goals: ledger.goals,
        shots: ledger.shots,
        shots_on_target: ledger.shots_on_target,
        passes: ledger.passes,
        passes_completed: ledger.passes_completed(),
        possession: ledger.possession_share(),
        possession_changes_per_minute: ledger.changes_per_minute(elapsed),
        tackles: ledger.tackles(),
        fouls: ledger.fouls(),
        loose_balls: ledger.loose_balls(),
        longest_spell: ledger.longest_spell,
        distinct_touchers: ledger.distinct_touchers(),
        elapsed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_distribution_counts_what_a_mean_hides() {
        // dos muestras de media 3: una concentrada, otra en los extremos
        let concentrated = Distribution::of([3, 3, 3, 3].into_iter());
        let extreme = Distribution::of([0, 6, 0, 6].into_iter());

        assert_eq!(concentrated.mean(), extreme.mean());
        assert_ne!(concentrated, extreme);
        assert_eq!(concentrated.count_of(3), 4);
        assert_eq!(extreme.count_of(3), 0);
        assert_eq!(extreme.min(), 0);
        assert_eq!(extreme.max(), 6);
    }

    /// La Poisson de referencia tiene que sumar uno y tener su moda donde debe,
    /// o el histograma comparado miente en la columna que sirve de patrón.
    #[test]
    fn the_reference_poisson_is_a_probability_distribution() {
        let total: f32 = (0..40)
            .map(|k| poisson_probability(REAL_GOALS_PER_MATCH, k))
            .sum();
        assert!((total - 1.0).abs() < 1e-3, "suma {total}");

        // λ = 2,7 → un partido de 2 goles es el más probable, y el 0-0 pasa
        // alrededor del 7 % de las veces
        let modal = (0..10)
            .max_by(|a, b| {
                poisson_probability(REAL_GOALS_PER_MATCH, *a)
                    .total_cmp(&poisson_probability(REAL_GOALS_PER_MATCH, *b))
            })
            .unwrap();
        assert_eq!(modal, 2);
        assert!((poisson_probability(REAL_GOALS_PER_MATCH, 0) - 0.067).abs() < 0.01);
    }
}
