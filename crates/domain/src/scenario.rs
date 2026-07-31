use crate::{ByTeam, MatchPhase, MatchRegulations, MatchTuning, PitchConfig, SetPiece, TeamId};
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use std::time::Duration;

/// Fixed simulation rate. The analytical ball integrator advances in 10 ms
/// steps, so this is part of the model, not a performance setting.
pub const SIMULATION_HZ: f64 = 100.0;

/// Duration of one simulation tick.
pub const TICK: Duration = Duration::from_millis(10);

/// Edition of the Laws of the Game a scenario is judged against.
///
/// Recorded, not yet branched on: no rule reads this today. It exists so a
/// result can never be reported without saying which laws produced it (law 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LawsEdition {
    Ifab2026_27,
}

/// How the ball starts.
#[derive(Debug, Clone, Copy)]
pub struct BallSetup {
    /// Centre of the ball, in metres.
    pub position: Vec3,
    /// Initial velocity in m/s. Zero for a dead ball.
    pub momentum: Vec3,
    /// Team credited with the last touch before the scenario begins. The referee
    /// needs it to award a restart to the right side; `None` means untouched.
    pub last_touched_by_team: Option<TeamId>,
}

impl BallSetup {
    /// At rest on the centre spot.
    pub fn on_the_centre_spot() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, crate::BALL_RADIUS),
            momentum: Vec3::ZERO,
            last_touched_by_team: None,
        }
    }

    pub fn travelling_from(position: Vec3, momentum: Vec3) -> Self {
        Self {
            position,
            momentum,
            last_touched_by_team: None,
        }
    }

    pub fn last_touched_by(mut self, team: TeamId) -> Self {
        self.last_touched_by_team = Some(team);
        self
    }
}

/// Which players take part.
///
/// Placing individual players explicitly is what MVP 6 needs to reconstruct a
/// real situation; until there is something to reconstruct from, a scenario
/// either fields both teams or isolates the ball.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerSetup {
    /// Both teams at their base 4-4-2 positions.
    DefaultFormations,
    /// No players at all, which isolates ball physics and referee decisions
    /// from any tactical behaviour.
    BallOnly,
    /// Only the two goalkeepers, which isolates what happens at the goal from
    /// everything a team does to get there.
    GoalkeepersOnly,
}

/// Whether the scenario opens with the ball live or with play stopped.
#[derive(Debug, Clone, Copy)]
pub enum PlayState {
    /// Stopped, waiting for a restart to be taken by `team`.
    AwaitingRestart {
        set_piece: SetPiece,
        team: TeamId,
        delay: Duration,
    },
    /// Live: the ball is already in play.
    InPlay,
}

/// What the scenario claims should happen. Absent fields make no claim.
#[derive(Debug, Clone, Default)]
pub struct Expectations {
    pub score: Option<ByTeam<u32>>,
    /// Restarts the referee must award, in order. Extra restarts after these
    /// are allowed: a scenario states what must happen, not everything that may.
    pub set_pieces: Vec<SetPiece>,
    /// Phases the match must pass through, in order.
    pub phases: Vec<MatchPhase>,
    /// Play must be running again by the end of the window.
    pub play_resumes: bool,
    /// The ball must never leave play: no restart at all is awarded. This is how
    /// a near miss is stated — the difference between "no goal" and "no goal and
    /// the ball is still live" matters at the goal line.
    pub play_never_stops: bool,
}

/// A reproducible situation: initial state, seed, window and claims.
///
/// The same value drives the headless runner and the rendered one; presentation
/// is never part of it.
#[derive(Resource, Debug, Clone)]
pub struct Scenario {
    pub name: String,
    pub laws_edition: LawsEdition,
    pub competition: Option<String>,
    pub pitch: PitchConfig,
    /// Law 7 lengths for this match.
    pub regulations: MatchRegulations,
    /// The parameters that fix how the match turns out. Part of the scenario for
    /// the same reason the seed is: a result cannot be reported, or reproduced,
    /// without the numbers that produced it.
    pub tuning: MatchTuning,
    /// Seed for every random draw in the match, so a run is repeatable.
    pub seed: u32,
    pub ball: BallSetup,
    pub players: PlayerSetup,
    pub play_state: PlayState,
    /// How much match time to simulate.
    pub window: Duration,
    pub expectations: Expectations,
}

impl Scenario {
    /// The standard opening: both teams formed up, home to kick off.
    pub fn kick_off() -> Self {
        Self {
            name: "kick-off".to_string(),
            laws_edition: LawsEdition::Ifab2026_27,
            competition: None,
            pitch: PitchConfig::default(),
            regulations: MatchRegulations::default(),
            tuning: MatchTuning::default(),
            seed: 0xC0FFEE,
            ball: BallSetup::on_the_centre_spot(),
            players: PlayerSetup::DefaultFormations,
            play_state: PlayState::AwaitingRestart {
                set_piece: SetPiece::KickOff,
                team: TeamId::Home,
                delay: Duration::from_secs(2),
            },
            window: Duration::from_secs(90 * 60),
            expectations: Expectations {
                play_resumes: true,
                ..Default::default()
            },
        }
    }

