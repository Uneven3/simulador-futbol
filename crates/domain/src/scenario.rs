use crate::{
    ByTeam, MatchPhase, MatchRegulations, MatchTuning, Observation, PitchConfig, PlayerId,
    PlayingPosition, SetPiece, TacticalPlans, TacticalRole, TeamId,
};
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use serde::{Deserialize, Serialize};
use std::{fmt, fmt::Write, time::Duration};

/// Fixed simulation rate. The analytical ball integrator advances in 10 ms
/// steps, so this is part of the model, not a performance setting.
pub const SIMULATION_HZ: f64 = 100.0;

/// Duration of one simulation tick.
pub const TICK: Duration = Duration::from_millis(10);

/// Edition of the Laws of the Game a scenario is judged against.
///
/// Recorded, not yet branched on: no rule reads this today. It exists so a
/// result can never be reported without saying which laws produced it (law 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LawsEdition {
    Ifab2026_27,
}

/// How the ball starts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
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

/// Which players take part. Placing individuals explicitly is what MVP 6 needs
/// to reconstruct a real situation; until then a scenario either fields both
/// teams or isolates the ball.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Colocación que sobrescribe al once de referencia de una situación.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PlayerPlacement {
    pub id: PlayerId,
    pub position: PlayingPosition,
    pub role: TacticalRole,
    pub formation_slot: Vec2,
    pub on_pitch: Vec3,
}

/// Movimiento que una alternativa propone a una persona concreta. Es intención
/// en m/s, no una teletransportación: el motor conserva la última palabra sobre
/// qué velocidad y posición logra el cuerpo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MovementProposal {
    pub player: PlayerId,
    pub desired_velocity: Vec2,
}

/// Datos de presentación de alternativas: una capa visual los consume, pero no
/// decide ni ejecuta ninguna propuesta.
#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub struct CounterfactualOverlay {
    pub alternatives: Vec<CounterfactualOverlayAlternative>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CounterfactualOverlayAlternative {
    pub name: String,
    pub movement_proposals: Vec<MovementProposal>,
}

/// What an initial belief is about. Identity belongs to the scenario registry,
/// never to a visual observation, so a player observation names its subject.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationSubject {
    Ball,
    Player(PlayerId),
}

/// Knowledge a player starts a teaching situation with. It is intentionally a
/// belief, with a spot, velocity and blur, rather than a copy of a body or ball
/// component. Thus a constructed situation can start after a scan, a shout or
/// a previous phase of play without letting decisions read current truth.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct InitialObservation {
    pub observer: PlayerId,
    pub subject: ObservationSubject,
    pub observation: Observation,
}

/// A named replacement on a team sheet. It is served only at a stoppage, never
/// by changing a body while the ball is live (Law 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Substitution {
    pub outgoing: PlayerId,
    pub incoming: PlayerId,
}

impl Substitution {
    pub fn new(outgoing: PlayerId, incoming: PlayerId) -> Self {
        Self { outgoing, incoming }
    }

    pub fn team(self) -> Option<TeamId> {
        (self.outgoing.team == self.incoming.team).then_some(self.outgoing.team)
    }
}

/// Whether the scenario opens with the ball live or with play stopped.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
#[derive(Resource, Debug, Clone, Serialize, Deserialize)]
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
    /// Las instrucciones configurables de ambos equipos son parte de la
    /// situación reproducible, igual que el campo y la semilla.
    pub tactical_plans: TacticalPlans,
    /// Seed for every random draw in the match, so a run is repeatable.
    pub seed: u32,
    pub ball: BallSetup,
    pub players: PlayerSetup,
    /// Ajustes individuales de la plantilla base. Una situación de enseñanza
    /// puede declarar los veintidós sin inferir posiciones ni roles.
    pub player_placements: Vec<PlayerPlacement>,
    /// Alternativa contrafactual que sustituye solo las intenciones nombradas.
    pub movement_proposals: Vec<MovementProposal>,
    /// Beliefs declared at the first tick, before the sensors produce any new
    /// observation. A player absent from `players` cannot receive one.
    pub initial_observations: Vec<InitialObservation>,
    /// Changes declared by this reproducible situation, served at the next
    /// legal stoppage under the competition's regulations.
    pub substitutions: Vec<Substitution>,
    pub play_state: PlayState,
    /// How much match time to simulate.
    pub window: Duration,
    pub expectations: Expectations,
}

