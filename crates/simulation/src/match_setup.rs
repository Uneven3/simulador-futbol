use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bevy_math::prelude::*;
use bevy_time::prelude::*;
use football_domain::scenario::{PlayState, PlayerSetup, SIMULATION_HZ, Scenario};
use football_domain::{
    Ball, BallTouched, DefensiveAction, Discipline, Facing, FatigueState, Gaze, Looking, MatchRng,
    MatchState, Mentality, MovementIntent, ObservationMemory, PitchSides, Player, PlayerId,
    PlayerMatchState, PlayerRegistry, PlayingPosition, Position, SetPiece, Stance,
    TacticalResponsibility, TeamId, TeamSide, Velocity, perception_profile, player_attributes,
    tactical_familiarity,
};
use std::time::Duration;

/// Installs a scenario: the state the match starts from, its seed, its pitch and
/// its bodies — the single source of the initial situation, so a run is
/// reproducible from one value. Nothing here is renderable (§1).
pub struct MatchSetupPlugin {
    scenario: Scenario,
}

impl MatchSetupPlugin {
    pub fn new(scenario: Scenario) -> Self {
        Self { scenario }
    }
}

impl Plugin for MatchSetupPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.scenario.clone())
            .insert_resource(initial_match_state(&self.scenario))
            .insert_resource(self.scenario.pitch.clone())
            .insert_resource(self.scenario.regulations.clone())
            .insert_resource(self.scenario.tuning.clone())
            .insert_resource(self.scenario.tactical_plans.clone())
            .insert_resource(MatchRng::seeded(self.scenario.seed))
            .insert_resource(PlayerRegistry::default())
            .insert_resource(Time::<Fixed>::from_hz(SIMULATION_HZ))
            .add_message::<BallTouched>()
            .add_systems(Startup, (spawn_scenario_ball, spawn_scenario_players))
            .add_systems(PreUpdate, (register_bodies, forget_departed_bodies));
    }
}

/// The match state a scenario opens in: stopped awaiting a restart, or live.
fn initial_match_state(scenario: &Scenario) -> MatchState {
    let mut state = MatchState::default();
    match scenario.play_state {
        PlayState::AwaitingRestart {
            set_piece,
            team,
            delay,
        } => {
            state.set_piece = set_piece;
            state.set_piece_team = Some(team);
            state.restart_in = delay;
            state.restart_pos = Vec3::ZERO;
            state.opening_kick_off_team = team;
        }
        PlayState::InPlay => {
            state.set_piece = SetPiece::None;
            state.set_piece_team = None;
            state.restart_in = Duration::ZERO;
        }
    }
    state
}

/// Keeps the identity → body index in step with the world. Once per frame and
/// not per tick: bodies are only created at setup, and substitutions will arrive
/// as a request served at a restart.
fn register_bodies(
    mut registry: ResMut<PlayerRegistry>,
    arrived: Query<(Entity, &Player), Added<Player>>,
) {
    for (body, player) in &arrived {
        registry.insert(player.id, body);
    }
}

fn forget_departed_bodies(
    mut registry: ResMut<PlayerRegistry>,
    mut departed: RemovedComponents<Player>,
) {
    for body in departed.read() {
        registry.remove_body(body);
    }
}

fn spawn_scenario_ball(mut commands: Commands, scenario: Res<Scenario>) {
    let setup = scenario.ball;
    let mut ball = Ball::placed_at(setup.position, setup.momentum);
    ball.last_touch_team = setup.last_touched_by_team;

    commands.spawn((Name::new("Ball"), ball, Position(setup.position)));
}

