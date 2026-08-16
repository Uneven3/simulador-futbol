//! Comparar alternativas de una misma situación, sin confundir una trayectoria
//! con una recomendación.
//!
//! Cada alternativa cambia solamente las propuestas declaradas. Campo, estado
//! inicial, tuning y, sobre todo, las semillas son idénticos: la comparación
//! responde al cambio propuesto y no a otro partido casualmente distinto.

use football_domain::{
    CounterfactualOverlay, CounterfactualOverlayAlternative, MovementProposal, Scenario,
};

use crate::envelope::{EnvelopeReport, EnvelopeSpec};

/// Una opción que se quiere contrastar dentro de la misma situación.
#[derive(Debug, Clone)]
pub struct CounterfactualAlternative {
    pub name: String,
    pub movement_proposals: Vec<MovementProposal>,
}

impl CounterfactualAlternative {
    pub fn movement(name: impl Into<String>, movement_proposals: Vec<MovementProposal>) -> Self {
        Self {
            name: name.into(),
            movement_proposals,
        }
    }
}

/// La distribución que produjo una alternativa, con la situación exacta que
/// se corrió para poder repetirla o inspeccionarla después.
#[derive(Debug, Clone)]
pub struct CounterfactualResult {
    pub name: String,
    pub scenario: Scenario,
    pub report: EnvelopeReport,
}

/// El contraste completo. No elige una alternativa: muestra el espacio de
/// resultados para que enseñanza y análisis expliquen el coste de cada una.
#[derive(Debug, Clone)]
pub struct CounterfactualReport {
    pub situation_name: String,
    pub seeds: Vec<u32>,
    pub alternatives: Vec<CounterfactualResult>,
}

impl CounterfactualReport {
    /// Corre cada alternativa contra exactamente las mismas semillas.
    pub fn run(
        situation: &Scenario,
        seeds: Vec<u32>,
        alternatives: impl IntoIterator<Item = CounterfactualAlternative>,
    ) -> Self {
        let alternatives = alternatives
            .into_iter()
            .map(|alternative| {
                let scenario = situation
                    .clone()
                    .named(&format!("{} · {}", situation.name, alternative.name))
                    .with_movement_proposals(alternative.movement_proposals);
                let report = EnvelopeReport::run(&EnvelopeSpec {
                    scenario: scenario.clone(),
                    seeds: seeds.clone(),
                });
                CounterfactualResult {
                    name: alternative.name,
                    scenario,
                    report,
                }
            })
            .collect();

        Self {
            situation_name: situation.name.clone(),
            seeds,
            alternatives,
        }
    }

    /// El resumen que puede insertarse en una app con presentación. Conserva
    /// intenciones declaradas; los resultados siguen siendo las métricas del
    /// informe y no se reinterpretan visualmente.
    pub fn overlay(&self) -> CounterfactualOverlay {
        CounterfactualOverlay {
            alternatives: self
                .alternatives
                .iter()
                .map(|alternative| CounterfactualOverlayAlternative {
                    name: alternative.name.clone(),
                    movement_proposals: alternative.scenario.movement_proposals.clone(),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_math::Vec2;
    use football_domain::{PlayerId, TeamId};
    use std::time::Duration;

    #[test]
    fn every_alternative_uses_the_same_situation_and_seed_envelope() {
        let situation = Scenario::kick_off()
            .already_in_play()
            .for_duration(Duration::from_millis(20));
        let advance = MovementProposal {
            player: PlayerId::new(TeamId::Home, 10),
            desired_velocity: Vec2::X,
        };
        let report = CounterfactualReport::run(
            &situation,
            vec![7, 42],
            [
                CounterfactualAlternative::movement("advance", vec![advance]),
                CounterfactualAlternative::movement("hold", vec![]),
            ],
        );

        assert_eq!(report.situation_name, "kick-off");
        assert_eq!(report.seeds, vec![7, 42]);
        assert_eq!(report.alternatives.len(), 2);
        assert_eq!(
            report.alternatives[0].scenario.movement_proposals,
            vec![advance]
        );
        assert!(
            report.alternatives[1]
                .scenario
                .movement_proposals
                .is_empty()
        );
        for alternative in &report.alternatives {
            assert_eq!(alternative.report.matches.len(), 2);
            assert_eq!(
                alternative
                    .report
                    .matches
                    .iter()
                    .map(|match_| match_.seed)
                    .collect::<Vec<_>>(),
                vec![7, 42]
            );
            assert!(
                alternative
                    .report
                    .matches
                    .iter()
                    .all(|match_| match_.elapsed > Duration::ZERO),
                "a counterfactual must exercise the kernel, not only its envelope shape"
            );
        }
        assert_eq!(report.overlay().alternatives.len(), 2);
    }
}