impl Scenario {
    /// Complete, versioned RON representation of the scenario, including its
    /// tuning, regulations and tactical plans. Unlike the compact teaching
    /// text below it never drops a causal input.
    pub fn to_ron(&self) -> Result<String, SituationTextError> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::new())
            .map_err(|error| SituationTextError(format!("could not write scenario RON: {error}")))
    }

    /// Reads the complete RON representation produced by [`Self::to_ron`].
    pub fn from_ron(text: &str) -> Result<Self, SituationTextError> {
        let scenario: Self = ron::from_str(text)
            .map_err(|error| SituationTextError(format!("could not read scenario RON: {error}")))?;
        scenario.validate()?;
        Ok(scenario)
    }

    /// The public situation interchange is complete RON, not the former
    /// compact subset: every causal field survives export/import.
    pub fn to_situation_text(&self) -> Result<String, SituationTextError> {
        self.to_ron()
    }

    pub fn from_situation_text(text: &str) -> Result<Self, SituationTextError> {
        if text.starts_with(SITUATION_TEXT_HEADER) {
            Self::from_compact_situation_text(text)
        } else {
            Self::from_ron(text)
        }
    }

    /// The standard opening: both teams formed up, home to kick off.
    pub fn kick_off() -> Self {
        Self {
            name: "kick-off".to_string(),
            laws_edition: LawsEdition::Ifab2026_27,
            competition: None,
            pitch: PitchConfig::default(),
            regulations: MatchRegulations::default(),
            tuning: MatchTuning::default(),
            tactical_plans: TacticalPlans::default(),
            seed: 0xC0FFEE,
            ball: BallSetup::on_the_centre_spot(),
            players: PlayerSetup::DefaultFormations,
            player_placements: Vec::new(),
            movement_proposals: Vec::new(),
            initial_observations: Vec::new(),
            substitutions: Vec::new(),
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

    pub fn with_player_placements(mut self, player_placements: Vec<PlayerPlacement>) -> Self {
        self.player_placements = player_placements;
        self
    }

    pub fn with_movement_proposals(mut self, movement_proposals: Vec<MovementProposal>) -> Self {
        self.movement_proposals = movement_proposals;
        self
    }

    pub fn with_initial_observations(
        mut self,
        initial_observations: Vec<InitialObservation>,
    ) -> Self {
        self.initial_observations = initial_observations;
        self
    }

    pub fn with_substitutions(mut self, substitutions: Vec<Substitution>) -> Self {
        self.substitutions = substitutions;
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

    pub fn with_tactical_plans(mut self, tactical_plans: TacticalPlans) -> Self {
        self.tactical_plans = tactical_plans;
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
    /// Rejects a situation before it can silently omit one of its declarations.
    /// Importers and runners call this at their boundary; constructors remain
    /// ergonomic for tests and scenario catalogues.
    pub fn validate(&self) -> Result<(), SituationTextError> {
        // A full 90-minute match is valid domain data even though asserting it
        // in the fast suite is not. `contradictions` keeps that suite guard for
        // `assert_scenario_holds`; interchange must not reject real matches.
        let contradictions: Vec<_> = self
            .contradictions()
            .into_iter()
            .filter(|message| !message.starts_with("runs for "))
            .collect();
        if contradictions.is_empty() {
            Ok(())
        } else {
            Err(SituationTextError(format!(
                "the situation contradicts itself: {}",
                contradictions.join("; ")
            )))
        }
    }

    /// Every way this scenario contradicts itself, in plain language; empty
    /// means it is worth running. A claim nobody can satisfy is worse than no
    /// claim: it fails forever, or passes for the wrong reason.
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
        for (index, placement) in self.player_placements.iter().enumerate() {
            if self.player_placements[..index]
                .iter()
                .any(|known| known.id == placement.id)
            {
                found.push(format!("places {} more than once", placement.id));
            }
            if !self.includes_player(placement.id) {
                found.push(format!(
                    "places {}, who is not in the declared line-up",
                    placement.id
                ));
            }
        }
        for (index, proposal) in self.movement_proposals.iter().enumerate() {
            if self.movement_proposals[..index]
                .iter()
                .any(|known| known.player == proposal.player)
            {
                found.push(format!(
                    "proposes more than one movement for {}",
                    proposal.player
                ));
            }
            if !self.includes_player(proposal.player) {
                found.push(format!(
                    "proposes movement for {}, who is not on the pitch",
                    proposal.player
                ));
            }
        }
        for (index, initial) in self.initial_observations.iter().enumerate() {
            if self.initial_observations[..index]
                .iter()
                .any(|known| known.observer == initial.observer && known.subject == initial.subject)
            {
                found.push(format!(
                    "declares more than one initial observation of {:?} for {}",
                    initial.subject, initial.observer
                ));
            }
            if !self.includes_player(initial.observer) {
                found.push(format!(
                    "declares an initial observation for {}, who is not on the pitch",
                    initial.observer
                ));
            }
            if let ObservationSubject::Player(subject) = initial.subject
                && !self.includes_player(subject)
            {
                found.push(format!(
                    "declares an observation of {}, who is not on the pitch",
                    subject
                ));
            }
            if initial.observation.seen_at != Duration::ZERO {
                found.push(format!(
                    "declares an initial observation for {} after the situation starts",
                    initial.observer
                ));
            }
        }
        for substitution in &self.substitutions {
            if substitution.team().is_none() {
                found.push(format!(
                    "tries to substitute {} with {} across teams",
                    substitution.outgoing, substitution.incoming
                ));
            }
            if substitution.outgoing == substitution.incoming {
                found.push(format!(
                    "substitutes {} for themselves",
                    substitution.outgoing
                ));
            }
            if !self.includes_player(substitution.outgoing) {
                found.push(format!(
                    "substitutes out {}, who is not in the declared line-up",
                    substitution.outgoing
                ));
            }
        }
        if !self.regulations.shootout_conversion_probability.is_finite()
            || self.regulations.shootout_conversion_probability <= 0.0
            || self.regulations.shootout_conversion_probability >= 1.0
        {
            found.push(
                "shoot-out conversion probability must be finite and strictly between zero and one"
                    .to_string(),
            );
        }

        found
    }

    fn includes_player(&self, id: PlayerId) -> bool {
        match self.players {
            PlayerSetup::DefaultFormations => (1..=11).contains(&id.shirt),
            PlayerSetup::GoalkeepersOnly => id.shirt == 1,
            PlayerSetup::BallOnly => false,
        }
    }
}

/// The longest a catalogued scenario may run. `Scenario::kick_off()` opens with
/// a full 90-minute window because it describes a match, not a situation; a
/// scenario meant to be asserted has to be cut down from it, or a suite that
/// runs it stalls on 540,000 ticks with nobody the wiser.
pub const MAX_REASONABLE_WINDOW: Duration = Duration::from_secs(20 * 60);

/// Header of the small, dependency-free interchange format for teaching
/// situations. The version is explicit so old situations never quietly acquire
/// a different meaning when the model grows.
pub const SITUATION_TEXT_HEADER: &str = "gameplayfootball-situation 1";

/// An invalid or unsupported teaching-situation document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SituationTextError(String);

impl fmt::Display for SituationTextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for SituationTextError {}

impl Scenario {
    /// Exports the initial state and explicitly proposed alternative in a
    /// stable, human-reviewable text form. The format carries the canonical
    /// Laws, pitch, regulations, tuning and tactical plan of this version; a
    /// scenario that changes one of those must be shared as Rust data until a
    /// corresponding versioned field is added here rather than silently losing
    /// its causal inputs.
    pub fn to_compact_situation_text(&self) -> Result<String, SituationTextError> {
        let contradictions = self.contradictions();
        if !contradictions.is_empty() {
            return Err(SituationTextError(format!(
                "the situation contradicts itself: {}",
                contradictions.join("; ")
            )));
        }
        self.require_canonical_situation_context()?;
        if self.name.contains(['\n', '\r']) {
            return Err(SituationTextError(
                "a situation name cannot contain a line break".to_string(),
            ));
        }

        let mut text = String::new();
        writeln!(text, "{SITUATION_TEXT_HEADER}").expect("writing to String cannot fail");
        writeln!(text, "name={}", self.name).expect("writing to String cannot fail");
        writeln!(text, "seed={}", self.seed).expect("writing to String cannot fail");
        writeln!(text, "players={}", player_setup_text(self.players))
            .expect("writing to String cannot fail");
        writeln!(
            text,
            "ball={},{},{},{},{},{},{}",
            self.ball.position.x,
            self.ball.position.y,
            self.ball.position.z,
            self.ball.momentum.x,
            self.ball.momentum.y,
            self.ball.momentum.z,
            optional_team_text(self.ball.last_touched_by_team),
        )
        .expect("writing to String cannot fail");
        match self.play_state {
            PlayState::InPlay => writeln!(text, "play=in-play"),
            PlayState::AwaitingRestart {
                set_piece,
                team,
                delay,
            } => writeln!(
                text,
                "play=awaiting-restart,{},{},{}",
                set_piece_text(set_piece),
                team_text(team),
                delay.as_millis()
            ),
        }
        .expect("writing to String cannot fail");
        writeln!(text, "window-ms={}", self.window.as_millis())
            .expect("writing to String cannot fail");

        for placement in &self.player_placements {
            writeln!(
                text,
                "placement={},{},{},{},{},{},{},{},{}",
                team_text(placement.id.team),
                placement.id.shirt,
                playing_position_text(placement.position),
                tactical_role_text(placement.role),
                placement.formation_slot.x,
                placement.formation_slot.y,
                placement.on_pitch.x,
                placement.on_pitch.y,
                placement.on_pitch.z,
            )
            .expect("writing to String cannot fail");
        }
        for proposal in &self.movement_proposals {
            writeln!(
                text,
                "proposal={},{},{},{}",
                team_text(proposal.player.team),
                proposal.player.shirt,
                proposal.desired_velocity.x,
                proposal.desired_velocity.y,
            )
            .expect("writing to String cannot fail");
        }
        for initial in &self.initial_observations {
            write!(
                text,
                "observation={},{},",
                team_text(initial.observer.team),
                initial.observer.shirt,
            )
            .expect("writing to String cannot fail");
            match initial.subject {
                ObservationSubject::Ball => write!(text, "ball,"),
                ObservationSubject::Player(subject) => write!(
                    text,
                    "player,{}, {},",
                    team_text(subject.team),
                    subject.shirt
                ),
            }
            .expect("writing to String cannot fail");
            writeln!(
                text,
                "{},{},{},{},{}",
                initial.observation.spot.x,
                initial.observation.spot.y,
                initial.observation.velocity.x,
                initial.observation.velocity.y,
                initial.observation.blur,
            )
            .expect("writing to String cannot fail");
        }
        Ok(text)
    }

    /// Imports a versioned teaching situation. The constructed value uses the
    /// canonical context guaranteed by the header; only its declared initial
    /// state reaches the simulation.
    pub fn from_compact_situation_text(text: &str) -> Result<Self, SituationTextError> {
        let mut lines = text.lines();
        if lines.next() != Some(SITUATION_TEXT_HEADER) {
            return Err(SituationTextError(format!(
                "expected header {SITUATION_TEXT_HEADER:?}"
            )));
        }
        let mut scenario = Scenario::kick_off();
        scenario.player_placements.clear();
        scenario.movement_proposals.clear();
        scenario.initial_observations.clear();
        let mut name_seen = false;
        let mut seed_seen = false;
        let mut players_seen = false;
        let mut ball_seen = false;
        let mut play_seen = false;
        let mut window_seen = false;

        for (line_index, line) in lines.enumerate() {
            if line.is_empty() {
                continue;
            }
            let line_number = line_index + 2;
            let (key, value) = line.split_once('=').ok_or_else(|| {
                SituationTextError(format!("line {line_number}: expected key=value"))
            })?;
            match key {
                "name" if !name_seen => {
                    scenario.name = value.to_string();
                    name_seen = true;
                }
                "seed" if !seed_seen => {
                    scenario.seed = parse_value(value, line_number, "seed")?;
                    seed_seen = true;
                }
                "players" if !players_seen => {
                    scenario.players = parse_player_setup(value, line_number)?;
                    players_seen = true;
                }
                "ball" if !ball_seen => {
                    let fields = comma_fields(value, line_number, "ball", 7)?;
                    scenario.ball = BallSetup {
                        position: Vec3::new(
                            parse_value(fields[0], line_number, "ball x")?,
                            parse_value(fields[1], line_number, "ball y")?,
                            parse_value(fields[2], line_number, "ball z")?,
                        ),
                        momentum: Vec3::new(
                            parse_value(fields[3], line_number, "ball velocity x")?,
                            parse_value(fields[4], line_number, "ball velocity y")?,
                            parse_value(fields[5], line_number, "ball velocity z")?,
                        ),
                        last_touched_by_team: parse_optional_team(fields[6], line_number)?,
                    };
                    ball_seen = true;
                }
                "play" if !play_seen => {
                    scenario.play_state = parse_play_state(value, line_number)?;
                    play_seen = true;
                }
                "window-ms" if !window_seen => {
                    scenario.window =
                        Duration::from_millis(parse_value(value, line_number, "window-ms")?);
                    window_seen = true;
                }
                "placement" => scenario
                    .player_placements
                    .push(parse_placement(value, line_number)?),
                "proposal" => scenario
                    .movement_proposals
                    .push(parse_proposal(value, line_number)?),
                "observation" => scenario
                    .initial_observations
                    .push(parse_initial_observation(value, line_number)?),
                _ => {
                    return Err(SituationTextError(format!(
                        "line {line_number}: unknown or repeated field {key:?}"
                    )));
                }
            }
        }
        if !(name_seen && seed_seen && players_seen && ball_seen && play_seen && window_seen) {
            return Err(SituationTextError(
                "a situation needs name, seed, players, ball, play and window-ms".to_string(),
            ));
        }
        let contradictions = scenario.contradictions();
        if contradictions.is_empty() {
            Ok(scenario)
        } else {
            Err(SituationTextError(format!(
                "the imported situation contradicts itself: {}",
                contradictions.join("; ")
            )))
        }
    }

    fn require_canonical_situation_context(&self) -> Result<(), SituationTextError> {
        if self.laws_edition != LawsEdition::Ifab2026_27
            || self.competition.is_some()
            || self.pitch != PitchConfig::default()
            || self.regulations != MatchRegulations::default()
            || self.tuning != MatchTuning::default()
            || self.tactical_plans != TacticalPlans::default()
            || !self.substitutions.is_empty()
        {
            return Err(SituationTextError(
                "this text format only carries the canonical match context; export a scenario with custom laws, competition, pitch, regulations, tuning, plan or substitutions as Rust data"
                    .to_string(),
            ));
        }
        Ok(())
    }
}

fn comma_fields<'a>(
    value: &'a str,
    line: usize,
    field: &str,
    expected: usize,
) -> Result<Vec<&'a str>, SituationTextError> {
    let fields: Vec<_> = value.split(',').map(str::trim).collect();
    if fields.len() == expected && fields.iter().all(|field| !field.is_empty()) {
        Ok(fields)
    } else {
        Err(SituationTextError(format!(
            "line {line}: {field} needs {expected} comma-separated values"
        )))
    }
}