/// The default 4-4-2 for both teams: eleven bodies each, at their base
/// formation positions, standing still and facing the opponent goal.
fn spawn_scenario_players(mut commands: Commands, scenario: Res<Scenario>) {
    if scenario.players == PlayerSetup::BallOnly {
        return;
    }

    /// The default 4-4-2, shirt 1 to 11 in the order football numbers them.
    const LINE_UP: [PlayingPosition; 11] = [
        PlayingPosition::Goalkeeper,
        PlayingPosition::LeftBack,
        PlayingPosition::CentreBack,
        PlayingPosition::CentreBack,
        PlayingPosition::RightBack,
        PlayingPosition::LeftMidfielder,
        PlayingPosition::CentreMidfielder,
        PlayingPosition::CentreMidfielder,
        PlayingPosition::RightMidfielder,
        PlayingPosition::CentreForward,
        PlayingPosition::CentreForward,
    ];

    let sides = PitchSides::opening();
    for team in TeamId::BOTH {
        for (index, position) in LINE_UP.iter().enumerate() {
            if scenario.players == PlayerSetup::GoalkeepersOnly
                && *position != PlayingPosition::Goalkeeper
            {
                continue;
            }
            let shirt_number = u8::try_from(index + 1).expect("LINE_UP is eleven players");
            let id = PlayerId::new(team, shirt_number);
            let base = base_formation_position(id, *position, sides);
            let placement = scenario
                .player_placements
                .iter()
                .find(|placement| placement.id == id);
            let (playing_position, role, formation_slot, on_pitch) = placement.map_or_else(
                || {
                    (
                        *position,
                        position.default_role(),
                        normalized_formation_position(id, *position),
                        Position::from_pitch(base, 0.0),
                    )
                },
                |placement| {
                    (
                        placement.position,
                        placement.role,
                        placement.formation_slot,
                        Position(placement.on_pitch),
                    )
                },
            );
            spawn_player_body(
                &mut commands,
                &scenario,
                id,
                playing_position,
                role,
                formation_slot,
                on_pitch,
            );
        }
    }
}

/// Installs one authoritative player body. Setup and substitutions share this
/// constructor so an entrant receives every domain component a starter has.
pub(crate) fn spawn_player_body(
    commands: &mut Commands,
    scenario: &Scenario,
    id: PlayerId,
    playing_position: PlayingPosition,
    role: football_domain::TacticalRole,
    formation_slot: Vec2,
    on_pitch: Position,
) {
    let perception = perception_profile(
        scenario.seed,
        id.shirt,
        &scenario.tuning.perception,
        &scenario.tuning.turning,
    );
    let (position_familiarity, role_familiarity) = tactical_familiarity(scenario.seed, id.shirt);
    let attacking_direction = if PitchSides::opening().attacking_x(id.team) > 0.0 {
        Dir2::X
    } else {
        Dir2::NEG_X
    };
    commands.spawn((
        Name::new(format!("{id} - {playing_position:?}")),
        Player {
            id,
            position: playing_position,
            role,
            formation_slot,
        },
        player_attributes(scenario.seed, id.shirt, playing_position),
        Mentality::default(),
        PlayerMatchState::default(),
        Velocity::default(),
        MovementIntent::default(),
        Gaze::default(),
        Stance::default(),
        FatigueState::default(),
        (
            perception.judgement,
            perception.senses,
            TacticalResponsibility::default(),
            position_familiarity,
            role_familiarity,
            DefensiveAction::default(),
            Discipline::default(),
        ),
        initial_memory(scenario, id),
        Facing(attacking_direction),
        Looking(attacking_direction),
        on_pitch,
    ));
}

/// A teaching situation may explicitly say what a player already knows. That
/// is installed while the body is born, before any perception system runs; it
/// is not reconstructed from authoritative positions or the authoritative ball.
fn initial_memory(scenario: &Scenario, id: PlayerId) -> ObservationMemory {
    let mut memory = ObservationMemory::default();
    for initial in scenario
        .initial_observations
        .iter()
        .filter(|initial| initial.observer == id)
    {
        match initial.subject {
            football_domain::ObservationSubject::Ball => memory.remember_ball(initial.observation),
            football_domain::ObservationSubject::Player(subject) => {
                memory.remember(subject, initial.observation);
            }
        }
    }
    memory
}

/// Normalized formation entry (original `FormationEntry::position`, -1..1) for
/// the default 4-4-2; `AI_GetAdaptedFormationPosition` scales it into the team
/// block. Paired positions are told apart by shirt, as a team sheet does.
pub fn normalized_formation_position(id: PlayerId, position: PlayingPosition) -> Vec2 {
    match position {
        PlayingPosition::Goalkeeper => Vec2::new(-1.0, 0.0),
        PlayingPosition::LeftBack => Vec2::new(-1.0, -1.0),
        PlayingPosition::CentreBack => Vec2::new(-1.0, if id.shirt == 3 { -0.33 } else { 0.33 }),
        PlayingPosition::RightBack => Vec2::new(-1.0, 1.0),
        PlayingPosition::LeftMidfielder => Vec2::new(0.0, -1.0),
        PlayingPosition::CentreMidfielder => {
            Vec2::new(0.0, if id.shirt == 7 { -0.33 } else { 0.33 })
        }
        PlayingPosition::RightMidfielder => Vec2::new(0.0, 1.0),
        PlayingPosition::DefensiveMidfielder => Vec2::new(-0.5, 0.0),
        PlayingPosition::AttackingMidfielder => Vec2::new(0.5, 0.0),
        PlayingPosition::CentreForward => Vec2::new(1.0, if id.shirt == 10 { -0.4 } else { 0.4 }),
    }
}

