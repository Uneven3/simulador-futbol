//! App: composition and lifecycle.
//!
//! Nothing here decides anything about football. It assembles the authoritative
//! kernel, optionally attaches presentation, and runs a scenario to its end.

pub mod scenarios;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use football_domain::scenario::{PlayState, TICK};
use football_presentation::{
    DebugHubPlugin, DiagnosticOverlaysPlugin, MatchHudPlugin, PrimitiveVisualsPlugin,
};
use football_simulation::MatchKernelPlugin;

pub use football_domain::scenario;
pub use football_domain::{
    ByTeam, MatchPhase, MatchState, PlayerId, Scenario, ScenarioOutcome, SetPiece, TeamId,
};

/// Runs one scenario, with or without visuals, one fixed tick per step.
///
/// The point of this type is that both ways of running a situation are the same
/// run: `headless` and `with_primitives` add the identical kernel, and the
/// second one merely lets presentation watch. Any divergence between them is a
/// bug in presentation, not a difference in configuration.
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
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(MatchKernelPlugin::new(scenario.clone()));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(TICK));
        Self::from_app(app, scenario)
    }

    /// The same scenario with primitive visuals attached. Assets are registered
    /// without a renderer: presentation needs somewhere to put meshes, not a
    /// window, so this still runs in a test or on a headless machine.
    pub fn with_primitives(scenario: Scenario) -> Self {
        let mut runner = Self::headless(scenario);
        runner.app.add_plugins(AssetPlugin::default());
        runner.app.init_asset::<Mesh>();
        runner.app.init_asset::<StandardMaterial>();
        runner.app.add_plugins((
            PrimitiveVisualsPlugin,
            DiagnosticOverlaysPlugin,
            DebugHubPlugin,
            MatchHudPlugin,
        ));
        runner
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

    /// Panics with every way the run failed the scenario's claims.
    pub fn assert_scenario_holds(self) {
        // A scenario that contradicts itself would either fail forever or pass
        // for the wrong reason, and one with a match-length window would stall
        // the suite. Both are the scenario's fault, not the kernel's, so they
        // are reported as such before a single tick runs.
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