fn parse_value<T: std::str::FromStr>(
    value: &str,
    line: usize,
    field: &str,
) -> Result<T, SituationTextError> {
    value.parse().map_err(|_| {
        SituationTextError(format!(
            "line {line}: {field} is not a valid value: {value:?}"
        ))
    })
}

fn team_text(team: TeamId) -> &'static str {
    match team {
        TeamId::Home => "home",
        TeamId::Away => "away",
    }
}

fn optional_team_text(team: Option<TeamId>) -> &'static str {
    team.map(team_text).unwrap_or("none")
}

fn parse_team(value: &str, line: usize) -> Result<TeamId, SituationTextError> {
    match value {
        "home" => Ok(TeamId::Home),
        "away" => Ok(TeamId::Away),
        _ => Err(SituationTextError(format!(
            "line {line}: expected team home or away, got {value:?}"
        ))),
    }
}

fn parse_optional_team(value: &str, line: usize) -> Result<Option<TeamId>, SituationTextError> {
    if value == "none" {
        Ok(None)
    } else {
        parse_team(value, line).map(Some)
    }
}

fn player_setup_text(setup: PlayerSetup) -> &'static str {
    match setup {
        PlayerSetup::DefaultFormations => "default-formations",
        PlayerSetup::BallOnly => "ball-only",
        PlayerSetup::GoalkeepersOnly => "goalkeepers-only",
    }
}