/// Base (kick-off) position on the pitch in metres, used both to spawn and to
/// re-form the teams for a restart.
pub fn base_formation_position(id: PlayerId, position: PlayingPosition, sides: PitchSides) -> Vec2 {
    let home_position = match position {
        PlayingPosition::Goalkeeper => Vec2::new(-54.0, 0.0),
        PlayingPosition::LeftBack => Vec2::new(-35.0, -18.0),
        PlayingPosition::CentreBack => {
            if id.shirt == 3 {
                Vec2::new(-35.0, -6.0)
            } else {
                Vec2::new(-35.0, 6.0)
            }
        }
        PlayingPosition::RightBack => Vec2::new(-35.0, 18.0),
        PlayingPosition::LeftMidfielder => Vec2::new(-20.0, -20.0),
        PlayingPosition::CentreMidfielder => {
            if id.shirt == 7 {
                Vec2::new(-20.0, -7.0)
            } else {
                Vec2::new(-20.0, 7.0)
            }
        }
        PlayingPosition::RightMidfielder => Vec2::new(-20.0, 20.0),
        PlayingPosition::DefensiveMidfielder => Vec2::new(-25.0, 0.0),
        PlayingPosition::AttackingMidfielder => Vec2::new(-15.0, 0.0),
        PlayingPosition::CentreForward => {
            if id.shirt == 10 {
                Vec2::new(-7.0, -10.0)
            } else {
                Vec2::new(-7.0, 10.0)
            }
        }
    };

    // La plantilla se escribe defendiendo a la izquierda; el equipo que defiende
    // a la derecha la ve rotada media vuelta. Con esto, cambiar de mitad en el
    // descanso no necesita otra tabla de posiciones.
    if sides.defended_by(id.team) == TeamSide::Left {
        home_position
    } else {
        Vec2::new(-home_position.x, -home_position.y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use football_domain::{InitialObservation, Observation, ObservationSubject, PlayerPlacement};

    #[test]
    fn a_scenario_installs_an_explicit_player_placement() {
        let placement = PlayerPlacement {
            id: PlayerId::home(6),
            position: PlayingPosition::AttackingMidfielder,
            role: football_domain::TacticalRole::Creating,
            formation_slot: Vec2::new(0.4, -0.2),
            on_pitch: Vec3::new(13.0, -9.0, 0.0),
        };
        let scenario = Scenario::kick_off().with_player_placements(vec![placement]);
        let mut app = App::new();
        app.add_plugins(MatchSetupPlugin::new(scenario));
        app.update();

        let mut players = app.world_mut().query::<(&Player, &Position)>();
        let (player, position) = players
            .iter(app.world())
            .find(|(player, _)| player.id == placement.id)
            .expect("el dorsal declarado fue instalado");

        assert_eq!(player.position, placement.position);
        assert_eq!(player.role, placement.role);
        assert_eq!(player.formation_slot, placement.formation_slot);
        assert_eq!(position.0, placement.on_pitch);
    }

    #[test]
    fn a_scenario_installs_declared_initial_beliefs_without_reading_the_world() {
        let observer = PlayerId::home(6);
        let belief = Observation {
            spot: Vec2::new(7.0, -3.0),
            velocity: Vec2::new(1.0, 0.5),
            seen_at: Duration::ZERO,
            blur: 2.0,
        };
        let scenario = Scenario::kick_off().with_initial_observations(vec![InitialObservation {
            observer,
            subject: ObservationSubject::Ball,
            observation: belief,
        }]);
        let mut app = App::new();
        app.add_plugins(MatchSetupPlugin::new(scenario));
        app.update();

        let mut memories = app.world_mut().query::<(&Player, &ObservationMemory)>();
        let (_, memory) = memories
            .iter(app.world())
            .find(|(player, _)| player.id == observer)
            .expect("el observador declarado fue instalado");
        assert_eq!(memory.ball(), Some(belief));
    }
}
