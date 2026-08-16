//! Correr un escenario hasta el final, sin nada que mire.
//!
//! Vivía en la capa de composición, junto a las importaciones de presentación, y
//! eso hacía que cualquier test que solo nombrase `ScenarioRunner` enlazara el
//! renderer entero para simular sin dibujar nada: el kernel corría sin renderer
//! y lo enlazaba igual. Aquí no puede, porque este crate no conoce más Bevy que
//! `bevy_app`, `bevy_ecs` y `bevy_time` (§ capas de `ARCHITECTURE.md`).
//!
//! La variante con primitivas se monta desde arriba, sobre esta misma app, para
//! que las dos formas de correr una situación sigan siendo la misma corrida.

use bevy_app::App;
use bevy_ecs::schedule::{Schedules, SingleThreadedExecutor};
use bevy_ecs::world::World;
use bevy_time::{TimePlugin, TimeUpdateStrategy};
use football_domain::scenario::{PlayState, TICK};
use football_domain::{ByTeam, MatchPhase, MatchState, Scenario, ScenarioOutcome, SetPiece};

use crate::MatchKernelPlugin;

/// Builds the reference headless app used by both a single teaching situation
/// and a calibration envelope. Keeping this setup in one place prevents a
/// measurement from changing merely because the scheduler picked another
/// valid parallel order.
pub(crate) fn headless_scenario_app(scenario: Scenario) -> App {
    scenario
        .validate()
        .unwrap_or_else(|error| panic!("cannot run invalid scenario: {error}"));
    let mut app = App::new();
    app.add_plugins(TimePlugin);
    app.add_plugins(MatchKernelPlugin::new(scenario));
    for (_, schedule) in app.world_mut().resource_mut::<Schedules>().iter_mut() {
        schedule.set_executor(SingleThreadedExecutor::new());
    }
    app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK));
    app
}

/// Runs one scenario, one fixed tick per step.
pub struct ScenarioRunner {
    app: App,
    scenario: Scenario,
    awarded_set_pieces: Vec<SetPiece>,
    previous_set_piece: SetPiece,
    entered_phases: Vec<MatchPhase>,
    previous_phase: MatchPhase,
    play_resumed: bool,
    ticks_simulated: u32,
}

impl ScenarioRunner {
    /// The scenario with no presentation at all: no window, no assets, no
    /// renderer. This is the reference run.
    pub fn headless(scenario: Scenario) -> Self {
        let app = headless_scenario_app(scenario.clone());
        Self::from_app(app, scenario)
    }

    fn from_app(app: App, scenario: Scenario) -> Self {
        let opening_set_piece = match scenario.play_state {
            PlayState::AwaitingRestart { set_piece, .. } => set_piece,
            PlayState::InPlay => SetPiece::None,
        };
        Self {
            app,
            scenario,
            awarded_set_pieces: Vec::new(),
            previous_set_piece: opening_set_piece,
            entered_phases: Vec::new(),
            previous_phase: MatchPhase::PreMatch,
            play_resumed: false,
            ticks_simulated: 0,
        }
    }

    /// Advances one fixed tick and records what the referee did.
    pub fn advance(&mut self) {
        self.app.update();
        self.ticks_simulated += 1;

        let state = self.app.world().resource::<MatchState>();
        let (set_piece, phase) = (state.set_piece, state.phase);

        if set_piece != self.previous_set_piece {
            if set_piece == SetPiece::None {
                self.play_resumed = true;
            } else {
                self.awarded_set_pieces.push(set_piece);
            }
            self.previous_set_piece = set_piece;
        }
        if phase != self.previous_phase {
            self.entered_phases.push(phase);
            self.previous_phase = phase;
        }
    }

    /// Runs the scenario's whole window.
    pub fn run(mut self) -> ScenarioOutcome {
        for _ in 0..self.scenario.ticks() {
            self.advance();
        }
        self.outcome()
    }

    pub fn outcome(&self) -> ScenarioOutcome {
        let state = self.app.world().resource::<MatchState>();
        ScenarioOutcome {
            scenario_name: self.scenario.name.clone(),
            ticks_simulated: self.ticks_simulated,
            score: ByTeam::new(state.home_score, state.away_score),
            set_pieces: self.awarded_set_pieces.clone(),
            phases: self.entered_phases.clone(),
            final_phase: state.phase,
            period_elapsed: state.period_elapsed,
            play_resumed: self.play_resumed,
        }
    }

    /// Access for inspection and diagnostics. Queries need mutable access to the
    /// world even to read, so this is the only accessor.
    pub fn world_mut(&mut self) -> &mut World {
        self.app.world_mut()
    }

    /// La app, para que la capa de composición le añada la presentación. Es lo
    /// único que necesita de aquí, y por eso no se expone nada más.
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// Panics with every way the run failed the scenario's claims.
    pub fn assert_scenario_holds(self) {
        // A self-contradicting scenario is the scenario's fault, not the
        // kernel's, so it is reported as such before a single tick runs.
        let contradictions = self.scenario.contradictions();
        assert!(
            contradictions.is_empty(),
            "scenario '{}' cannot be asserted:\n  - {}",
            self.scenario.name,
            contradictions.join("\n  - ")
        );

        let expectations = self.scenario.expectations.clone();
        let outcome = self.run();
        let mismatches = outcome.mismatches(&expectations);
        assert!(
            mismatches.is_empty(),
            "scenario '{}' did not hold after {} ticks:\n  - {}",
            outcome.scenario_name,
            outcome.ticks_simulated,
            mismatches.join("\n  - ")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_math::Vec3;
    use football_domain::{TeamId, scenario::BallSetup};
    use std::time::Duration;

    #[test]
    fn an_exported_teaching_situation_reproduces_the_same_headless_run() {
        let source = Scenario::kick_off()
            .named("imported live ball")
            .with_ball(
                BallSetup::travelling_from(Vec3::new(8.0, -5.0, 0.4), Vec3::new(4.0, 1.0, 0.0))
                    .last_touched_by(TeamId::Home),
            )
            .already_in_play()
            .for_duration(Duration::from_millis(800));
        let imported = Scenario::from_situation_text(
            &source
                .to_situation_text()
                .expect("a canonical situation exports"),
        )
        .expect("the exported situation imports");

        let original_outcome = ScenarioRunner::headless(source).run();
        let imported_outcome = ScenarioRunner::headless(imported).run();
        assert_eq!(
            original_outcome.ticks_simulated,
            imported_outcome.ticks_simulated
        );
        assert_eq!(original_outcome.score, imported_outcome.score);
        assert_eq!(original_outcome.set_pieces, imported_outcome.set_pieces);
        assert_eq!(original_outcome.phases, imported_outcome.phases);
        assert_eq!(original_outcome.final_phase, imported_outcome.final_phase);
        assert_eq!(
            original_outcome.period_elapsed,
            imported_outcome.period_elapsed
        );
        assert_eq!(original_outcome.play_resumed, imported_outcome.play_resumed);
    }
}