fn parse_player_setup(value: &str, line: usize) -> Result<PlayerSetup, SituationTextError> {
    match value {
        "default-formations" => Ok(PlayerSetup::DefaultFormations),
        "ball-only" => Ok(PlayerSetup::BallOnly),
        "goalkeepers-only" => Ok(PlayerSetup::GoalkeepersOnly),
        _ => Err(SituationTextError(format!(
            "line {line}: unknown player setup {value:?}"
        ))),
    }
}

fn set_piece_text(set_piece: SetPiece) -> &'static str {
    match set_piece {
        SetPiece::None => "none",
        SetPiece::KickOff => "kick-off",
        SetPiece::GoalKick => "goal-kick",
        SetPiece::FreeKick => "free-kick",
        SetPiece::Corner => "corner",
        SetPiece::ThrowIn => "throw-in",
        SetPiece::Penalty => "penalty",
        SetPiece::DroppedBall => "dropped-ball",
    }
}

fn parse_set_piece(value: &str, line: usize) -> Result<SetPiece, SituationTextError> {
    match value {
        "none" => Ok(SetPiece::None),
        "kick-off" => Ok(SetPiece::KickOff),
        "goal-kick" => Ok(SetPiece::GoalKick),
        "free-kick" => Ok(SetPiece::FreeKick),
        "corner" => Ok(SetPiece::Corner),
        "throw-in" => Ok(SetPiece::ThrowIn),
        "penalty" => Ok(SetPiece::Penalty),
        "dropped-ball" => Ok(SetPiece::DroppedBall),
        _ => Err(SituationTextError(format!(
            "line {line}: unknown set piece {value:?}"
        ))),
    }
}