    pub fn named(mut self, name: &str) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn with_ball(mut self, ball: BallSetup) -> Self {
        self.ball = ball;
        self
    }

    pub fn with_players(mut self, players: PlayerSetup) -> Self {
        self.players = players;
        self
    }

    pub fn already_in_play(mut self) -> Self {
        self.play_state = PlayState::InPlay;
        self
    }

    pub fn for_duration(mut self, window: Duration) -> Self {
        self.window = window;
        self
    }

    pub fn with_regulations(mut self, regulations: MatchRegulations) -> Self {
        self.regulations = regulations;
        self
    }

    /// Run the same situation under different parameters. This is what a sweep
    /// turns: the scenario is otherwise identical, so any difference in the
    /// outcome is the tuning's doing.
    pub fn with_tuning(mut self, tuning: MatchTuning) -> Self {
        self.tuning = tuning;
        self
    }

    pub fn expecting(mut self, expectations: Expectations) -> Self {
        self.expectations = expectations;
        self
    }

    /// Number of fixed ticks in the scenario's window, saturated: a window that
    /// does not fit in `u32` ticks is 497 days of match.
    pub fn ticks(&self) -> u32 {
        u32::try_from(self.window.as_millis() / TICK.as_millis()).unwrap_or(u32::MAX)
    }
}

/// What actually happened in one run.
#[derive(Debug, Clone)]
pub struct ScenarioOutcome {
    pub scenario_name: String,
    pub ticks_simulated: u32,
    pub score: ByTeam<u32>,
    /// Restarts awarded, in the order the referee awarded them.
    pub set_pieces: Vec<SetPiece>,
    /// Phases entered, in order.
    pub phases: Vec<MatchPhase>,
    pub final_phase: MatchPhase,
    /// Time played in the period the run ended in.
    pub period_elapsed: Duration,
    pub play_resumed: bool,
}

impl Scenario {
    /// Every way this scenario contradicts itself, in plain language. Empty
    /// means it is worth running.
    ///
    /// A scenario is a claim about a situation, and a claim nobody can satisfy
    /// is worse than no claim: it either fails forever or, worse, passes for the
    /// wrong reason.
    pub fn contradictions(&self) -> Vec<String> {
        let mut found = Vec::new();

        if self.expectations.play_never_stops && !self.expectations.set_pieces.is_empty() {
            found.push(format!(
                "expects play never to stop and also expects {:?}",
                self.expectations.set_pieces
            ));
        }
        if self.expectations.play_never_stops && self.expectations.play_resumes {
            found.push(
                "expects play never to stop, so there is nothing for it to resume from".to_string(),
            );
        }
        if let Some(score) = self.expectations.score
            && (score.home > 0 || score.away > 0)
            && self.expectations.play_never_stops
        {
            found.push(
                "expects a goal, which stops play, and also that play never stops".to_string(),
            );
        }
        if self.window > MAX_REASONABLE_WINDOW {
            found.push(format!(
                "runs for {:?}, which is {} ticks — too long to belong in a suite",
                self.window,
                self.ticks()
            ));
        }

        found
    }
}

/// The longest a catalogued scenario may run. `Scenario::kick_off()` opens with
/// a full 90-minute window because it describes a match, not a situation; a
/// scenario meant to be asserted has to be cut down from it, or a suite that
/// runs it stalls on 540,000 ticks with nobody the wiser.
pub const MAX_REASONABLE_WINDOW: Duration = Duration::from_secs(20 * 60);

impl ScenarioOutcome {
    /// Every way this run failed the scenario's claims, in plain language.
    /// Empty means the scenario held.
    pub fn mismatches(&self, expectations: &Expectations) -> Vec<String> {
        let mut mismatches = Vec::new();

        if let Some(expected) = expectations.score
            && expected != self.score
        {
            mismatches.push(format!(
                "score was {}-{}, expected {}-{}",
                self.score.home, self.score.away, expected.home, expected.away
            ));
        }

        // the expected restarts must appear in order, extras are tolerated
        let mut awarded = self.set_pieces.iter();
        for expected in &expectations.set_pieces {
            if !awarded.any(|observed| observed == expected) {
                mismatches.push(format!(
                    "expected a {expected:?} that never came (awarded: {:?})",
                    self.set_pieces
                ));
            }
        }

        let mut entered = self.phases.iter();
        for expected in &expectations.phases {
            if !entered.any(|observed| observed == expected) {
                mismatches.push(format!(
                    "expected the match to reach {expected:?}, it went through {:?}",
                    self.phases
                ));
            }
        }

        if expectations.play_resumes && !self.play_resumed {
            mismatches.push("play never resumed".to_string());
        }

        if expectations.play_never_stops && !self.set_pieces.is_empty() {
            mismatches.push(format!(
                "play was expected to continue, but {:?} was awarded",
                self.set_pieces
            ));
        }

        mismatches
    }
}