fn parse_play_state(value: &str, line: usize) -> Result<PlayState, SituationTextError> {
    if value == "in-play" {
        return Ok(PlayState::InPlay);
    }
    let fields = comma_fields(value, line, "play", 4)?;
    if fields[0] != "awaiting-restart" {
        return Err(SituationTextError(format!(
            "line {line}: unknown play state {:?}",
            fields[0]
        )));
    }
    Ok(PlayState::AwaitingRestart {
        set_piece: parse_set_piece(fields[1], line)?,
        team: parse_team(fields[2], line)?,
        delay: Duration::from_millis(parse_value(fields[3], line, "restart delay")?),
    })
}

fn playing_position_text(position: PlayingPosition) -> &'static str {
    match position {
        PlayingPosition::Goalkeeper => "goalkeeper",
        PlayingPosition::CentreBack => "centre-back",
        PlayingPosition::LeftBack => "left-back",
        PlayingPosition::RightBack => "right-back",
        PlayingPosition::DefensiveMidfielder => "defensive-midfielder",
        PlayingPosition::CentreMidfielder => "centre-midfielder",
        PlayingPosition::LeftMidfielder => "left-midfielder",
        PlayingPosition::RightMidfielder => "right-midfielder",
        PlayingPosition::AttackingMidfielder => "attacking-midfielder",
        PlayingPosition::CentreForward => "centre-forward",
    }
}

fn parse_playing_position(value: &str, line: usize) -> Result<PlayingPosition, SituationTextError> {
    match value {
        "goalkeeper" => Ok(PlayingPosition::Goalkeeper),
        "centre-back" => Ok(PlayingPosition::CentreBack),
        "left-back" => Ok(PlayingPosition::LeftBack),
        "right-back" => Ok(PlayingPosition::RightBack),
        "defensive-midfielder" => Ok(PlayingPosition::DefensiveMidfielder),
        "centre-midfielder" => Ok(PlayingPosition::CentreMidfielder),
        "left-midfielder" => Ok(PlayingPosition::LeftMidfielder),
        "right-midfielder" => Ok(PlayingPosition::RightMidfielder),
        "attacking-midfielder" => Ok(PlayingPosition::AttackingMidfielder),
        "centre-forward" => Ok(PlayingPosition::CentreForward),
        _ => Err(SituationTextError(format!(
            "line {line}: unknown playing position {value:?}"
        ))),
    }
}

fn tactical_role_text(role: TacticalRole) -> &'static str {
    match role {
        TacticalRole::Defending => "defending",
        TacticalRole::Holding => "holding",
        TacticalRole::Linking => "linking",
        TacticalRole::Creating => "creating",
        TacticalRole::Attacking => "attacking",
    }
}

fn parse_tactical_role(value: &str, line: usize) -> Result<TacticalRole, SituationTextError> {
    match value {
        "defending" => Ok(TacticalRole::Defending),
        "holding" => Ok(TacticalRole::Holding),
        "linking" => Ok(TacticalRole::Linking),
        "creating" => Ok(TacticalRole::Creating),
        "attacking" => Ok(TacticalRole::Attacking),
        _ => Err(SituationTextError(format!(
            "line {line}: unknown tactical role {value:?}"
        ))),
    }
}

fn parse_player_id(team: &str, shirt: &str, line: usize) -> Result<PlayerId, SituationTextError> {
    Ok(PlayerId::new(
        parse_team(team, line)?,
        parse_value(shirt, line, "shirt number")?,
    ))
}

fn parse_placement(value: &str, line: usize) -> Result<PlayerPlacement, SituationTextError> {
    let fields = comma_fields(value, line, "placement", 9)?;
    Ok(PlayerPlacement {
        id: parse_player_id(fields[0], fields[1], line)?,
        position: parse_playing_position(fields[2], line)?,
        role: parse_tactical_role(fields[3], line)?,
        formation_slot: Vec2::new(
            parse_value(fields[4], line, "formation x")?,
            parse_value(fields[5], line, "formation y")?,
        ),
        on_pitch: Vec3::new(
            parse_value(fields[6], line, "pitch x")?,
            parse_value(fields[7], line, "pitch y")?,
            parse_value(fields[8], line, "pitch z")?,
        ),
    })
}

fn parse_proposal(value: &str, line: usize) -> Result<MovementProposal, SituationTextError> {
    let fields = comma_fields(value, line, "proposal", 4)?;
    Ok(MovementProposal {
        player: parse_player_id(fields[0], fields[1], line)?,
        desired_velocity: Vec2::new(
            parse_value(fields[2], line, "proposal velocity x")?,
            parse_value(fields[3], line, "proposal velocity y")?,
        ),
    })
}

fn parse_initial_observation(
    value: &str,
    line: usize,
) -> Result<InitialObservation, SituationTextError> {
    let fields: Vec<_> = value.split(',').map(str::trim).collect();
    let observer = fields
        .first()
        .zip(fields.get(1))
        .ok_or_else(|| SituationTextError(format!("line {line}: observation needs an observer")))
        .and_then(|(team, shirt)| parse_player_id(team, shirt, line))?;
    let (subject, values_at) = match fields.get(2).copied() {
        Some("ball") => (ObservationSubject::Ball, 3),
        Some("player") => {
            let team = *fields.get(3).ok_or_else(|| {
                SituationTextError(format!("line {line}: player observation needs a team"))
            })?;
            let shirt = *fields.get(4).ok_or_else(|| {
                SituationTextError(format!("line {line}: player observation needs a shirt"))
            })?;
            (
                ObservationSubject::Player(parse_player_id(team, shirt, line)?),
                5,
            )
        }
        _ => {
            return Err(SituationTextError(format!(
                "line {line}: observation subject must be ball or player"
            )));
        }
    };
    if fields.len() != values_at + 5 || fields.iter().any(|field| field.is_empty()) {
        return Err(SituationTextError(format!(
            "line {line}: observation has the wrong number of values"
        )));
    }
    Ok(InitialObservation {
        observer,
        subject,
        observation: Observation {
            spot: Vec2::new(
                parse_value(fields[values_at], line, "observation x")?,
                parse_value(fields[values_at + 1], line, "observation y")?,
            ),
            velocity: Vec2::new(
                parse_value(fields[values_at + 2], line, "observation velocity x")?,
                parse_value(fields[values_at + 3], line, "observation velocity y")?,
            ),
            seen_at: Duration::ZERO,
            blur: parse_value(fields[values_at + 4], line, "observation blur")?,
        },
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_teaching_situation_round_trips_its_initial_state_and_beliefs() {
        let ball_belief = Observation {
            spot: Vec2::new(14.5, -2.0),
            velocity: Vec2::new(2.5, 1.0),
            seen_at: Duration::ZERO,
            blur: 1.2,
        };
        let teammate_belief = Observation {
            spot: Vec2::new(9.0, 4.0),
            velocity: Vec2::new(-1.0, 0.0),
            seen_at: Duration::ZERO,
            blur: 0.5,
        };
        let mut source = Scenario::kick_off()
            .named("support after a diagonal pass")
            .with_ball(
                BallSetup::travelling_from(Vec3::new(11.0, -3.0, 0.8), Vec3::new(7.0, 1.5, 0.0))
                    .last_touched_by(TeamId::Away),
            )
            .already_in_play()
            .for_duration(Duration::from_millis(7300))
            .with_player_placements(vec![PlayerPlacement {
                id: PlayerId::home(6),
                position: PlayingPosition::AttackingMidfielder,
                role: TacticalRole::Creating,
                formation_slot: Vec2::new(0.4, -0.2),
                on_pitch: Vec3::new(13.0, -9.0, 0.0),
            }])
            .with_movement_proposals(vec![MovementProposal {
                player: PlayerId::home(6),
                desired_velocity: Vec2::new(3.0, -2.0),
            }])
            .with_substitutions(vec![Substitution::new(
                PlayerId::home(9),
                PlayerId::home(19),
            )])
            .with_initial_observations(vec![
                InitialObservation {
                    observer: PlayerId::home(6),
                    subject: ObservationSubject::Ball,
                    observation: ball_belief,
                },
                InitialObservation {
                    observer: PlayerId::home(6),
                    subject: ObservationSubject::Player(PlayerId::away(4)),
                    observation: teammate_belief,
                },
            ]);
        source.regulations.maximum_substitutions = 3;
        source.tuning.shooting.odds_threshold = 0.73;
        source.tactical_plans.team[TeamId::Away].press_distance = 11.0;

        let text = source
            .to_situation_text()
            .expect("the canonical state exports");
        let imported = Scenario::from_situation_text(&text).expect("the exported state imports");

        assert_eq!(imported.name, source.name);
        assert_eq!(imported.seed, source.seed);
        assert_eq!(imported.players, source.players);
        assert_eq!(imported.ball, source.ball);
        assert!(matches!(imported.play_state, PlayState::InPlay));
        assert_eq!(imported.window, source.window);
        assert_eq!(imported.player_placements, source.player_placements);
        assert_eq!(imported.movement_proposals, source.movement_proposals);
        assert_eq!(imported.initial_observations, source.initial_observations);

        let complete =
            Scenario::from_ron(&source.to_ron().expect("RON exports")).expect("RON imports");
        assert_eq!(complete.tactical_plans, source.tactical_plans);
        assert_eq!(complete.regulations, source.regulations);
        assert_eq!(complete.tuning, source.tuning);
        assert_eq!(complete.laws_edition, source.laws_edition);
        assert_eq!(complete.competition, source.competition);
        assert_eq!(complete.pitch, source.pitch);
        assert_eq!(complete.substitutions, source.substitutions);
        assert_eq!(complete.expectations.score, source.expectations.score);
        assert_eq!(
            complete.expectations.set_pieces,
            source.expectations.set_pieces
        );
        assert_eq!(complete.expectations.phases, source.expectations.phases);
        assert_eq!(
            complete.expectations.play_resumes,
            source.expectations.play_resumes
        );
        assert_eq!(
            complete.expectations.play_never_stops,
            source.expectations.play_never_stops
        );
    }

    #[test]
    fn a_situation_rejects_an_observation_for_a_body_that_is_not_present() {
        let scenario = Scenario::kick_off()
            .with_players(PlayerSetup::BallOnly)
            .with_initial_observations(vec![InitialObservation {
                observer: PlayerId::home(6),
                subject: ObservationSubject::Ball,
                observation: Observation {
                    spot: Vec2::ZERO,
                    velocity: Vec2::ZERO,
                    seen_at: Duration::ZERO,
                    blur: 0.0,
                },
            }]);

        assert!(
            scenario
                .contradictions()
                .iter()
                .any(|message| message.contains("not on the pitch"))
        );
    }

    #[test]
    fn ron_rejects_a_proposal_for_a_player_the_situation_did_not_field() {
        let invalid = Scenario::kick_off().with_movement_proposals(vec![MovementProposal {
            player: PlayerId::home(19),
            desired_velocity: Vec2::X,
        }]);

        assert!(Scenario::from_ron(&invalid.to_ron().expect("RON exports")).is_err());
    }

    #[test]
    fn public_import_keeps_reading_the_versioned_compact_document() {
        let source = Scenario::kick_off().for_duration(Duration::from_secs(10));
        let compact = source
            .to_compact_situation_text()
            .expect("canonical compact situation exports");

        let imported = Scenario::from_situation_text(&compact).expect("legacy document imports");
        assert_eq!(imported.name, source.name);
        assert_eq!(imported.window, source.window);
    }

    #[test]
    fn a_shootout_probability_must_be_strictly_between_zero_and_one() {
        let mut scenario = Scenario::kick_off();
        scenario.regulations.shootout_conversion_probability = 0.0;

        assert!(scenario.validate().is_err());
    }
}
